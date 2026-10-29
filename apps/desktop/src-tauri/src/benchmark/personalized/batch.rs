use super::{
    cloud_run,
    plan::{Binding, Price},
};
use crate::{
    benchmark::{
        dataset,
        preflight::{ModelRequest, Parameters},
        repository::StartInput,
        runner_v2,
    },
    cloud::CloudState,
    commands::AppState,
    db::{Database, error, json_error},
    models::{AppResult, new_id, now},
    tasks::TaskKind,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tauri::State;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelChoice {
    pub kind: String,
    pub local: Option<ModelRequest>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchInput {
    pub dataset_id: String,
    pub dataset_version: u32,
    pub dataset_hash: String,
    pub models: Vec<ModelChoice>,
    pub max_output_tokens: u32,
    pub price: Option<Price>,
    pub python_image: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub case_ids: Vec<String>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub repetitions: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub release_models: Vec<String>,
}
fn one() -> u32 {
    1
}
fn is_one(n: &u32) -> bool {
    *n == 1
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPlan {
    pub input: BatchInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,
    pub profile: Value,
    pub cases: Vec<String>,
    pub cloud_requests: Vec<Value>,
    pub reserved_cost: Option<f64>,
    pub max_requests: usize,
}
impl Database {
    pub fn plan_personalized_batch(
        &self,
        workspace: &str,
        connection: &crate::cloud::Connection,
        input: BatchInput,
    ) -> AppResult<Value> {
        if input.dataset_id == "cortex-core-60" {
            return Err("请通过标准评测入口创建固定题库任务。".into());
        }
        self.plan_batch(workspace, Some(connection), input)
    }
    pub fn plan_standard_batch(&self, workspace: &str, input: BatchInput) -> AppResult<Value> {
        if input.dataset_id != "cortex-core-60"
            || input.dataset_version != 2
            || input.models.iter().any(|m| m.kind != "local")
            || input.price.is_some()
        {
            return Err("标准批量评测使用 Core 60 v2 和本地模型，无需云端连接。".into());
        }
        self.plan_batch(workspace, None, input)
    }
    fn plan_batch(
        &self,
        workspace: &str,
        connection: Option<&crate::cloud::Connection>,
        input: BatchInput,
    ) -> AppResult<Value> {
        let standard = input.dataset_id == "cortex-core-60";
        let dataset = if standard {
            self.benchmark_datasets(workspace)?
                .into_iter()
                .find(|d| d.id == input.dataset_id && d.version == input.dataset_version)
                .ok_or("固定题库版本不存在。")?
        } else {
            self.personalized_dataset(workspace, &input.dataset_id, input.dataset_version)?
        };
        if dataset.content_hash != input.dataset_hash
            || input.models.is_empty()
            || input.models.len() > 6
            || !(1..=5).contains(&input.repetitions)
            || !(64..=2048).contains(&input.max_output_tokens)
            || input.models.iter().any(|m| {
                !matches!(m.kind.as_str(), "local" | "cloud")
                    || (m.kind == "local") != m.local.is_some()
            })
            || input.models.iter().filter(|m| m.kind == "cloud").count() > 1
            || input
                .python_image
                .as_ref()
                .is_some_and(|s| !crate::benchmark::python::valid_image(s))
        {
            return Err("请检查题库、模型、输出上限或 Docker 镜像标识。".into());
        }
        let all =
            self.benchmark_dataset_cases(workspace, &input.dataset_id, input.dataset_version)?;
        if input.case_ids.len() > all.len()
            || input
                .case_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != input.case_ids.len()
            || input
                .case_ids
                .iter()
                .any(|id| !all.iter().any(|c| c.case_id == *id))
        {
            return Err("题目选择无效、重复或不属于固定题库。".into());
        }
        let cases = if input.case_ids.is_empty() {
            all
        } else {
            input
                .case_ids
                .iter()
                .map(|id| {
                    all.iter()
                        .find(|c| c.case_id == *id)
                        .cloned()
                        .ok_or("Selected case missing".to_string())
                })
                .collect::<AppResult<Vec<_>>>()?
        };
        for m in &input.models {
            if let Some(r) = &m.local {
                r.validate()?;
                if r.expected_digest.is_none() {
                    return Err("本地模型需要实际内容指纹，请刷新模型列表。".into());
                }
            }
        }
        if input
            .release_models
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != input.release_models.len()
            || input.release_models.iter().any(|name| {
                !input
                    .models
                    .iter()
                    .any(|m| m.local.as_ref().is_some_and(|r| r.model == *name))
            })
        {
            return Err("只能释放本次明确选择并授权的本地模型。".into());
        }
        let mut unique = std::collections::BTreeSet::new();
        for m in &input.models {
            let key = if let Some(l) = &m.local {
                format!("local:{}", l.model)
            } else {
                "cloud".into()
            };
            if !unique.insert(key) {
                return Err("请移除重复模型。".into());
            }
        }
        let binding = connection.map(Binding::from).transpose()?;
        let mut cloud_requests = vec![];
        let mut reserve = 0u64;
        if input.models.iter().any(|m| m.kind == "cloud") {
            let binding = binding.as_ref().ok_or("云端模型需要有效连接。")?;
            for case in &cases {
                let request = cloud_run::request(
                    case,
                    dataset.manifest["prompt_template"]["system"]
                        .as_str()
                        .ok_or("模板缺失。")?,
                    &binding.model,
                    input.max_output_tokens,
                );
                completion_check(binding.provider, &request)?;
                let (_, wire) = crate::cloud::completion::wire(binding.provider, &request)?;
                reserve += wire.to_string().len() as u64 + 1024;
                cloud_requests.push(json!({"caseId":case.case_id,"request":request}));
            }
        }
        let reserved_cost = if cloud_requests.is_empty() {
            None
        } else {
            Some(
                input
                    .price
                    .as_ref()
                    .ok_or("云端评测需提供单价与总预算。")?
                    .estimate(
                        reserve * input.repetitions as u64,
                        cloud_requests.len() as u64
                            * input.repetitions as u64
                            * input.max_output_tokens as u64,
                    )?,
            )
        };
        let profile = if standard {
            Value::Null
        } else {
            serde_json::to_value(
                self.benchmark_profile(workspace)?
                    .ok_or("请先保存使用偏好。")?,
            )
            .map_err(json_error)?
        };
        let scheduled: Vec<_> = (0..input.repetitions)
            .flat_map(|_| cases.iter().map(|c| c.case_id.clone()))
            .collect();
        let plan = BatchPlan {
            max_requests: scheduled.len() * input.models.len(),
            input,
            binding,
            profile,
            cases: scheduled,
            cloud_requests,
            reserved_cost,
        };
        let raw = serde_json::to_value(&plan).map_err(json_error)?.to_string();
        let hash = dataset::hash(raw.as_bytes());
        let id = new_id();
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(error)?;
        tx.execute(
            "INSERT INTO benchmark_batches VALUES(?1,?2,'planned',?3,?4,?5,NULL)",
            params![id, workspace, raw, hash, now()],
        )
        .map_err(error)?;
        for (mi, _) in plan.input.models.iter().enumerate() {
            for (ci, case) in plan.cases.iter().enumerate() {
                tx.execute(
                    "INSERT INTO benchmark_batch_items VALUES(?1,?2,?3,?4,'queued',NULL,NULL)",
                    params![id, (mi * plan.cases.len() + ci) as u32, mi as u32, case],
                )
                .map_err(error)?;
            }
        }
        tx.commit().map_err(error)?;
        drop(c);
        self.personalized_batch(workspace, &id)
    }
    pub fn personalized_batch(&self, workspace: &str, id: &str) -> AppResult<Value> {
        let c = self.connection()?;
        let mut batch=c.query_row("SELECT status,plan_json,plan_hash,created_at FROM benchmark_batches WHERE workspace_id=?1 AND id=?2",params![workspace,id],|r|{let raw:String=r.get(1)?;Ok(json!({"id":id,"status":r.get::<_,String>(0)?,"plan":serde_json::from_str::<Value>(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?,"planHash":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?}))}).map_err(|_|"评测批次不存在或不属于此工作区。".to_string())?;
        let mut q=c.prepare("SELECT ordinal,model_index,case_id,status,run_id,error FROM benchmark_batch_items WHERE batch_id=?1 ORDER BY ordinal").map_err(error)?;
        let mut rows=q.query_map([id],|r|Ok(json!({"ordinal":r.get::<_,u32>(0)?,"modelIndex":r.get::<_,u32>(1)?,"caseId":r.get::<_,String>(2)?,"status":r.get::<_,String>(3)?,"runId":r.get::<_,Option<String>>(4)?,"error":r.get::<_,Option<String>>(5)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        let per_model = batch["plan"]["cases"]
            .as_array()
            .ok_or("固定题目计划缺失。")?
            .len()
            .max(1) as u64;
        let repetitions = batch["plan"]["input"]["repetitions"]
            .as_u64()
            .unwrap_or(1)
            .max(1);
        let distinct = (per_model / repetitions).max(1);
        for row in &mut rows {
            row["repetition"] =
                json!(row["ordinal"].as_u64().unwrap_or(0) % per_model / distinct + 1);
        }
        batch["items"] = json!(rows);
        Ok(batch)
    }
    pub fn require_batch_scope(
        &self,
        workspace: &str,
        id: &str,
        standard: bool,
    ) -> AppResult<Value> {
        let batch = self.personalized_batch(workspace, id)?;
        if (batch["plan"]["input"]["datasetId"] == "cortex-core-60") != standard {
            return Err("所选任务不属于此评测类型。".into());
        }
        Ok(batch)
    }
    pub fn personalized_batch_history(&self, workspace: &str) -> AppResult<Vec<Value>> {
        self.batch_history(workspace, false)
    }
    pub fn standard_batch_history(&self, workspace: &str) -> AppResult<Vec<Value>> {
        self.batch_history(workspace, true)
    }
    fn batch_history(&self, workspace: &str, standard: bool) -> AppResult<Vec<Value>> {
        let c = self.connection()?;
        dataset::require_workspace(&c, workspace)?;
        let mut q=c.prepare("SELECT id,status,created_at,json_extract(plan_json,'$.input.datasetId'),json_extract(plan_json,'$.input.datasetVersion') FROM benchmark_batches WHERE workspace_id=?1 AND (json_extract(plan_json,'$.input.datasetId')='cortex-core-60')=?2 ORDER BY created_at DESC LIMIT 100").map_err(error)?;
        q.query_map(params![workspace,standard],|r|Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?,"datasetId":r.get::<_,String>(3)?,"datasetVersion":r.get::<_,u32>(4)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)
    }
    pub fn begin_personalized_batch(
        &self,
        workspace: &str,
        id: &str,
        hash: &str,
        connection: &crate::cloud::Connection,
    ) -> AppResult<BatchPlan> {
        self.require_batch_scope(workspace, id, false)?;
        self.begin_batch(workspace, id, hash, Some(connection))
    }
    pub fn begin_standard_batch(
        &self,
        workspace: &str,
        id: &str,
        hash: &str,
    ) -> AppResult<BatchPlan> {
        self.require_batch_scope(workspace, id, true)?;
        self.begin_batch(workspace, id, hash, None)
    }
    fn begin_batch(
        &self,
        workspace: &str,
        id: &str,
        hash: &str,
        connection: Option<&crate::cloud::Connection>,
    ) -> AppResult<BatchPlan> {
        let batch = self.personalized_batch(workspace, id)?;
        let plan: BatchPlan = serde_json::from_value(batch["plan"].clone()).map_err(json_error)?;
        if batch["planHash"] != hash
            || dataset::hash(batch["plan"].to_string().as_bytes()) != hash
            || !match (&plan.binding, connection) {
                (Some(binding), Some(c)) => binding.matches(c),
                (None, None) => {
                    plan.input.dataset_id == "cortex-core-60"
                        && plan.input.models.iter().all(|m| m.kind == "local")
                }
                _ => false,
            }
        {
            return Err("评测计划或连接已改变，请重新建立计划。".into());
        }
        if self.connection()?.execute("UPDATE benchmark_batches SET status='running',finished_at=NULL WHERE workspace_id=?1 AND id=?2 AND status IN ('planned','interrupted','cancelled','failed') AND EXISTS(SELECT 1 FROM benchmark_batch_items WHERE batch_id=?2 AND (status='queued' OR (status!='finished' AND run_id IS NOT NULL AND EXISTS(SELECT 1 FROM benchmark_attempts a JOIN execution_runs r ON r.id=a.execution_run_id WHERE a.case_run_id IN (SELECT id FROM benchmark_case_runs WHERE run_id=benchmark_batch_items.run_id) AND r.status='completed'))))",params![workspace,id]).map_err(error)?!=1{return Err("批次正在运行，或没有可继续、可补评分的题目；已发送题目不会重跑。".into());}
        Ok(plan)
    }
    pub fn set_batch_item(
        &self,
        id: &str,
        ordinal: u32,
        status: &str,
        run: Option<&str>,
        message: Option<&str>,
    ) -> AppResult<()> {
        self.connection()?.execute("UPDATE benchmark_batch_items SET status=?1,run_id=COALESCE(?2,run_id),error=?3 WHERE batch_id=?4 AND ordinal=?5",params![status,run,message,id,ordinal]).map_err(error)?;
        Ok(())
    }
    pub fn finish_batch(&self, workspace: &str, id: &str, status: &str) -> AppResult<()> {
        self.connection()?.execute("UPDATE benchmark_batches SET status=?1,finished_at=?2 WHERE workspace_id=?3 AND id=?4 AND status IN ('running','stopping')",params![status,now(),workspace,id]).map_err(error)?;
        Ok(())
    }
}
fn completion_check(
    provider: crate::cloud::Provider,
    input: &crate::cloud::completion::CompletionInput,
) -> AppResult<()> {
    crate::cloud::completion::wire(provider, input).map(|_| ())
}
pub async fn execute(
    state: AppState,
    cloud: CloudState,
    workspace: String,
    id: String,
    plan: BatchPlan,
    lease: Arc<crate::tasks::Lease>,
) -> AppResult<()> {
    let db = state.database.clone()?;
    let batch = db.personalized_batch(&workspace, &id)?;
    let mut previous: Option<(ModelRequest, String)> = None;
    for item in batch["items"].as_array().ok_or("批次明细缺失。")? {
        if *lease.receiver().borrow() {
            db.finish_batch(&workspace, &id, "cancelled")?;
            return Ok(());
        }
        if plan.binding.is_some() {
            cloud.guard()?;
        }
        if item["status"] != "queued" {
            if item["status"] != "finished"
                && let Some(run) = item["runId"].as_str()
            {
                let detail = db.benchmark_detail(&workspace, run)?;
                if detail.execution.run.status == "completed" {
                    if db.benchmark_score_pending(&workspace, run)? {
                        let task = run.to_string();
                        let scope = workspace.clone();
                        let cancel = lease.receiver();
                        crate::commands::blocking(db.clone(), move |db| {
                            db.finish_benchmark_v2(&scope, &task, Some(cancel))
                        })
                        .await?;
                    }
                    if !db.benchmark_detail(&workspace, run)?.evaluations.is_empty() {
                        db.set_batch_item(
                            &id,
                            item["ordinal"].as_u64().ok_or("任务序号缺失。")? as u32,
                            "finished",
                            Some(run),
                            None,
                        )?;
                    }
                }
            }
            continue;
        }
        let index = item["modelIndex"].as_u64().ok_or("模型序号缺失。")? as usize;
        let ordinal = item["ordinal"].as_u64().unwrap() as u32;
        let case_id = item["caseId"].as_str().ok_or("题目缺失。")?;
        let case = db.benchmark_case(
            &workspace,
            &plan.input.dataset_id,
            plan.input.dataset_version,
            case_id,
        )?;
        let choice = &plan.input.models[index];
        if previous.as_ref().is_some_and(|(old, _)| {
            choice
                .local
                .as_ref()
                .is_none_or(|next| next.model != old.model)
        }) && let Some((old, run)) = previous.take()
            && plan.input.release_models.contains(&old.model)
        {
            db.batch_model_event(
                &workspace,
                &run,
                "model_release_started",
                json!({"model":old.model,"authorization":"frozen_batch_plan"}),
            )?;
            let mut cancel = lease.receiver();
            let observed = tokio::select! {biased;_=cancel.changed()=>Err("释放请求已取消，实际状态需重新检查。".into()),r=state.provider.as_ref().map_err(Clone::clone)?.release_benchmark_model(&old)=>r};
            db.batch_model_event(
                &workspace,
                &run,
                "model_release_finished",
                match observed {
                    Ok(value) => value,
                    Err(message) => {
                        json!({"model":old.model,"status":"unconfirmed","error":message})
                    }
                },
            )?;
            if *cancel.borrow() {
                db.finish_batch(&workspace, &id, "cancelled")?;
                return Ok(());
            }
        }
        let requirements = case
            .requirements
            .iter()
            .filter(|v| *v != "python_sandbox")
            .cloned()
            .collect();
        let model = if let Some(mut local) = choice.local.clone() {
            local.requirements = requirements;
            local
        } else {
            ModelRequest {
                model: plan
                    .binding
                    .as_ref()
                    .ok_or("Missing cloud binding")?
                    .model
                    .clone(),
                expected_digest: None,
                parameters: Parameters {
                    num_ctx: 4096,
                    num_predict: plan.input.max_output_tokens,
                    temperature: 0.,
                    timeout_seconds: 120,
                },
                requirements,
            }
        };
        let local_observation = choice.local.as_ref().map(|_| model.clone());
        let input = StartInput {
            launch_key: format!("{id}:{ordinal}"),
            development_only: plan.input.dataset_id == "cortex-core-60",
            dataset_hash: plan.input.dataset_hash.clone(),
            case_id: case_id.into(),
            model,
            dataset_version: plan.input.dataset_version,
            python_image: plan.input.python_image.clone(),
            personalized_dataset: (plan.input.dataset_id != "cortex-core-60")
                .then(|| plan.input.dataset_id.clone()),
        };
        db.set_batch_item(&id, ordinal, "running", None, None)?;
        let result = if choice.kind == "local" {
            runner_v2::run_batch_case(state.clone(), workspace.clone(), input, lease.clone()).await
        } else {
            cloud_run::run(
                &state,
                &cloud,
                &workspace,
                input,
                plan.binding.as_ref().ok_or("Missing cloud binding")?,
                lease.clone(),
            )
            .await
        };
        match result {
            Ok(run) => {
                let saved = db.benchmark_detail(&workspace, &run.id)?;
                if saved.evaluations.is_empty() {
                    db.set_batch_item(
                        &id,
                        ordinal,
                        "error",
                        Some(&run.id),
                        Some("输出已保留，但评分尚未保存；后续请求已停止，请重新读取后恢复。"),
                    )?;
                    db.finish_batch(&workspace, &id, "failed")?;
                    return Ok(());
                }
                if let Some(request) = local_observation {
                    previous = Some((request, run.id.clone()));
                }
                db.set_batch_item(&id, ordinal, "finished", Some(&run.id), None)?;
            }
            Err(e) => {
                db.set_batch_item(
                    &id,
                    ordinal,
                    if *lease.receiver().borrow() {
                        "cancelled"
                    } else {
                        "error"
                    },
                    None,
                    Some(&e),
                )?;
                if choice.kind == "cloud" {
                    db.finish_batch(&workspace, &id, "failed")?;
                    return Ok(());
                }
            }
        }
    }
    db.finish_batch(
        &workspace,
        &id,
        if *lease.receiver().borrow() {
            "cancelled"
        } else {
            "completed"
        },
    )
}
#[tauri::command]
pub fn plan_personalized_batch(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    input: BatchInput,
) -> AppResult<Value> {
    let c = cloud.guard()?;
    state
        .database
        .as_ref()
        .map_err(Clone::clone)?
        .plan_personalized_batch(&workspace_id, &c, input)
}
#[tauri::command]
pub fn get_personalized_batch(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
) -> AppResult<Value> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .personalized_report(&workspace_id, &batch_id)
    })
}
#[tauri::command]
pub fn list_personalized_batches(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
) -> AppResult<Vec<Value>> {
    cloud.with_access(|| {
        state
            .database
            .as_ref()
            .map_err(Clone::clone)?
            .personalized_batch_history(&workspace_id)
    })
}
#[tauri::command]
pub async fn start_personalized_batch(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    approved_plan_hash: String,
) -> AppResult<Value> {
    let connection = cloud.guard()?;
    let lease = Arc::new(state.claim_task(&batch_id, &workspace_id, TaskKind::Benchmark)?);
    let db = state.database.clone()?;
    let plan =
        db.begin_personalized_batch(&workspace_id, &batch_id, &approved_plan_hash, &connection)?;
    let initial = db.personalized_batch(&workspace_id, &batch_id)?;
    let state = state.inner().clone();
    let cloud = cloud.inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = execute(
            state,
            cloud,
            workspace_id.clone(),
            batch_id.clone(),
            plan,
            lease,
        )
        .await;
        if result.is_err() {
            let _ = db.finish_batch(&workspace_id, &batch_id, "failed");
        }
    });
    Ok(initial)
}
