use super::repository::{StartInput, Summary};
use crate::{
    commands::{AppState, blocking},
    generation::{self, InspectionPolicy},
    models::{AppResult, new_id},
    tasks::TaskKind,
};

pub async fn start(state: AppState, workspace: String, input: StartInput) -> AppResult<Summary> {
    if input.dataset_version == 2 || input.personalized_dataset.is_some() {
        return super::runner_v2::start(state, workspace, input).await;
    }
    let database = state.database.clone()?;
    let provider = state.provider.clone()?;
    let scope = workspace.clone();
    let (intent, existing) = blocking(database.clone(), move |db| {
        let intent = db.benchmark_intent(&scope, input)?;
        let existing = db.benchmark_existing(&scope, &intent)?;
        Ok((intent, existing))
    })
    .await?;
    if let Some(existing) = existing {
        return Ok(existing);
    }
    let id = new_id();
    let lease = state.claim_task(&id, &workspace, TaskKind::Benchmark)?;
    let mut cancel = lease.receiver();
    if *cancel.borrow() {
        return Err("评测准备已停止，尚未发送模型请求。".into());
    }
    let inspection = tokio::select! {
        biased;
        _=cancel.changed()=>return Err("评测准备已停止，尚未发送模型请求。".into()),
        result=provider.benchmark_inspect(intent.input.model.clone())=>result?,
    };
    if inspection.status != "ready_for_probe" {
        return Err(format!(
            "模型预检未通过：{}",
            inspection
                .issues
                .iter()
                .map(|i| i.message.as_str())
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    let scope = workspace.clone();
    let task_id = id.clone();
    let (prepared, intent) = blocking(database.clone(), move |db| {
        if *cancel.borrow() {
            return Err("评测准备已停止，尚未发送模型请求。".into());
        }
        let prepared = db.prepare_benchmark(&scope, &task_id, &intent, &inspection)?;
        Ok((prepared, intent))
    })
    .await?;
    lease.set_execution(&prepared.execution.run.id)?;
    let summary = prepared.summary.clone();
    tauri::async_runtime::spawn(async move {
        let settings = intent.input.model.settings();
        let completion = generation::execute(
            database.clone(),
            provider,
            prepared.execution,
            settings,
            InspectionPolicy::Benchmark(intent.input.model),
            lease.receiver(),
            |_| {},
        )
        .await;
        let scope = workspace.clone();
        let experiment = id.clone();
        let saved = completion.saved;
        let result = blocking(database.clone(), move |db| {
            if !saved {
                return Err(completion
                    .update
                    .error_message
                    .unwrap_or_else(|| "回答保存失败。".into()));
            }
            db.finish_benchmark(&scope, &experiment)
        })
        .await;
        if let Err(message) = result {
            let scope = workspace;
            let experiment = id;
            if let Err(error) = blocking(database, move |db| {
                db.benchmark_finalize_error(&scope, &experiment, &message)
            })
            .await
            {
                eprintln!("评测保存未完成，请保留数据并重启后检查：{error}");
            }
        }
        // The task owns the lease through durable output, scoring and experiment finalization.
        drop(lease);
    });
    Ok(summary)
}
