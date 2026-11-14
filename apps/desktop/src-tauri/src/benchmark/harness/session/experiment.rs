//! Serial comparison of approved tasks in isolated copies.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
mod report;
const PURPOSE: &str = "harness_experiment";
const MAX_RUNS: usize = 24;
const KINDS: [Kind; 3] = [Kind::Codex, Kind::Claude, Kind::Aider];

impl Kind {
    fn harness(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude-code",
            Self::Aider => "aider",
        }
    }
    fn cli_version(self) -> &'static str {
        match self {
            Self::Codex => python::codex::VERSION,
            Self::Claude => python::claude::VERSION,
            Self::Aider => python::aider::VERSION,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Selection {
    runtime_id: String,
    runtime_hash: String,
    model: String,
    digest: String,
    weight: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperimentInput {
    assessment_version: u32,
    repetitions: u32,
    selections: Vec<Selection>,
    confirmed: bool,
}

fn task_key(p: &Value) -> String {
    format!(
        "{}:{}:{}",
        p["datasetId"], p["datasetVersion"], p["datasetHash"]
    )
}

impl Database {
    fn experiment_state(&self, w: &str, assessment: u32) -> AppResult<Value> {
        self.fit_read(w, Some(assessment))?;
        let c = self.connection()?;
        let mut q = c.prepare("SELECT id,status,created_at FROM benchmark_generation_jobs WHERE workspace_id=?1 AND json_extract(plan_json,'$.assessmentVersion')=?2 AND json_extract(plan_json,'$.purpose')=?3 ORDER BY rowid DESC LIMIT 100").map_err(error)?;
        let history = q.query_map(params![w,assessment,PURPOSE], |r| Ok(json!({"id":r.get::<_,String>(0)?,"status":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?}))).map_err(error)?.collect::<Result<Vec<_>,_>>().map_err(error)?;
        let mut candidates = Vec::new();
        for kind in KINDS {
            let mut checks = c.prepare("SELECT id FROM benchmark_generation_jobs WHERE workspace_id=?1 AND status='completed' AND json_extract(plan_json,'$.purpose')=?2 AND json_extract(plan_json,'$.assessmentVersion')=?3 ORDER BY rowid DESC LIMIT 100").map_err(error)?;
            let ids = checks
                .query_map(params![w, kind.runtime().purpose(), assessment], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            candidates.push((kind, ids));
        }
        let active = c.query_row("SELECT id,CAST(json_extract(plan_json,'$.assessmentVersion') AS INTEGER) FROM benchmark_generation_jobs WHERE workspace_id=?1 AND status='running' AND json_extract(plan_json,'$.purpose')=?2 LIMIT 1",params![w,PURPOSE],|r| Ok(json!({"id":r.get::<_,String>(0)?,"assessmentVersion":r.get::<_,u32>(1)?}))).optional().map_err(error)?;
        drop(q);
        drop(c);
        let mut options = Vec::new();
        for (kind, ids) in candidates {
            let mut included = BTreeSet::new();
            for id in ids {
                let runtime = self.runtime_record_for(w, &id, kind.runtime())?;
                let plan = &runtime["plan"];
                if runtime["clean"] != true
                    || plan["securityPolicy"] != kind.runtime_policy()?
                    || !runtime_ready(kind, &runtime)
                {
                    continue;
                }
                let key = task_key(plan);
                if !included.insert(key.clone()) {
                    continue;
                }
                let task = self.preflight_task(w, &workspace::task_input(plan)?)?;
                let function = &task["payload"]["task"]["body"]["spec"]["function"];
                options.push(json!({"id":id,"hash":runtime["planHash"],"harness":kind.harness(),"harnessVersion":kind.cli_version(),"taskKey":key,"datasetVersion":plan["datasetVersion"],"datasetId":plan["datasetId"],"function":function,"imageId":plan["imageId"]}));
            }
        }
        Ok(json!({"history":history,"options":options,"active":active,"maxRuns":MAX_RUNS}))
    }

    fn experiment_plan(
        &self,
        w: &str,
        id: &str,
    ) -> AppResult<(Value, String, String, Option<String>, Option<String>)> {
        let c = self.connection()?;
        require_workspace(&c, w)?;
        let (raw,digest,status,finished,problem):(String,String,String,Option<String>,Option<String>) = c.query_row("SELECT plan_json,plan_hash,status,finished_at,error FROM benchmark_generation_jobs WHERE workspace_id=?1 AND id=?2 AND json_extract(plan_json,'$.purpose')=?3",params![w,id,PURPOSE],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|_|"实验不存在或不属于当前工作区。")?;
        let p: Value = serde_json::from_str(&raw).map_err(json_error)?;
        if hash(raw.as_bytes()) != digest
            || p["purpose"] != PURPOSE
            || p["version"] != 1
            || p["owner"] != id
            || p["workspaceId"] != w
        {
            return Err("实验计划校验失败。".into());
        }
        Ok((p, digest, status, finished, problem))
    }
}

pub(super) fn runtime_ready(kind: Kind, runtime: &Value) -> bool {
    runtime["events"].as_array().is_some_and(|events| {
        events.iter().any(|e| {
            if e["kind"] != "probe" {
                return false;
            }
            if kind != Kind::Aider {
                return e["data"]["sandboxReady"] == true;
            }
            ["version", "automation", "container"].iter().all(|id| {
                e["data"]["checks"].as_array().is_some_and(|checks| {
                    checks
                        .iter()
                        .any(|c| c["id"] == *id && c["state"] == "passed")
                })
            })
        })
    })
}

#[tauri::command]
pub async fn list_harness_experiments(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.experiment_state(&workspace_id, assessment_version)
    })
    .await
}

#[tauri::command]
pub async fn start_harness_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    input: ExperimentInput,
) -> AppResult<Value> {
    if !input.confirmed
        || !(1..=3).contains(&input.repetitions)
        || input.selections.len() < 2
        || input
            .selections
            .len()
            .saturating_mul(input.repetitions as usize)
            > MAX_RUNS
    {
        return Err("请选择至少两个任务组合、1–3 次重复并确认实验。每批最多 24 个任务。".into());
    }
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let cancel = lease.receiver();
    let db = state.database.clone()?;
    let provider = state.provider.clone()?;
    let options = db.experiment_state(&workspace_id, input.assessment_version)?;
    let available = options["options"].as_array().ok_or("检查列表缺失。")?;
    let mut combinations = BTreeSet::new();
    let mut weights = BTreeMap::new();
    let mut inspections = BTreeMap::new();
    let mut environments = BTreeMap::new();
    let mut entries = Vec::new();
    for item in input.selections {
        if *cancel.borrow() {
            return Err("cancelled".into());
        }
        let option = available
            .iter()
            .find(|o| o["id"] == item.runtime_id && o["hash"] == item.runtime_hash)
            .ok_or("检查记录已变化，请刷新后重新选择。")?;
        let kind = KINDS
            .into_iter()
            .find(|k| option["harness"] == k.harness())
            .ok_or("不支持的编程工具。")?;
        let task = option["taskKey"]
            .as_str()
            .ok_or("任务版本缺失。")?
            .to_string();
        if !(1..=10).contains(&item.weight)
            || weights
                .insert(task.clone(), item.weight)
                .is_some_and(|w| w != item.weight)
        {
            return Err("同一任务的权重须一致，范围为 1–10。".into());
        }
        if !combinations.insert((
            kind.harness(),
            item.model.clone(),
            item.digest.clone(),
            task.clone(),
        )) {
            return Err("同一工具、模型和任务只能选择一次，请使用重复次数。".into());
        }
        let model = models::ModelRequest {
            model: item.model,
            expected_digest: Some(item.digest),
            parameters: parameters(),
            requirements: if kind == Kind::Aider {
                vec!["completion".into()]
            } else {
                vec!["completion".into(), "tool_calls".into()]
            },
        };
        model.validate()?;
        let inspection_key = (
            model.model.clone(),
            model.expected_digest.clone(),
            kind == Kind::Aider,
        );
        let inspection = if let Some(existing) = inspections.get(&inspection_key) {
            existing
        } else {
            let mut inspection_cancel = cancel.clone();
            let checked = tokio::select! {
                _=inspection_cancel.wait_for(|v| *v)=>return Err("cancelled".into()),
                result=provider.benchmark_inspect(model.clone())=>result?,
            };
            if !models::can_probe(&checked) {
                return Err(format!(
                    "{} / {} 未通过模型检查。{}",
                    kind.harness(),
                    model.model,
                    checked
                        .issues
                        .iter()
                        .map(|issue| issue.message.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            inspections
                .entry(inspection_key)
                .or_insert(serde_json::to_value(checked).map_err(json_error)?)
        };
        let runtime = db.runtime_record_for(&workspace_id, &item.runtime_id, kind.runtime())?;
        let image = option["imageId"].as_str().ok_or("镜像缺失。")?;
        let env_key = (kind.harness(), image.to_string());
        let env = if let Some(existing) = environments.get(&env_key) {
            existing
        } else {
            environments
                .entry(env_key)
                .or_insert(environment(kind, image, &cancel)?)
        };
        let check_id = runtime["plan"]["checkId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let sample = Input {
            assessment_version: input.assessment_version,
            check_id,
            runtime_id: item.runtime_id,
            runtime_hash: item.runtime_hash,
            model,
            confirmed: true,
        };
        let mut plan = db.session_plan(
            kind,
            &workspace_id,
            &new_id(),
            sample,
            inspection.clone(),
            env.clone(),
        )?;
        let task = db.preflight_task(&workspace_id, &workspace::task_input(&plan)?)?;
        plan["experimentId"] = json!(id);
        plan["harness"] = json!(kind.harness());
        plan["harnessVersion"] = json!(kind.cli_version());
        plan["repositorySnapshotId"] =
            task["payload"]["task"]["body"]["origin"]["repositoryId"].clone();
        plan["repositoryHash"] = task["payload"]["task"]["body"]["repositoryHash"].clone();
        plan["taskKey"] = json!(task_key(&plan));
        plan["function"] = task["payload"]["task"]["body"]["spec"]["function"].clone();
        plan["weight"] = json!(item.weight);
        entries.push((kind, plan));
    }
    let arms = entries
        .iter()
        .map(|(k, p)| {
            (
                k.harness(),
                p["model"]["model"].as_str(),
                p["model"]["expectedDigest"].as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if arms.len() < 2 {
        return Err("请至少选择两个不同的模型与工具组合。".into());
    }
    let mut schedule = Vec::new();
    for repetition in 1..=input.repetitions {
        for (kind, template) in &entries {
            let mut plan = template.clone();
            plan["owner"] = json!(new_id());
            plan["containerName"] = json!(format!("cortexos-bench-{}", new_id()));
            plan["repetition"] = json!(repetition);
            schedule.push((*kind, plan));
        }
    }
    let mut hardware = crate::benchmark::telemetry::hardware_snapshot();
    hardware["gpuSnapshot"] = crate::benchmark::resources::Sampler::new().gpu().await;
    let plan = json!({"purpose":PURPOSE,"version":1,"owner":id,"workspaceId":workspace_id,"assessmentVersion":input.assessment_version,"provider":"ollama","evidence":provider.benchmark_evidence(),"hardware":hardware,"appVersion":env!("CARGO_PKG_VERSION"),"repetitions":input.repetitions,"confirmed":true,"createdAt":now(),"items":schedule.iter().map(|(_,p)|p).collect::<Vec<_>>()});
    let raw = plan.to_string();
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    let mut c = db.connection()?;
    let tx = c.transaction().map_err(error)?;
    let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM benchmark_generation_jobs WHERE json_extract(plan_json,'$.purpose') IN ('harness_experiment','harness_codex_session','harness_claude_session','harness_aider_session','harness_codex_runtime','harness_claude_runtime','harness_aider_runtime','harness_workspace') AND status IN ('running','checking','opening','ready','grading','interrupted','cleanup_required') AND (json_extract(plan_json,'$.purpose')<>'harness_experiment' OR status='running'))",[],|r|r.get(0)).map_err(error)?;
    if occupied {
        return Err("请先结束或清理已有任务。".into());
    }
    tx.execute("INSERT INTO benchmark_generation_jobs(id,workspace_id,status,plan_json,plan_hash,created_at) VALUES(?1,?2,'running',?3,?4,?5)",params![id,workspace_id,raw,hash(raw.as_bytes()),plan["createdAt"].as_str()]).map_err(error)?;
    tx.commit().map_err(error)?;
    drop(c);
    let returned = id.clone();
    tauri::async_runtime::spawn(async move {
        let mut problem = None;
        for (kind, mut sample) in schedule {
            if *cancel.borrow() {
                break;
            }
            let child = sample["owner"].as_str().expect("generated id").to_string();
            sample["createdAt"] = json!(now());
            if let Err(reason) = db.insert_session_plan(&workspace_id, &child, &sample) {
                problem = Some(reason);
                break;
            }
            execute_session(&db, kind, &workspace_id, &child, &provider, cancel.clone()).await;
            match db.session_record(kind, &workspace_id, &child) {
                Ok(r)
                    if r["clean"] == true
                        && r["pendingGraders"].as_array().is_some_and(Vec::is_empty)
                        && !matches!(
                            r["status"].as_str(),
                            Some("running" | "grading" | "cleanup_required")
                        ) => {}
                _ => {
                    problem = Some("单题容器或结果待处理，已停止后续任务。".into());
                    break;
                }
            }
        }
        let status = if problem.is_some() {
            "failed"
        } else if *cancel.borrow() {
            "cancelled"
        } else {
            "completed"
        };
        if let Ok(c) = db.connection() {
            let _=c.execute("UPDATE benchmark_generation_jobs SET status=?1,finished_at=?2,error=?3 WHERE workspace_id=?4 AND id=?5 AND status='running'",params![status,now(),problem,workspace_id,id]);
        }
        drop(lease);
    });
    Ok(json!({"id":returned}))
}

#[tauri::command]
pub async fn stop_harness_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    experiment_id: String,
) -> AppResult<()> {
    let db = state.database.clone()?;
    let (_, _, status, _, _) = db.experiment_plan(&workspace_id, &experiment_id)?;
    if status == "running" {
        state.cancel_personalized_task(&workspace_id, &experiment_id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_harness_experiment(
    state: State<'_, AppState>,
    workspace_id: String,
    experiment_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.experiment_report(&workspace_id, &experiment_id)
    })
    .await
}

#[tauri::command]
pub async fn get_harness_experiment_trace(
    state: State<'_, AppState>,
    workspace_id: String,
    experiment_id: String,
    run_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.experiment_trace(&workspace_id, &experiment_id, &run_id)
    })
    .await
}

#[tauri::command]
pub async fn export_harness_experiment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
    experiment_id: String,
    format: crate::benchmark::report_transfer::Format,
) -> AppResult<bool> {
    let export_id = experiment_id.clone();
    let (bytes, ext) = blocking(state.database.clone()?, move |db| {
        db.experiment_export(&workspace_id, &export_id, format)
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_title("保存组合实验报告")
            .add_filter("组合实验报告", &[ext])
            .set_file_name(format!("CortexOS-Harness-{}.{}", experiment_id, ext))
            .blocking_save_file();
        let Some(path) = path else {
            return Ok(false);
        };
        crate::benchmark::report_transfer::write_new(
            &path.into_path().map_err(|_| "仅支持本机保存位置。")?,
            &bytes,
        )?;
        Ok(true)
    })
    .await
    .map_err(|_| "保存窗口异常。".to_string())?
}
