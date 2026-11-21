use crate::{
    benchmark::{python, report_transfer::write_new, resources::Sampler},
    commands::{AppState, blocking},
    db,
    models::{AppResult, new_id, now},
    tasks::{Lease, TaskCounts, TaskKind, TaskStatus},
};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    area: &'static str,
    status: TaskStatus,
    count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Problem {
    area: &'static str,
    category: &'static str,
    next_action: &'static str,
    count: i64,
    last_seen_unix: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalData {
    schema_version: usize,
    cloud_connections: Vec<ConnectionCount>,
    activity: Vec<Activity>,
    problems: Vec<Problem>,
}

#[derive(Serialize)]
struct ConnectionCount {
    provider: &'static str,
    status: &'static str,
    count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticReport {
    format_version: u32,
    app_version: &'static str,
    captured_at: String,
    backend: &'static str,
    closing: bool,
    ollama: &'static str,
    docker: &'static str,
    queue: Option<TaskCounts>,
    storage: Option<TaskCounts>,
    database: Option<LocalData>,
    history_hours: u32,
    resources: Value,
}

const CLOUD_FAILURES: &str = "
SELECT CASE
  WHEN state='outcome_unknown' THEN 'outcome_unknown'
  WHEN failure IN ('invalid_credential','model_denied','quota_exceeded','rate_limited','network_unavailable','service_error','revalidation_required') THEN failure
  ELSE 'failed' END AS reason, count(*), max(started_unix)
FROM model_cloud_calls WHERE state IN ('failed','outcome_unknown') AND started_unix>=?1
GROUP BY reason";

fn problem(category: &str) -> (&'static str, &'static str) {
    match category {
        "invalid_credential" => ("invalid_credential", "check_credentials"),
        "model_denied" => ("model_denied", "check_configuration"),
        "revalidation_required" => ("revalidation_required", "check_configuration"),
        "quota_exceeded" => ("quota_exceeded", "check_budget"),
        "rate_limited" => ("rate_limited", "retry_manually_later"),
        "network_unavailable" => ("network_unavailable", "check_connection"),
        "service_error" => ("service_error", "retry_manually_later"),
        "outcome_unknown" => ("outcome_unknown", "check_history_before_retry"),
        "interrupted" => ("interrupted", "check_history_before_retry"),
        _ => ("failed", "check_history"),
    }
}

fn history_status(raw: &str) -> Option<TaskStatus> {
    match raw {
        "completed" => Some(TaskStatus::Completed),
        "failed" | "blocked" | "cleanup_required" => Some(TaskStatus::Failed),
        "cancelled" | "stopped" => Some(TaskStatus::Cancelled),
        "interrupted" | "outcome_unknown" => Some(TaskStatus::Interrupted),
        _ => None,
    }
}

fn provider(raw: &str) -> &'static str {
    match raw {
        "openai" => "openai",
        "anthropic" => "anthropic",
        "gemini" => "gemini",
        "compatible" => "compatible",
        _ => "other",
    }
}

fn connection_state(raw: &str) -> &'static str {
    match raw {
        "verified" => "verified",
        "invalid_credential" => "invalid_credential",
        "model_denied" => "model_denied",
        "quota_exceeded" => "quota_exceeded",
        "rate_limited" => "rate_limited",
        "network_unavailable" => "network_unavailable",
        "service_error" => "service_error",
        "revalidation_required" => "revalidation_required",
        _ => "unverified",
    }
}

fn read_data(c: &Connection, since: i64) -> AppResult<LocalData> {
    let schema_version = db::backup_schema_version(c)?;
    let mut query = c
        .prepare(
            "
        SELECT json_extract(configuration_json,'$.provider'),
               json_extract(configuration_json,'$.catalogState'), count(*)
        FROM model_cloud_connections GROUP BY 1,2 ORDER BY 1,2
    ",
        )
        .map_err(db::error)?;
    let cloud_connections = query
        .query_map([], |row| {
            let kind = row.get::<_, Option<String>>(0)?;
            let state = row.get::<_, Option<String>>(1)?;
            Ok(ConnectionCount {
                provider: provider(kind.as_deref().unwrap_or("")),
                status: connection_state(state.as_deref().unwrap_or("")),
                count: row.get(2)?,
            })
        })
        .map_err(db::error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(db::error)?;
    let mut activity = Vec::new();
    let mut problems = Vec::new();
    // Each table represents a user-visible operation. The cloud section counts individual
    // HTTP attempts separately, including bounded 429 retries. Raw errors never leave SQLite.
    for (area, sql) in [
        (
            "chat",
            "SELECT r.status, count(*), max(unixepoch(r.finished_at)) FROM execution_runs r JOIN conversations c ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id WHERE c.purpose='chat' AND unixepoch(r.finished_at)>=?1 GROUP BY r.status",
        ),
        (
            "benchmark",
            "SELECT status, count(*), max(unixepoch(finished_at)) FROM benchmark_batches WHERE unixepoch(finished_at)>=?1 GROUP BY status",
        ),
        (
            "harness",
            "SELECT status, count(*), max(unixepoch(finished_at)) FROM benchmark_generation_jobs WHERE unixepoch(finished_at)>=?1 AND substr(json_extract(plan_json,'$.purpose'),1,8)='harness_' GROUP BY status",
        ),
        (
            "rag",
            "SELECT status, count(*), max(unixepoch(finished_at)) FROM rag_runtime_experiments WHERE unixepoch(finished_at)>=?1 GROUP BY status",
        ),
        (
            "embedding",
            "SELECT state, count(*), max(started_unix) FROM model_local_embeddings WHERE started_unix>=?1 GROUP BY state",
        ),
        ("cloud", CLOUD_FAILURES),
    ] {
        let mut query = c.prepare(sql).map_err(db::error)?;
        let mut rows = query.query([since]).map_err(db::error)?;
        while let Some(row) = rows.next().map_err(db::error)? {
            let raw: String = row.get(0).map_err(db::error)?;
            let count = row.get(1).map_err(db::error)?;
            if area != "cloud" {
                let Some(status) = history_status(&raw) else {
                    continue;
                };
                activity.push(Activity {
                    area,
                    status,
                    count,
                });
                if !matches!(status, TaskStatus::Failed | TaskStatus::Interrupted) {
                    continue;
                }
            }
            let (category, next_action) = problem(&raw);
            problems.push(Problem {
                area,
                category,
                next_action,
                count,
                last_seen_unix: row.get(2).map_err(db::error)?,
            });
        }
    }
    problems.sort_by_key(|item| std::cmp::Reverse(item.last_seen_unix));
    Ok(LocalData {
        schema_version,
        cloud_connections,
        activity,
        problems,
    })
}

fn claim(state: &AppState) -> AppResult<Lease> {
    let lease = state
        .storage_scans
        .claim(&new_id(), "diagnostics", TaskKind::LocalMaintenance)?;
    if state.closing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("应用正在关闭，已取消本机诊断。".into());
    }
    Ok(lease)
}

async fn capture(state: &AppState, lease: &Lease) -> AppResult<DiagnosticReport> {
    let data = async {
        blocking(state.database.clone()?, |db| {
            read_data(&*db.connection()?, chrono::Utc::now().timestamp() - 86400)
        })
        .await
    };
    let ollama = async {
        match &state.provider {
            Ok(client) if client.get("/api/version").await.is_ok() => "available",
            _ => "unavailable",
        }
    };
    let cancel = lease.receiver();
    let docker = tauri::async_runtime::spawn_blocking(move || python::local_engine_status(&cancel));
    let resources = async {
        let mut sampler = Sampler::new();
        sampler.system();
        let (gpu, _) = tokio::join!(
            sampler.gpu(),
            tokio::time::sleep(Duration::from_millis(200))
        );
        let system = sampler.system();
        let devices = gpu["devices"].as_array().map(|devices| {
            devices
                .iter()
                .map(|device| {
                    json!({"usedBytes":device["usedBytes"],"totalBytes":device["totalBytes"],
                   "utilizationPercent":device["utilizationPercent"]})
                })
                .collect::<Vec<_>>()
        });
        json!({"systemCpuPercent":system["systemCpuPercent"],"systemMemory":system["systemMemory"],
            "backendWorkingSetBytes":system["appWorkingSetBytes"],"gpus":devices})
    };
    let (database, ollama, docker, resources) = tokio::join!(data, ollama, docker, resources);
    if *lease.receiver().borrow() {
        return Err("已取消本机诊断。".into());
    }
    Ok(DiagnosticReport {
        format_version: 1,
        app_version: crate::app_version(),
        captured_at: now(),
        backend: "available",
        closing: state.closing.load(std::sync::atomic::Ordering::SeqCst),
        ollama,
        docker: docker.unwrap_or("unavailable"),
        queue: state.tasks.counts().ok(),
        // This check holds the storage slot. Report other storage work only.
        storage: state.storage_scans.counts().ok().map(|mut task| {
            if task.running > 0 {
                task.running -= 1;
            } else {
                task.cancelling = task.cancelling.saturating_sub(1);
            }
            task
        }),
        database: database.ok(),
        history_hours: 24,
        resources,
    })
}

#[tauri::command]
pub(crate) async fn read_diagnostics(state: State<'_, AppState>) -> AppResult<DiagnosticReport> {
    let lease = claim(&state)?;
    capture(&state, &lease).await
}

#[tauri::command]
pub(crate) async fn export_diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<bool> {
    let lease = claim(&state)?;
    let report = capture(&state, &lease).await?;
    let bytes = serde_json::to_vec_pretty(&report).map_err(db::json_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_title("保存本机诊断报告")
            .add_filter("诊断 JSON", &["json"])
            .set_file_name(format!("CortexOS-诊断-{}.json", new_id()))
            .blocking_save_file();
        let Some(path) = path else { return Ok(false) };
        write_new(
            &path.into_path().map_err(|_| "请选择可用的本机文件位置。")?,
            &bytes,
        )?;
        Ok(true)
    })
    .await
    .map_err(|_| "诊断导出意外结束。")?
}
