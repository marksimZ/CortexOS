use super::{
    dataset::{self, Case, Expected},
    preflight::{ModelRequest, Preflight},
};
use crate::{
    db::{Database, error, event, insert_execution, json_error},
    models::*,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartInput {
    pub launch_key: String,
    pub development_only: bool,
    pub dataset_hash: String,
    pub case_id: String,
    pub model: ModelRequest,
    #[serde(default = "legacy_version", skip_serializing_if = "is_legacy")]
    pub dataset_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personalized_dataset: Option<String>,
}
fn legacy_version() -> u32 {
    1
}
fn is_legacy(version: &u32) -> bool {
    *version == 1
}
pub struct Intent {
    pub input: StartInput,
    pub case: Case,
    pub manifest: Value,
    pub config: Value,
    pub config_hash: String,
}
pub struct Prepared {
    pub summary: Summary,
    pub execution: PreparedRun,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub workspace_id: String,
    pub status: String,
    pub development_only: bool,
    pub case_run_id: String,
    pub attempt_id: String,
    pub execution_run_id: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub summary: Summary,
    pub config: Value,
    pub model: Value,
    pub case: Case,
    pub execution: RunDetail,
    pub output: String,
    pub attempt: Value,
    pub evaluations: Vec<Value>,
    pub events: Vec<Value>,
    pub hardware: Value,
    pub resources: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<Value>,
}
fn encode<T: Serialize>(v: &T) -> AppResult<String> {
    serde_json::to_string(v).map_err(json_error)
}
fn value<T: Serialize>(v: &T) -> AppResult<Value> {
    serde_json::to_value(v).map_err(json_error)
}
fn source_hash() -> String {
    // Fingerprints the execution implementation, not the full repository or a release commit.
    dataset::hash(
        concat!(
            include_str!("../commands.rs"),
            include_str!("../db.rs"),
            include_str!("../models.rs"),
            include_str!("../ollama.rs"),
            include_str!("../generation.rs"),
            include_str!("../tasks.rs"),
            include_str!("dataset.rs"),
            include_str!("evaluation.rs"),
            include_str!("evaluation_json.rs"),
            include_str!("python.rs"),
            include_str!("scoring_repository.rs"),
            include_str!("preflight.rs"),
            include_str!("telemetry.rs"),
            include_str!("resources.rs"),
            include_str!("repository.rs"),
            include_str!("runner.rs"),
            include_str!("runner_v2.rs"),
            include_str!("python_runner.py"),
            include_str!("dataset_v2.rs"),
            include_str!("personalized/cases.rs"),
            include_str!("personalized/cloud_run.rs"),
            include_str!("personalized/batch.rs"),
            include_str!("../cloud/completion.rs"),
            include_str!("../../Cargo.lock"),
            include_str!("../../Cargo.toml")
        )
        .as_bytes(),
    )
}
pub fn benchmark_event(
    c: &Connection,
    id: &str,
    workspace: &str,
    attempt: &str,
    kind: &str,
    payload: Value,
) -> AppResult<()> {
    c.execute("INSERT INTO benchmark_events(run_id,seq,workspace_id,kind,occurred_at,attempt_id,payload_json) VALUES(?1,(SELECT COALESCE(MAX(seq),0)+1 FROM benchmark_events WHERE run_id=?1),?2,?3,?4,?5,?6)",params![id,workspace,kind,now(),attempt,payload.to_string()]).map_err(error)?;
    Ok(())
}

impl Database {
    pub fn benchmark_intent(&self, workspace: &str, input: StartInput) -> AppResult<Intent> {
        input.model.validate()?;
        let personal = input.personalized_dataset.is_some();
        if !personal
            && (!input.development_only
                || ![1, 2].contains(&input.dataset_version)
                || (input.dataset_version == 1
                    && !["math-001", "zh-006"].contains(&input.case_id.as_str()))
                || input.model.expected_digest.is_none()
                || (input.dataset_version == 1 && input.model.requirements != ["completion"]))
            || input.launch_key.is_empty()
            || input.launch_key.len() > 128
            || input.launch_key.chars().any(char::is_control)
        {
            return Err("本切片仅支持已固定模型版本的单题开发检查（math-001或zh-006），请明确选择开发模式并提供有效启动标识。".into());
        }
        let dataset_id = input
            .personalized_dataset
            .as_deref()
            .unwrap_or("cortex-core-60");
        let dataset = if personal {
            self.personalized_dataset(workspace, dataset_id, input.dataset_version)?
        } else {
            self.benchmark_datasets(workspace)?
                .into_iter()
                .find(|d| d.id == dataset_id && d.version == input.dataset_version)
                .ok_or("所选测试集版本不存在。")?
        };
        if dataset.content_hash != input.dataset_hash {
            return Err("测试集版本指纹已变化，请重新读取测试集后开始。".into());
        }
        let case =
            self.benchmark_case(workspace, dataset_id, input.dataset_version, &input.case_id)?;
        let needed: Vec<_> = case
            .requirements
            .iter()
            .filter(|s| s.as_str() != "python_sandbox")
            .cloned()
            .collect();
        if input.model.requirements != needed
            || input
                .python_image
                .as_ref()
                .is_some_and(|s| !super::python::valid_image(s))
        {
            return Err("模型能力要求或Python镜像内容标识无效。".into());
        }
        let rules: Value = serde_json::from_str(if input.dataset_version == 1 && !personal {
            include_str!("../../datasets/cortex-core-60/scoring-rules.json")
        } else {
            include_str!("../../datasets/cortex-core-60-v2/scoring-rules.json")
        })
        .map_err(json_error)?;
        let rule = rules["rules"]
            .as_array()
            .and_then(|r| {
                r.iter()
                    .find(|r| r["id"] == case.scoring.id && r["version"] == case.scoring.version)
            })
            .ok_or("本题的评分规则不可用。")?;
        let manifest = dataset.manifest.clone();
        let mut config = json!({"mode":"development_single_case_v1","repetitions":1,"input":input,"caseHash":case.content_hash,"datasetId":dataset_id,"datasetVersion":input.dataset_version,"reviewStatus":manifest["review_status"],"scoringRule":rule,"promptTemplate":manifest["prompt_template"]});
        if input.dataset_version == 2 || personal {
            config["mode"] = json!("development_single_case_v2");
            config["caseScoring"] = serde_json::to_value(&case.scoring).map_err(json_error)?;
            config["sandboxPolicy"] = super::python::policy();
        }
        if personal {
            config["mode"] = json!("personalized_single_case_v1");
        }
        let config_hash = dataset::hash(encode(&config)?.as_bytes());
        Ok(Intent {
            input,
            case,
            manifest,
            config,
            config_hash,
        })
    }

    pub fn benchmark_existing(
        &self,
        workspace: &str,
        intent: &Intent,
    ) -> AppResult<Option<Summary>> {
        let c = self.connection()?;
        dataset::require_workspace(&c, workspace)?;
        let existing: Option<(String, String)> = c
            .query_row(
                "SELECT id,config_hash FROM benchmark_runs WHERE workspace_id=?1 AND launch_key=?2",
                params![workspace, intent.input.launch_key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(error)?;
        match existing {
            Some((id, hash)) if hash == intent.config_hash => {
                Ok(Some(summary(&c, workspace, &id)?))
            }
            Some(_) => Err("该启动标识已用于其他配置，请查看原实验或使用新的启动标识。".into()),
            None => Ok(None),
        }
    }

    pub fn prepare_benchmark(
        &self,
        workspace: &str,
        id: &str,
        intent: &Intent,
        inspection: &Preflight,
    ) -> AppResult<Prepared> {
        if inspection.status != "ready_for_probe"
            || value(&inspection.request)? != value(&intent.input.model)?
        {
            return Err("模型预检与本次配置不一致。".into());
        }
        self.prepare_benchmark_record(workspace, id, intent, &value(inspection)?)
    }

    pub(crate) fn prepare_benchmark_record(
        &self,
        workspace: &str,
        id: &str,
        intent: &Intent,
        profile: &Value,
    ) -> AppResult<Prepared> {
        if profile["request"] != value(&intent.input.model)? {
            return Err("预检请求身份不一致。".into());
        }
        let version = intent.input.dataset_version;
        let modern = intent.case.scoring.version == 2;
        let dataset_id = intent.config["datasetId"]
            .as_str()
            .ok_or("Missing dataset identity")?;
        let sandbox_observation = profile.get("sandbox").cloned();
        let settings = intent.input.model.settings();
        let system = intent.manifest["prompt_template"]["system"]
            .as_str()
            .ok_or("测试集模板缺失。")?;
        let mut prompt = intent.case.input.prompt.clone();
        if modern {
            for source in &intent.case.fixture {
                prompt.push_str(&format!("\n\n资料 {}：{}\n", source.id, source.title));
                for (i, line) in source.lines.iter().enumerate() {
                    prompt.push_str(&format!("{}: {}\n", i + 1, line));
                }
            }
        }
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: prompt,
            },
        ];
        let mut request = crate::ollama::chat_request(&settings, &messages);
        if modern {
            if intent.case.scoring.id == "schema_semantic" {
                request["format"] = intent
                    .case
                    .input
                    .response_schema
                    .clone()
                    .ok_or("缺少JSON Schema。")?;
            }
            if !intent.case.input.tools.is_empty() {
                request["tools"] = json!(intent.case.input.tools);
            }
        }
        let mut manifest = json!({"schemaVersion":1,"purpose":"benchmark","mode":intent.config["mode"],"datasetId":dataset_id,"datasetVersion":version,"datasetHash":intent.input.dataset_hash,"caseId":intent.case.case_id,"caseHash":intent.case.content_hash,"reviewStatus":intent.manifest["review_status"],"modelDigest":intent.input.model.expected_digest,"knowledgeRefs":[],"messageRefs":[],"historyUsed":false,"retrievalUsed":false,"systemHash":dataset::hash(system.as_bytes()),"promptTemplateVersion":version,"parameterStatus":"requested_not_verified_as_effective"});
        manifest["benchmarkRunId"] = json!(id);
        if intent.input.personalized_dataset.is_some() {
            manifest["batchId"] = json!(intent.input.launch_key.split(':').next());
        }
        if let Some(cloud_request) = profile.get("cloudRequest") {
            request = cloud_request.clone();
            manifest["provider"] = profile["provider"].clone();
            manifest["parameterStatus"] =
                json!("cloud_wire_request_only_local_context_settings_not_applied");
        }
        if modern {
            manifest["fixtureRefs"]=json!(intent.case.fixture.iter().map(|s|json!({"sourceId":s.id,"lineCount":s.lines.len(),"contentHash":dataset::hash(serde_json::to_string(s).unwrap().as_bytes())})).collect::<Vec<_>>());
        }
        if let Some((batch, _)) = intent.input.launch_key.rsplit_once(':') {
            let owned:bool=self.connection()?.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_batches WHERE workspace_id=?1 AND id=?2)",params![workspace,batch],|r|r.get(0)).map_err(error)?;
            if owned {
                manifest["batchId"] = json!(batch);
                manifest["benchmarkScope"] =
                    json!(if intent.input.personalized_dataset.is_some() {
                        "personalized"
                    } else {
                        "standard"
                    });
            }
        }
        let run = RunSummary {
            id: new_id(),
            workspace_id: workspace.into(),
            conversation_id: new_id(),
            assistant_message_id: new_id(),
            model_id: settings.model.clone(),
            status: "running".into(),
            started_at: now(),
            finished_at: None,
            error_message: None,
        };
        let result = Summary {
            id: id.into(),
            workspace_id: workspace.into(),
            status: "running".into(),
            development_only: intent.input.personalized_dataset.is_none(),
            case_run_id: new_id(),
            attempt_id: new_id(),
            execution_run_id: run.id.clone(),
        };
        let mut c = self.connection()?;
        dataset::require_workspace(&c, workspace)?;
        let tx = c.transaction().map_err(error)?;
        let hardware = if modern && profile.get("cloudRequest").is_none() {
            super::telemetry::hardware_snapshot()
        } else {
            json!({"availability":"unavailable","reason":"not_collected_for_legacy_or_remote"})
        };
        tx.execute("INSERT INTO benchmark_runs(id,workspace_id,dataset_id,dataset_version,launch_key,status,config_snapshot_json,config_hash,hardware_snapshot_json,app_version,source_revision,evaluator_version,created_at,started_at) VALUES(?1,?2,?12,?3,?4,'running',?5,?6,?7,?8,?9,?10,?11,?11)",params![id,workspace,version,intent.input.launch_key,encode(&intent.config)?,intent.config_hash,hardware.to_string(),env!("CARGO_PKG_VERSION"),format!("{}:{}",if version==1{"slice3-core-sha256"}else{"benchmark-scoring-sha256"},source_hash()),if !modern {"slice3-exact-v1"}else{super::evaluation::ENGINE_VERSION},run.started_at,dataset_id]).map_err(error)?;
        let profile = encode(profile)?;
        tx.execute(
            "INSERT INTO benchmark_run_models VALUES(?1,0,?2,?3,?4)",
            params![id, workspace, profile, dataset::hash(profile.as_bytes())],
        )
        .map_err(error)?;
        tx.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose) VALUES(?1,?2,?3,?4,?4,'benchmark')",params![run.conversation_id,workspace,format!("评测开发检查 · {}",intent.case.case_id),run.started_at]).map_err(error)?;
        let user_id = new_id();
        insert_execution(
            &tx,
            &run,
            &settings,
            &request,
            &manifest,
            &user_id,
            &intent.case.input.prompt,
        )?;
        tx.execute(
            "INSERT INTO benchmark_case_runs VALUES(?1,?2,?3,0,?6,?4,?5,1,'single','running')",
            params![
                result.case_run_id,
                workspace,
                id,
                version,
                intent.case.case_id,
                dataset_id
            ],
        )
        .map_err(error)?;
        tx.execute("INSERT INTO benchmark_attempts(id,workspace_id,case_run_id,attempt_no,execution_run_id,status,request_disposition,started_at) VALUES(?1,?2,?3,1,?4,'running','not_sent',?5)",params![result.attempt_id,workspace,result.case_run_id,run.id,run.started_at]).map_err(error)?;
        if let Some((batch, ordinal)) = intent.input.launch_key.rsplit_once(':')
            && let Ok(ordinal) = ordinal.parse::<u32>()
        {
            tx.execute("UPDATE benchmark_batch_items SET run_id=?1 WHERE batch_id=?2 AND ordinal=?3 AND status='running' AND batch_id IN (SELECT id FROM benchmark_batches WHERE workspace_id=?4 AND status='running')",params![id,batch,ordinal,workspace]).map_err(error)?;
        }
        event(
            &tx,
            &run.id,
            "request_received",
            json!({"userMessageId":user_id,"purpose":"benchmark"}),
        )?;
        event(&tx, &run.id, "context_prepared", manifest.clone())?;
        if modern && let Some(observed) = sandbox_observation {
            event(&tx, &run.id, "python_environment_checked", observed)?;
        }
        benchmark_event(
            &tx,
            id,
            workspace,
            &result.attempt_id,
            "development_check_started",
            json!({"caseId":intent.case.case_id,"caseCount":1,"configHash":intent.config_hash}),
        )?;
        benchmark_event(
            &tx,
            id,
            workspace,
            &result.attempt_id,
            "model_preflight_completed",
            profile.parse::<Value>().map_err(json_error)?,
        )?;
        tx.commit().map_err(error)?;
        Ok(Prepared {
            summary: result,
            execution: PreparedRun {
                run,
                request,
                manifest,
            },
        })
    }

    pub fn benchmark_detail(&self, workspace: &str, id: &str) -> AppResult<Detail> {
        let (summary, config, model, output, attempt, evaluations, events) = {
            let c = self.connection()?;
            let summary = summary(&c, workspace, id)?;
            let config: String = c
                .query_row(
                    "SELECT config_snapshot_json FROM benchmark_runs WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .map_err(error)?;
            let model:String=c.query_row("SELECT profile_snapshot_json FROM benchmark_run_models WHERE run_id=?1 AND model_slot=0",params![id],|r|r.get(0)).map_err(error)?;
            let output:String=c.query_row("SELECT m.content FROM messages m JOIN execution_runs r ON r.assistant_message_id=m.id WHERE r.id=?1",params![summary.execution_run_id],|r|r.get(0)).map_err(error)?;
            let attempt=json_rows(&c,"SELECT json_object('id',id,'status',status,'requestDisposition',request_disposition,'startedAt',started_at,'finishedAt',finished_at,'errorKind',error_kind) FROM benchmark_attempts WHERE id=?1",&summary.attempt_id)?.pop().ok_or("评测尝试记录缺失。")?;
            let evaluations = json_rows(
                &c,
                "SELECT json_object('id',id,'evaluatorId',evaluator_id,'evaluatorVersion',evaluator_version,'scoringConfigHash',scoring_config_hash,'outputHash',output_hash,'metrics',json(metrics_json),'details',json(details_json),'status',status) FROM benchmark_evaluations WHERE attempt_id=?1 ORDER BY created_at,id",
                &summary.attempt_id,
            )?;
            let events = json_rows(
                &c,
                "SELECT json_object('seq',seq,'kind',kind,'occurredAt',occurred_at,'payload',json(payload_json)) FROM benchmark_events WHERE run_id=?1 ORDER BY seq",
                id,
            )?;
            (
                summary,
                serde_json::from_str::<Value>(&config).map_err(json_error)?,
                serde_json::from_str::<Value>(&model).map_err(json_error)?,
                output,
                attempt,
                evaluations,
                events,
            )
        };
        let case_id = config["input"]["caseId"]
            .as_str()
            .ok_or("评测配置缺少题目。")?;
        let version = config["datasetVersion"]
            .as_u64()
            .ok_or("缺少测试集版本。")? as u32;
        let dataset_id = config["datasetId"].as_str().unwrap_or("cortex-core-60");
        let case = self.benchmark_case(workspace, dataset_id, version, case_id)?;
        let modern = case.scoring.version == 2;
        let execution = self.recorded_run_detail(
            workspace,
            &summary.execution_run_id,
            ExecutionPurpose::Benchmark,
        )?;
        let hardware: String = self
            .connection()?
            .query_row(
                "SELECT hardware_snapshot_json FROM benchmark_runs WHERE id=?1 AND workspace_id=?2",
                params![id, workspace],
                |r| r.get(0),
            )
            .map_err(error)?;
        let resources = self.benchmark_resource_samples(workspace, &summary.attempt_id)?;
        Ok(Detail {
            summary,
            config,
            model,
            case,
            execution,
            output,
            attempt,
            evaluations,
            events,
            hardware: serde_json::from_str(&hardware).map_err(json_error)?,
            resources,
            statistics: if modern {
                Some(self.benchmark_statistics(workspace, id)?)
            } else {
                None
            },
        })
    }

    pub fn benchmark_finalize_error(
        &self,
        workspace: &str,
        id: &str,
        message: &str,
    ) -> AppResult<()> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let summary = summary(&tx, workspace, id)?;
        let status: String = tx
            .query_row(
                "SELECT status FROM execution_runs WHERE id=?1",
                params![summary.execution_run_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if status != "running" {
            tx.execute("UPDATE benchmark_attempts SET status=?1,request_disposition=CASE WHEN ?1='completed' THEN 'finished' ELSE request_disposition END,finished_at=?2,error_kind='result_save_failed' WHERE id=?3 AND status='running'",params![status,now(),summary.attempt_id]).map_err(error)?;
            tx.execute(
                "UPDATE benchmark_case_runs SET status=?1 WHERE id=?2 AND status='running'",
                params![status, summary.case_run_id],
            )
            .map_err(error)?;
        }
        tx.execute("UPDATE benchmark_runs SET status='failed',finished_at=?1 WHERE id=?2 AND status='running'",params![now(),id]).map_err(error)?;
        benchmark_event(
            &tx,
            id,
            workspace,
            &summary.attempt_id,
            "result_save_failed",
            json!({"message":message,"executionStatus":status,"automaticRetry":false}),
        )?;
        tx.commit().map_err(error)
    }

    pub fn finish_benchmark(&self, workspace: &str, id: &str) -> AppResult<()> {
        if self.benchmark_modern(workspace, id)? {
            return self.finish_benchmark_v2(workspace, id, None);
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        let summary = summary(&tx, workspace, id)?;
        let (status,output,finished_reason):(String,String,Option<String>)=tx.query_row("SELECT r.status,m.content,(SELECT json_extract(payload_json,'$.metrics.finishReason') FROM execution_events WHERE run_id=r.id AND kind='response_completed' ORDER BY seq DESC LIMIT 1) FROM execution_runs r JOIN messages m ON m.id=r.assistant_message_id WHERE r.id=?1",params![summary.execution_run_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)?;
        if status == "running" {
            return Err("回答尚未保存为终态，不能评分或完成实验。".into());
        }
        let attempt_status: String = tx
            .query_row(
                "SELECT status FROM benchmark_attempts WHERE id=?1",
                params![summary.attempt_id],
                |r| r.get(0),
            )
            .map_err(error)?;
        if attempt_status != "running" {
            return Ok(());
        }
        if status == "completed" {
            let config: String = tx
                .query_row(
                    "SELECT config_snapshot_json FROM benchmark_runs WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .map_err(error)?;
            let config: Value = serde_json::from_str(&config).map_err(json_error)?;
            let case_id = config["input"]["caseId"].as_str().ok_or("缺少题目。")?;
            let expected:String=tx.query_row("SELECT expected_json FROM benchmark_cases WHERE dataset_id='cortex-core-60' AND dataset_version=1 AND case_id=?1",params![case_id],|r|r.get(0)).map_err(error)?;
            let expected: Expected = serde_json::from_str(&expected).map_err(json_error)?;
            let (metrics, details, evaluation_status) = if finished_reason.as_deref()
                == Some("length")
            {
                (
                    json!({"correctness":null}),
                    json!({"reason":"output_limit","developmentOnly":true}),
                    "needs_review",
                )
            } else {
                let trimmed = output.trim();
                let (correct, format_valid) = match expected {
                    Expected::Aliases { answers } => (answers.iter().any(|a| a == trimmed), true),
                    Expected::Integer { answer } => {
                        let digits = trimmed.strip_prefix('-').unwrap_or(trimmed);
                        let parsed = (!digits.is_empty()
                            && digits.bytes().all(|c| c.is_ascii_digit()))
                        .then(|| trimmed.parse::<i64>().ok())
                        .flatten();
                        (parsed == Some(answer), parsed.is_some())
                    }
                    _ => return Err("本开发切片不执行该题评分。".into()),
                };
                (
                    json!({"correctness":if correct {1} else {0},"formatValid":format_valid}),
                    json!({"developmentOnly":true,"reviewStatus":"needs_human_review","normalization":"trim_unicode_whitespace_exact_case"}),
                    "completed",
                )
            };
            let rule = &config["scoringRule"];
            let evaluator = rule["id"].as_str().ok_or("评分规则缺失。")?;
            tx.execute(
                "INSERT INTO benchmark_evaluations VALUES(?1,?2,?3,?4,'1',?5,?6,?7,?8,?9,?10)",
                params![
                    new_id(),
                    workspace,
                    summary.attempt_id,
                    evaluator,
                    dataset::hash(encode(rule)?.as_bytes()),
                    dataset::hash(output.as_bytes()),
                    metrics.to_string(),
                    details.to_string(),
                    evaluation_status,
                    now()
                ],
            )
            .map_err(error)?;
            benchmark_event(
                &tx,
                id,
                workspace,
                &summary.attempt_id,
                "evaluation_saved",
                json!({"evaluatorId":evaluator,"status":evaluation_status}),
            )?;
        }
        tx.execute("UPDATE benchmark_attempts SET status=?1,request_disposition=CASE WHEN ?1='completed' THEN 'finished' ELSE request_disposition END,finished_at=?2,error_kind=CASE WHEN ?1='completed' THEN NULL ELSE ?1 END WHERE id=?3 AND status='running'",params![status,now(),summary.attempt_id]).map_err(error)?;
        tx.execute(
            "UPDATE benchmark_case_runs SET status=?1 WHERE id=?2 AND status='running'",
            params![status, summary.case_run_id],
        )
        .map_err(error)?;
        tx.execute(
            "UPDATE benchmark_runs SET status=?1,finished_at=?2 WHERE id=?3 AND status='running'",
            params![status, now(), id],
        )
        .map_err(error)?;
        benchmark_event(
            &tx,
            id,
            workspace,
            &summary.attempt_id,
            "development_check_finished",
            json!({"status":status}),
        )?;
        tx.commit().map_err(error)
    }
}

pub(super) fn summary(c: &Connection, workspace: &str, id: &str) -> AppResult<Summary> {
    c.query_row("SELECT b.id,b.workspace_id,b.status,c.id,a.id,a.execution_run_id,b.dataset_id FROM benchmark_runs b JOIN benchmark_case_runs c ON c.run_id=b.id JOIN benchmark_attempts a ON a.case_run_id=c.id WHERE b.workspace_id=?1 AND b.id=?2 AND ((b.dataset_id='cortex-core-60' AND ((b.dataset_version=1 AND json_extract(b.config_snapshot_json,'$.mode')='development_single_case_v1') OR (b.dataset_version=2 AND json_extract(b.config_snapshot_json,'$.mode')='development_single_case_v2'))) OR (b.dataset_id LIKE 'personal-%' AND json_extract(b.config_snapshot_json,'$.mode')='personalized_single_case_v1')) AND a.attempt_no=1",params![workspace,id],|r|Ok(Summary{id:r.get(0)?,workspace_id:r.get(1)?,status:r.get(2)?,development_only:r.get::<_,String>(6)?=="cortex-core-60",case_run_id:r.get(3)?,attempt_id:r.get(4)?,execution_run_id:r.get(5)?})).optional().map_err(error)?.ok_or_else(||"所选评测不存在或不属于当前工作区。".into())
}
fn json_rows(c: &Connection, sql: &str, id: &str) -> AppResult<Vec<Value>> {
    let mut q = c.prepare(sql).map_err(error)?;
    let rows = q
        .query_map(params![id], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    rows.iter()
        .map(|r| serde_json::from_str(r).map_err(json_error))
        .collect()
}

#[cfg(test)]
mod tests;
