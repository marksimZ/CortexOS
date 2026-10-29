use super::{
    evaluation::{Score, State},
    preflight, python,
    repository::{StartInput, Summary},
};
use crate::{
    commands::{AppState, blocking},
    generation::{self, InspectionPolicy},
    models::{AppResult, ExecutionPurpose, new_id},
    tasks::TaskKind,
};
use serde_json::json;

pub async fn start(state: AppState, workspace: String, input: StartInput) -> AppResult<Summary> {
    start_inner(state, workspace, input, None, false).await
}
pub(crate) async fn run_batch_case(
    state: AppState,
    workspace: String,
    input: StartInput,
    lease: std::sync::Arc<crate::tasks::Lease>,
) -> AppResult<Summary> {
    start_inner(state, workspace, input, Some(lease), true).await
}
async fn start_inner(
    state: AppState,
    workspace: String,
    input: StartInput,
    shared: Option<std::sync::Arc<crate::tasks::Lease>>,
    wait: bool,
) -> AppResult<Summary> {
    let case_started = std::time::Instant::now();
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
    let lease = match shared {
        Some(l) => l,
        None => std::sync::Arc::new(state.claim_task(&id, &workspace, TaskKind::Benchmark)?),
    };
    let mut cancel = lease.receiver();
    let inspection = tokio::select! {biased;_=cancel.changed()=>return Err("评测准备已停止，尚未发送。".into()),result=provider.benchmark_inspect(intent.input.model.clone())=>result};
    let (mut profile, mut blocked) = match inspection {
        Ok(checked) => {
            let blocked = if preflight::can_probe(&checked) {
                None
            } else {
                Some(Score::state(
                    if checked
                        .issues
                        .iter()
                        .any(|i| i.code == "capability_unsupported")
                    {
                        State::Unsupported
                    } else {
                        State::ExecutionError
                    },
                    checked
                        .issues
                        .iter()
                        .map(|i| i.code)
                        .collect::<Vec<_>>()
                        .join(","),
                ))
            };
            (
                serde_json::to_value(checked).map_err(|_| "无法保存预检。")?,
                blocked,
            )
        }
        Err(error) => (
            json!({"request":intent.input.model,"status":"unavailable","error":error}),
            Some(Score::state(
                State::ExecutionError,
                "model_preflight_unavailable",
            )),
        ),
    };
    if blocked.is_none() && intent.case.scoring.id == "python_assertions" {
        let image = intent.input.python_image.clone();
        let check_cancel = lease.receiver();
        let checked = tauri::async_runtime::spawn_blocking(move || {
            python::environment(image.as_deref(), Some(&check_cancel))
        })
        .await
        .map_err(|_| "无法检查Python运行环境。")?;
        match checked {
            Ok(snapshot) => profile["sandbox"] = snapshot,
            Err(reason) => {
                profile["sandbox"] = json!({"availability":"unavailable","reason":reason});
                blocked = Some(Score::state(
                    if reason == "cancelled" {
                        State::Cancelled
                    } else {
                        State::ExecutionError
                    },
                    reason,
                ));
            }
        }
    }
    let scope = workspace.clone();
    let task = id.clone();
    let preparing_cancel = lease.receiver();
    let (prepared, intent) = blocking(database.clone(), move |db| {
        if *preparing_cancel.borrow() {
            return Err("评测准备已停止。".into());
        }
        let prepared = db.prepare_benchmark_record(&scope, &task, &intent, &profile)?;
        Ok((prepared, intent))
    })
    .await?;
    let preparation_ms = case_started.elapsed().as_millis() as u64;
    lease.set_execution(&prepared.execution.run.id)?;
    if let Some(score) = blocked {
        let scope = workspace.clone();
        let task = id.clone();
        let run = prepared.execution.run;
        let scoring_started = std::time::Instant::now();
        blocking(database.clone(), move |db| {
            db.execution_finish(
                &run,
                ExecutionPurpose::Benchmark,
                "",
                "failed",
                Some("预检未通过，未发送模型请求。"),
                json!({}),
            )?;
            db.save_benchmark_score(&scope, &task, score)?;
            db.benchmark_case_timing(&scope, &task, json!({"clock":"monotonic","preparationMs":preparation_ms,"scoringAndSaveMs":scoring_started.elapsed().as_millis() as u64,"totalMs":case_started.elapsed().as_millis() as u64,"sent":false,"scope":"logical_case_through_first_score_saved"}))?;
            Ok(())
        })
        .await?;
        return blocking(database, move |db| {
            Ok(db.benchmark_detail(&workspace, &id)?.summary)
        })
        .await;
    }
    let summary = prepared.summary.clone();
    let work = async move {
        let generation_started = std::time::Instant::now();
        let completion = generation::execute(
            database.clone(),
            provider,
            prepared.execution,
            intent.input.model.settings(),
            InspectionPolicy::BenchmarkV2(intent.input.model),
            lease.receiver(),
            |_| {},
        )
        .await;
        let generation_and_save_ms = generation_started.elapsed().as_millis() as u64;
        let scoring_started = std::time::Instant::now();
        let scope = workspace.clone();
        let task = id.clone();
        let scoring_cancel = lease.receiver();
        let result = blocking(database.clone(), move |db| {
            if !completion.saved {
                return Err("评测输出未保存，不能评分。".into());
            }
            db.finish_benchmark_v2(&scope, &task, Some(scoring_cancel))?;
            db.benchmark_case_timing(&scope, &task, json!({"clock":"monotonic","preparationMs":preparation_ms,"generationAndSaveMs":generation_and_save_ms,"scoringAndSaveMs":scoring_started.elapsed().as_millis() as u64,"totalMs":case_started.elapsed().as_millis() as u64,"scope":"logical_case_through_first_score_saved"}))
        })
        .await;
        if let Err(message) = result {
            let scope = workspace;
            let task = id;
            let _ = blocking(database, move |db| {
                db.benchmark_finalize_error(&scope, &task, &message)
            })
            .await;
        }
        drop(lease);
    };
    if wait {
        work.await;
    } else {
        tauri::async_runtime::spawn(work);
    }
    Ok(summary)
}
