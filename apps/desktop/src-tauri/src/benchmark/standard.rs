//! Standard batches reuse the existing scheduler and graders without cloud/profile access.
use super::personalized::batch::{self, BatchInput};
use crate::{
    cloud::CloudState,
    commands::{AppState, blocking},
    db::Database,
    models::AppResult,
};
use serde_json::{Value, json};
#[cfg(test)]
use {std::sync::Arc, crate::tasks::TaskKind};
use tauri::State;

impl Database {
    pub(crate) fn batch_model_event(
        &self,
        workspace: &str,
        run: &str,
        kind: &str,
        payload: Value,
    ) -> AppResult<()> {
        let c = self.connection()?;
        let s = super::repository::summary(&c, workspace, run)?;
        super::repository::benchmark_event(&c, run, workspace, &s.attempt_id, kind, payload)
    }
    pub fn standard_case_catalog(&self, workspace: &str) -> AppResult<Vec<Value>> {
        Ok(self.benchmark_dataset_cases(workspace,"cortex-core-60",2)?.iter().map(|c|json!({"id":c.case_id,"prompt":c.input.prompt,"difficulty":c.difficulty,"evaluator":c.scoring.id,"requirements":c.requirements,"contentHash":c.content_hash})).collect())
    }
    pub(crate) fn mark_batch_stopping(&self, workspace: &str, id: &str) -> AppResult<()> {
        self.connection()?.execute("UPDATE benchmark_batches SET status='stopping' WHERE workspace_id=?1 AND id=?2 AND status='running'",rusqlite::params![workspace,id]).map_err(crate::db::error)?;
        Ok(())
    }
}
#[tauri::command]
pub async fn list_standard_benchmark_cases(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<Value>> {
    blocking(state.database.clone()?, move |db| {
        db.standard_case_catalog(&workspace_id)
    })
    .await
}
#[tauri::command]
pub async fn plan_standard_benchmark(
    state: State<'_, AppState>,
    workspace_id: String,
    input: BatchInput,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.plan_standard_batch(&workspace_id, input)
    })
    .await
}
#[tauri::command]
pub async fn get_standard_benchmark(
    state: State<'_, AppState>,
    workspace_id: String,
    batch_id: String,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.standard_report(&workspace_id, &batch_id)
    })
    .await
}
#[tauri::command]
pub async fn list_standard_benchmarks(
    state: State<'_, AppState>,
    workspace_id: String,
) -> AppResult<Vec<Value>> {
    blocking(state.database.clone()?, move |db| {
        db.standard_batch_history(&workspace_id)
    })
    .await
}
#[tauri::command]
pub async fn start_standard_benchmark(
    state: State<'_, AppState>,
    cloud: State<'_, CloudState>,
    workspace_id: String,
    batch_id: String,
    approved_plan_hash: String,
    status_channel: Option<tauri::ipc::JavaScriptChannelId>,
    webview: tauri::Webview,
) -> AppResult<Value> {
    let lease = batch::batch_lease(&state, &workspace_id, &batch_id, &approved_plan_hash, true, status_channel.map(|id| id.channel_on(webview))).await?;
    let db = state.database.clone()?;
    let scope = workspace_id.clone();
    let id = batch_id.clone();
    let (plan, initial) = blocking(db.clone(), move |db| {
        let plan = db.begin_standard_batch(&scope, &id, &approved_plan_hash)?;
        Ok((plan, db.personalized_batch(&scope, &id)?))
    })
    .await?;
    let state = state.inner().clone();
    let cloud = cloud.inner().clone();
    tauri::async_runtime::spawn(async move {
        if batch::execute(
            state,
            cloud,
            workspace_id.clone(),
            batch_id.clone(),
            plan,
            lease,
        )
        .await
        .is_err()
        {
            let _ = db.finish_batch(&workspace_id, &batch_id, "failed");
        }
    });
    Ok(initial)
}
#[tauri::command]
pub fn stop_standard_benchmark(
    state: State<'_, AppState>,
    workspace_id: String,
    batch_id: String,
    only_waiting: Option<bool>,
) -> AppResult<()> {
    let db = state.database.as_ref().map_err(Clone::clone)?;
    db.require_batch_scope(&workspace_id, &batch_id, true)?;
    if only_waiting == Some(true) { return state.tasks.cancel_waiting(&batch_id, &workspace_id, crate::tasks::TaskKind::Benchmark); }
    state.cancel_personalized_task(&workspace_id, &batch_id)?;
    db.mark_batch_stopping(&workspace_id, &batch_id)
}
#[cfg(test)]
#[path = "standard_tests.rs"]
mod tests;
