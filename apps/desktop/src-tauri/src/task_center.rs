use crate::{
    benchmark::dataset::require_workspace,
    cloud::CloudState,
    commands::{AppState, blocking},
    db::error,
    models::AppResult,
    tasks::{LiveTask, TaskKind},
};
use rusqlite::{Connection, params};
use serde::Serialize;
use tauri::State;

#[tauri::command]
pub(crate) fn read_live_tasks(
    state: State<'_, AppState>,
    workspace_id: Option<String>,
) -> AppResult<Vec<LiveTask>> {
    let mut tasks = state.tasks.live_tasks(workspace_id.as_deref())?;
    tasks.extend(state.storage_scans.live_tasks(workspace_id.as_deref())?);
    Ok(tasks)
}

#[tauri::command]
pub(crate) fn cancel_waiting_task(
    state: State<'_, AppState>,
    workspace_id: String,
    task_id: String,
    kind: TaskKind,
) -> AppResult<()> {
    state.tasks.cancel_waiting(&task_id, &workspace_id, kind)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskRecord {
    id: String,
    kind: String,
    status: String,
    at: String,
    error: Option<String>,
    context_label: Option<String>,
    conversation: Option<String>,
    message: Option<String>,
    assessment: Option<u32>,
    processed: Option<u32>,
    total: Option<u32>,
}

#[tauri::command]
pub(crate) async fn read_task_history(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    active_id: Option<String>,
) -> AppResult<Vec<TaskRecord>> {
    let cloud = cloud.inner().clone();
    blocking(state.database.clone()?, move |db| {
        let conn = db.connection()?;
        require_workspace(&conn, &workspace_id)?;
        let read =
            |personalized| read_history(&conn, &workspace_id, active_id.as_deref(), personalized);
        cloud
            .with_access(|| Ok(read(true)))
            .unwrap_or_else(|_| read(false))
    })
    .await
}

fn read_history(
    conn: &Connection,
    workspace_id: &str,
    active_id: Option<&str>,
    personalized: bool,
) -> AppResult<Vec<TaskRecord>> {
    // Select only rows the workspace can open in the original modules.
    let mut recent = conn.prepare(
        "WITH history(id,kind,status,at,error,label,conversation,message,assessment) AS (
            SELECT r.id,'chat',r.status,COALESCE(r.finished_at,r.started_at),r.error_message,
                   c.title,r.conversation_id,r.assistant_message_id,NULL
              FROM execution_runs r JOIN conversations c
                ON c.id=r.conversation_id AND c.workspace_id=r.workspace_id
             WHERE r.workspace_id=?1 AND c.purpose='chat'
            UNION ALL
            SELECT id, CASE WHEN json_extract(plan_json,'$.input.datasetId')='cortex-core-60'
                            THEN 'standard' ELSE 'personalized_batch' END,
                   status,COALESCE(finished_at,created_at),NULL,NULL,NULL,NULL,NULL
              FROM benchmark_batches
             WHERE workspace_id=?1 AND (?2 OR json_extract(plan_json,'$.input.datasetId')='cortex-core-60')
            UNION ALL
            SELECT id, CASE COALESCE(json_extract(plan_json,'$.purpose'),'personalized')
                            WHEN 'personalized' THEN 'personalized_generation'
                            ELSE json_extract(plan_json,'$.purpose') END,
                   status,COALESCE(finished_at,created_at),error,NULL,NULL,NULL,
                   CASE WHEN json_type(plan_json,'$.assessmentVersion')='integer'
                        THEN json_extract(plan_json,'$.assessmentVersion') END
              FROM benchmark_generation_jobs
             WHERE workspace_id=?1 AND
                  (json_extract(plan_json,'$.purpose') IN
                    ('harness_experiment','harness_readonly_run','harness_programming_validation',
                     'harness_workspace','harness_codex_session','harness_claude_session','harness_aider_session',
                     'harness_execution_preflight','harness_codex_runtime','harness_claude_runtime','harness_aider_runtime')
                    OR (?2 AND COALESCE(json_extract(plan_json,'$.purpose'),'personalized')='personalized'))
            UNION ALL
            SELECT id,'rag',status,COALESCE(finished_at,created_at),NULL,NULL,NULL,NULL,NULL
              FROM rag_runtime_experiments WHERE workspace_id=?1
        )
        SELECT id,kind,status,at,substr(error,1,4096),label,conversation,message,assessment
          FROM history
         ORDER BY CASE WHEN id=?3 THEN 0 ELSE 1 END,at DESC,id DESC LIMIT 32",
    ).map_err(error)?;
    let mut records = recent
        .query_map(params![workspace_id, personalized, active_id], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                status: row.get(2)?,
                at: row.get(3)?,
                error: row.get(4)?,
                context_label: row.get(5)?,
                conversation: row.get(6)?,
                message: row.get(7)?,
                assessment: row.get(8)?,
                processed: None,
                total: None,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    let mut batch = conn
        .prepare(
            "SELECT COUNT(*), COALESCE(SUM(status IN ('finished','error','cancelled')),0),
                (SELECT substr(error,1,4096) FROM benchmark_batch_items
                   WHERE batch_id=?1 AND status='error' AND error IS NOT NULL
                   ORDER BY ordinal DESC LIMIT 1)
           FROM benchmark_batch_items WHERE batch_id=?1",
        )
        .map_err(error)?;
    for record in records
        .iter_mut()
        .filter(|r| matches!(r.kind.as_str(), "standard" | "personalized_batch"))
    {
        let (total, processed, reason) = batch
            .query_row([&record.id], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(error)?;
        record.total = Some(total);
        record.processed = Some(processed);
        record.error = reason;
    }
    Ok(records)
}
