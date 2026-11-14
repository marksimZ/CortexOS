//! D3-2 environment gate only; no inference or automatic programming is enabled.
use super::*;
use crate::{benchmark::python, tasks::TaskKind};
pub mod aider;
pub mod claude;
mod records;
const PURPOSE: &str = "harness_codex_runtime";
const VERSION: &str = "codex-runtime-check-2";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Codex,
    Claude,
    Aider,
}
impl Kind {
    pub(super) fn purpose(self) -> &'static str {
        match self {
            Self::Codex => PURPOSE,
            Self::Claude => "harness_claude_runtime",
            Self::Aider => "harness_aider_runtime",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    assessment_version: u32,
    check_id: String,
    check_hash: String,
    image_id: String,
    confirmed: bool,
    #[serde(default)]
    security_policy: Option<String>,
}

fn cleanup_for(db: &Database, workspace: &str, id: &str, kind: Kind) -> AppResult<bool> {
    let r = db.runtime_record_for(workspace, id, kind)?;
    if r["clean"] == true {
        return Ok(true);
    }
    let p = &r["plan"];
    match python::cleanup_validation_container(
        p["environment"]["endpoint"]
            .as_str()
            .ok_or("本地端点缺失。")?,
        p["imageId"].as_str().ok_or("镜像缺失。")?,
        id,
        p["containerName"].as_str().ok_or("容器名字缺失。")?,
    ) {
        Ok(mut observation) => {
            if kind == Kind::Codex && p["securityPolicy"]["id"] == python::codex_policy::COMPATIBLE
            {
                if let Err(reason) = python::codex_policy::cleanup_file(id) {
                    db.runtime_event_for(workspace,id,kind,"cleanup_error",json!({"reason":reason,"containerClean":true,"temporaryPolicyFileClean":false}))?;
                    return Ok(false);
                }
                observation["temporaryPolicyFileClean"] = json!(true);
            }
            if kind == Kind::Claude
                && p["securityPolicy"]["id"] == python::claude_policy::COMPATIBLE
            {
                if let Err(reason) = python::claude_policy::cleanup(id) {
                    db.runtime_event_for(workspace,id,kind,"cleanup_error",json!({"reason":reason,"containerClean":true,"temporaryPolicyFileClean":false}))?;
                    return Ok(false);
                }
                observation["temporaryPolicyFileClean"] = json!(true);
            }
            db.runtime_event_for(workspace, id, kind, "cleanup", observation)?;
            Ok(true)
        }
        Err(reason) => {
            db.runtime_event_for(
                workspace,
                id,
                kind,
                "cleanup_error",
                json!({"reason":reason}),
            )?;
            Ok(false)
        }
    }
}
pub(crate) fn cleanup_on_close(db: &Database) -> AppResult<()> {
    for kind in [Kind::Codex, Kind::Claude, Kind::Aider] {
        for (workspace, id) in db.pending_runtime_jobs(kind)? {
            let clean = cleanup_for(db, &workspace, &id, kind)?;
            db.finish_runtime_for(
                &workspace,
                &id,
                kind,
                if clean { "closed" } else { "cleanup_required" },
                None,
            )?;
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn get_harness_codex_images(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        let c = db.connection()?;
        require_workspace(&c, &workspace_id)?;
        drop(c);
        python::codex::images()
    })
    .await
}
#[tauri::command]
pub async fn get_harness_codex_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    check_id: String,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.runtime_state(
            &workspace_id,
            assessment_version,
            &check_id,
            job_id.as_deref(),
        )
    })
    .await
}
#[tauri::command]
pub async fn check_harness_codex_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
) -> AppResult<Value> {
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let job = id.clone();
    let cancel = lease.receiver();
    blocking(db.clone(), move |db| {
        db.runtime_task(&scope, &input)?;
        let env = python::codex::environment_for(&input.image_id, &cancel)?;
        db.begin_runtime(&scope, &job, &input, env)
    })
    .await?;
    let returned = id.clone();
    spawn_check(db, workspace_id, id, Kind::Codex, lease);
    Ok(json!({"id":returned}))
}
fn spawn_check(
    db: std::sync::Arc<Database>,
    workspace_id: String,
    id: String,
    kind: Kind,
    lease: crate::tasks::Lease,
) {
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> AppResult<()> {
            let r = db.runtime_record_for(&workspace_id, &id, kind)?;
            let p = &r["plan"];
            let observed = match kind {
                Kind::Aider => python::aider::observe(
                    p["environment"]["endpoint"].as_str().unwrap(),
                    p["imageId"].as_str().unwrap(),
                    &id,
                    p["containerName"].as_str().unwrap(),
                    &lease.receiver(),
                )?,
                Kind::Codex => python::codex::observe(
                    p["environment"]["endpoint"].as_str().unwrap(),
                    p["imageId"].as_str().unwrap(),
                    &id,
                    p["containerName"].as_str().unwrap(),
                    p["securityPolicy"]["id"]
                        .as_str()
                        .unwrap_or(python::codex_policy::DEFAULT),
                    &lease.receiver(),
                )?,
                Kind::Claude => python::claude::observe(
                    p["environment"]["endpoint"].as_str().unwrap(),
                    p["imageId"].as_str().unwrap(),
                    &id,
                    p["containerName"].as_str().unwrap(),
                    p["securityPolicy"]["id"]
                        .as_str()
                        .unwrap_or(python::codex_policy::DEFAULT),
                    &lease.receiver(),
                )?,
            };
            db.runtime_event_for(&workspace_id, &id, kind, "probe", observed)
        })();
        let problem = result.err();
        if let Some(reason) = &problem {
            let _ = db.runtime_event_for(
                &workspace_id,
                &id,
                kind,
                "probe_error",
                json!({"reason":reason}),
            );
        }
        let clean = cleanup_for(&db, &workspace_id, &id, kind).unwrap_or(false);
        let status = if !clean {
            "cleanup_required"
        } else if *lease.receiver().borrow() {
            "cancelled"
        } else if problem.is_some() {
            "failed"
        } else {
            "completed"
        };
        let _ = db.finish_runtime_for(&workspace_id, &id, kind, status, problem);
        drop(lease);
    });
}
#[tauri::command]
pub async fn cleanup_harness_codex_runtime(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
) -> AppResult<Value> {
    cleanup_command(state, workspace_id, job_id, Kind::Codex).await
}
async fn cleanup_command(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
    kind: Kind,
) -> AppResult<Value> {
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let job = job_id.clone();
    let r = blocking(db.clone(), move |db| {
        db.runtime_record_for(&scope, &job, kind)
    })
    .await?;
    if r["clean"] == true {
        return Ok(r);
    }
    if r["status"] == "checking" {
        state.cancel_personalized_task(&workspace_id, &job_id)?;
        return Ok(json!({"stopping":true}));
    }
    let _lease = state.claim_task(&job_id, &workspace_id, TaskKind::Benchmark)?;
    blocking(db, move |db| {
        let clean = cleanup_for(db, &workspace_id, &job_id, kind)?;
        db.finish_runtime_for(
            &workspace_id,
            &job_id,
            kind,
            if clean { "closed" } else { "cleanup_required" },
            None,
        )?;
        db.runtime_record_for(&workspace_id, &job_id, kind)
    })
    .await
}
