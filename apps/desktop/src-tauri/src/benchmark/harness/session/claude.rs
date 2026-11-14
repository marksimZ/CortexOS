use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeInput {
    assessment_version: u32,
    runtime_id: String,
    runtime_hash: String,
    model: models::ModelRequest,
    confirmed: bool,
}
#[tauri::command]
pub async fn get_harness_claude_session(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.session_state(
            Kind::Claude,
            &workspace_id,
            assessment_version,
            "",
            job_id.as_deref(),
        )
    })
    .await
}
#[tauri::command]
pub async fn start_harness_claude_session(
    state: State<'_, AppState>,
    workspace_id: String,
    input: ClaudeInput,
) -> AppResult<Value> {
    start_command(
        state,
        workspace_id,
        Input {
            assessment_version: input.assessment_version,
            check_id: String::new(),
            runtime_id: input.runtime_id,
            runtime_hash: input.runtime_hash,
            model: input.model,
            confirmed: input.confirmed,
        },
        Kind::Claude,
    )
    .await
}
#[tauri::command]
pub async fn cleanup_harness_claude_session(
    state: State<'_, AppState>,
    workspace_id: String,
    job_id: String,
) -> AppResult<Value> {
    cleanup_command(state, workspace_id, job_id, Kind::Claude).await
}
