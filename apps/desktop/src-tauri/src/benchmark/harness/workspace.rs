//! Task-scoped copy, manual handoff and independent grading. No Coding Harness runs.
use super::*;
use crate::{
    benchmark::{dataset::Case, evaluation, python},
    tasks::TaskKind,
};
use tokio::sync::watch;
pub(super) mod package;
mod records;
const PURPOSE: &str = "harness_workspace";
const VERSION: &str = "function-workspace-1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prepare {
    assessment_version: u32,
    check_id: String,
    check_hash: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Submit {
    job_id: String,
    plan_hash: String,
    code: String,
    confirmed: bool,
}
pub(super) fn task_input(plan: &Value) -> AppResult<preflight::CheckInput> {
    serde_json::from_value(json!({"assessmentVersion":plan["assessmentVersion"],"datasetId":plan["datasetId"],"version":plan["datasetVersion"],"datasetHash":plan["datasetHash"]})).map_err(json_error)
}
fn copy_ref<'a>(plan: &'a Value, env: &'a Value) -> AppResult<python::workspace::CopyRef<'a>> {
    Ok(python::workspace::CopyRef {
        endpoint: env["endpoint"].as_str().ok_or("副本环境记录缺失。")?,
        image: plan["imageId"].as_str().ok_or("镜像记录缺失。")?,
        owner: plan["owner"].as_str().ok_or("副本归属缺失。")?,
        name: plan["containerName"].as_str().ok_or("副本名字缺失。")?,
    })
}
fn cleanup(db: &Database, workspace: &str, id: &str) -> AppResult<bool> {
    let record = db.workspace_record(workspace, id)?;
    let plan = &record["plan"];
    let Some(env) = record["events"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["kind"] == "environment"))
        .map(|e| &e["data"])
    else {
        return Ok(true);
    };
    let mut clean = true;
    for name in record["pendingContainers"]
        .as_array()
        .ok_or("容器清单缺失。")?
    {
        let name = name.as_str().ok_or("容器名字缺失。")?;
        match python::cleanup_validation_container(
            env["endpoint"].as_str().ok_or("本地端点缺失。")?,
            plan["imageId"].as_str().ok_or("镜像缺失。")?,
            id,
            name,
        ) {
            Ok(observation) => db.workspace_event(workspace, id, "cleanup", observation)?,
            Err(reason) => {
                clean = false;
                db.workspace_event(
                    workspace,
                    id,
                    "cleanup_error",
                    json!({"name":name,"reason":reason}),
                )?;
            }
        }
    }
    Ok(clean)
}
fn prepare_copy(
    db: &Database,
    workspace: &str,
    id: &str,
    cancel: watch::Receiver<bool>,
) -> AppResult<()> {
    let record = db.workspace_record(workspace, id)?;
    let plan = &record["plan"];
    let result = (|| -> AppResult<()> {
        let env = python::harness_environment(
            plan["imageId"].as_str().ok_or("镜像缺失。")?,
            Some(&cancel),
        )?;
        db.workspace_event(workspace, id, "environment", env.clone())?;
        db.workspace_event(
            workspace,
            id,
            "container",
            json!({"name":plan["containerName"],"owner":id,"role":"workspace"}),
        )?;
        let observation =
            python::workspace::open(&copy_ref(plan, &env)?, &plan["bundle"], &cancel)?;
        package::verify_observation(
            plan,
            &observation["observed"],
            plan["bundle"]["originalCode"].as_str().unwrap(),
        )?;
        db.workspace_event(workspace, id, "prepared", observation)?;
        if *cancel.borrow() {
            return Err("cancelled".into());
        }
        db.workspace_finish(workspace, id, "ready", None)
    })();
    if let Err(reason) = result {
        db.workspace_event(workspace, id, "preparation_error", json!({"reason":reason}))?;
        let clean = cleanup(db, workspace, id)?;
        db.workspace_finish(
            workspace,
            id,
            if !clean {
                "cleanup_required"
            } else if *cancel.borrow() {
                "cancelled"
            } else {
                "failed"
            },
            Some(reason),
        )?;
    }
    Ok(())
}
fn submit_copy(
    db: &Database,
    workspace: &str,
    id: &str,
    code: &str,
    cancel: watch::Receiver<bool>,
) -> AppResult<()> {
    let record = db.workspace_record(workspace, id)?;
    let plan = &record["plan"];
    let mut outcome = "completed";
    let result = (|| -> AppResult<()> {
        let env = record["events"]
            .as_array()
            .and_then(|a| a.iter().find(|e| e["kind"] == "environment"))
            .map(|e| &e["data"])
            .ok_or("副本环境记录缺失。")?;
        let r = copy_ref(plan, env)?;
        let observed = python::workspace::change(
            &r,
            code,
            plan["bundle"]["taskHash"].as_str().unwrap(),
            &cancel,
        )?;
        package::verify_observation(plan, &observed, code)?;
        let candidate = package::patch(db, workspace, plan, code)?;
        db.workspace_event(
            workspace,
            id,
            "candidate",
            json!({"patch":candidate,"observation":observed}),
        )?;
        if !cleanup(db, workspace, id)? {
            return Err("container_cleanup_failed".into());
        }
        if *cancel.borrow() {
            return Err("cancelled".into());
        }
        let task = db.preflight_task(workspace, &task_input(plan)?)?;
        let case: Case =
            serde_json::from_value(task["payload"]["case"].clone()).map_err(json_error)?;
        let score = python::score_owned(
            &case,
            code,
            plan["imageId"].as_str().unwrap(),
            id,
            &cancel,
            |name| {
                db.workspace_event(
                    workspace,
                    id,
                    "container",
                    json!({"name":name,"owner":id,"role":"grader"}),
                )
            },
        );
        outcome = match score.state {
            evaluation::State::Cancelled => "cancelled",
            evaluation::State::ExecutionError | evaluation::State::ScoringError => "failed",
            _ => "completed",
        };
        db.workspace_event(workspace,id,"score",json!({"score":score,"candidateHash":hash(code.as_bytes()),"provenance":"manual_handoff","modelCalls":0,"harnessExecution":"Not Run"}))?;
        Ok(())
    })();
    let problem = result.err();
    if let Some(reason) = &problem {
        db.workspace_event(workspace, id, "handoff_error", json!({"reason":reason}))?;
        outcome = if *cancel.borrow() {
            "cancelled"
        } else {
            "failed"
        };
    }
    let clean = cleanup(db, workspace, id)?;
    db.workspace_finish(
        workspace,
        id,
        if clean { outcome } else { "cleanup_required" },
        problem,
    )
}

pub(crate) fn cleanup_on_close(db: &Database) -> AppResult<()> {
    for (workspace, id) in db.pending_workspace_jobs()? {
        let clean = cleanup(db, &workspace, &id)?;
        db.workspace_finish(
            &workspace,
            &id,
            if clean { "closed" } else { "cleanup_required" },
            None,
        )?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_harness_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    check_id: String,
    job_id: Option<String>,
) -> AppResult<Value> {
    let tasks = state.tasks.clone();
    blocking(state.database.clone()?, move |db| {
        let request = tasks.scoped_request(
            &workspace_id,
            TaskKind::HarnessWorkspace,
            &format!("{assessment_version}:{check_id}"),
        )?;
        let result = db.workspace_state(
            &workspace_id,
            assessment_version,
            &check_id,
            job_id.as_deref(),
        )?;
        Ok(add_job_request(result, request))
    })
    .await
}
#[tauri::command]
pub async fn prepare_harness_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Prepare,
    wait_for_resources: Option<bool>,
) -> AppResult<Value> {
    let id = new_id();
    let db = state.database.clone()?;
    let input = if wait_for_resources == Some(true) {
        let workspace = workspace_id.clone();
        blocking(db.clone(), move |db| {
            let check =
                db.harness_preflight(&workspace, input.assessment_version, Some(&input.check_id))?;
            if check["selected"]["contentHash"] != input.check_hash {
                return Err("执行前检查已不匹配，请重新读取。".into());
            }
            Ok(input)
        })
        .await?
    } else {
        input
    };
    let subject = format!("{}:{}", input.assessment_version, input.check_id);
    let lease = queue_job(
        &state,
        &id,
        &workspace_id,
        TaskKind::HarnessWorkspace,
        &subject,
        wait_for_resources,
    )
    .await?;
    let scope = workspace_id.clone();
    let job = id.clone();
    blocking(db.clone(), move |db| {
        db.begin_workspace(&scope, &job, &input)
    })
    .await?;
    let returned = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = prepare_copy(&db, &workspace_id, &id, lease.receiver());
        if let Err(reason) = result {
            let _ = db.workspace_finish(&workspace_id, &id, "cleanup_required", Some(reason));
        }
        drop(lease);
    });
    Ok(json!({"id":returned}))
}
#[tauri::command]
pub async fn submit_harness_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Submit,
    wait_for_resources: Option<bool>,
) -> AppResult<Value> {
    let db = state.database.clone()?;
    let (subject, input) = if wait_for_resources == Some(true) {
        let workspace = workspace_id.clone();
        blocking(db.clone(), move |db| {
            let record = db.workspace_record(&workspace, &input.job_id)?;
            if !input.confirmed
                || record["status"] != "ready"
                || record["planHash"] != input.plan_hash
            {
                return Err("副本状态已变化，请重新读取。".into());
            }
            let plan = record.get("plan").ok_or("副本计划缺失。")?;
            let assessment = plan["assessmentVersion"].as_u64().ok_or("分析版本缺失。")?;
            let check = plan["checkId"].as_str().ok_or("执行前检查缺失。")?;
            Ok((format!("{assessment}:{check}"), input))
        })
        .await?
    } else {
        (String::new(), input)
    };
    let lease = queue_job(
        &state,
        &input.job_id,
        &workspace_id,
        TaskKind::HarnessWorkspace,
        &subject,
        wait_for_resources,
    )
    .await?;
    let scope = workspace_id.clone();
    let (id, code) = blocking(db.clone(), move |db| {
        db.begin_workspace_submit(&scope, input)
    })
    .await?;
    let returned = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = submit_copy(&db, &workspace_id, &id, &code, lease.receiver());
        if let Err(reason) = result {
            let _ = db.workspace_finish(&workspace_id, &id, "cleanup_required", Some(reason));
        }
        drop(lease);
    });
    Ok(json!({"id":returned}))
}
#[tauri::command]
pub async fn close_harness_workspace(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
    only_waiting: Option<bool>,
) -> AppResult<Value> {
    if only_waiting == Some(true) {
        state
            .tasks
            .cancel_waiting(&job_id, &workspace_id, TaskKind::HarnessWorkspace)?;
        return Ok(json!({"cancelledWait":true}));
    }
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let id = job_id.clone();
    let record = blocking(db.clone(), move |db| db.workspace_record(&scope, &id)).await?;
    if ["completed", "closed", "failed", "cancelled"]
        .contains(&record["status"].as_str().unwrap_or(""))
        && record["pendingContainers"]
            .as_array()
            .is_some_and(Vec::is_empty)
    {
        return Ok(record);
    }
    if ["opening", "grading"].contains(&record["status"].as_str().unwrap_or("")) {
        state
            .tasks
            .cancel(&job_id, &workspace_id, TaskKind::HarnessWorkspace)?;
        return Ok(json!({"stopping":true}));
    }
    let _lease = state.claim_task(&job_id, &workspace_id, TaskKind::Benchmark)?;
    blocking(db, move |db| {
        let clean = cleanup(db, &workspace_id, &job_id)?;
        db.workspace_finish(
            &workspace_id,
            &job_id,
            if clean { "closed" } else { "cleanup_required" },
            None,
        )?;
        db.workspace_record(&workspace_id, &job_id)
    })
    .await
}
