use crate::{
    benchmark::preflight::ModelRequest,
    commands::blocking,
    db::Database,
    models::*,
    ollama::{Ndjson, Ollama},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

pub enum InspectionPolicy {
    Chat,
    Benchmark(ModelRequest),
    BenchmarkV2(ModelRequest),
}
impl InspectionPolicy {
    fn purpose(&self) -> ExecutionPurpose {
        match self {
            Self::Chat => ExecutionPurpose::Chat,
            Self::Benchmark(_) | Self::BenchmarkV2(_) => ExecutionPurpose::Benchmark,
        }
    }
    async fn inspect(&self, provider: &Ollama, settings: &ModelSettings) -> AppResult<Value> {
        match self {
            Self::Chat => provider.inspect(settings).await,
            Self::Benchmark(request) | Self::BenchmarkV2(request) => {
                let checked = provider.benchmark_inspect(request.clone()).await?;
                if checked.status != "ready_for_probe" && !matches!(self, Self::BenchmarkV2(_)) {
                    return Err(format!(
                        "模型运行前复核未通过：{}",
                        checked
                            .issues
                            .iter()
                            .map(|i| i.message.as_str())
                            .collect::<Vec<_>>()
                            .join("；")
                    ));
                }
                serde_json::to_value(checked).map_err(|_| "无法保存模型复核信息。".into())
            }
        }
    }
}
pub struct Completion {
    pub saved: bool,
    pub update: StreamUpdate,
}

pub async fn execute(
    database: Arc<Database>,
    provider: Ollama,
    prepared: PreparedRun,
    settings: ModelSettings,
    policy: InspectionPolicy,
    mut cancel: watch::Receiver<bool>,
    mut notify: impl FnMut(StreamUpdate),
) -> Completion {
    let purpose = policy.purpose();
    let v2 = matches!(&policy, InspectionPolicy::BenchmarkV2(_));
    let run = prepared.run;
    let mut content = String::new();
    let mut seq = 0u64;
    let operation_started = std::time::Instant::now();
    let mut sent_at: Option<std::time::Instant> = None;
    let mut header_ms = None;
    let mut first_text_ms = None;
    let mut first_tool_ms = None;
    let mut load_observation = Value::Null;
    let mut monitor = None;
    let mut resource_error = None;
    let mut resource_setup_ms = None;
    let mut resource_attempted = false;
    let already_cancelled = *cancel.borrow();
    let result = if already_cancelled {
        Err("__cancelled__".into())
    } else {
        let generation = async {
            let recorded = run.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(
                    &recorded,
                    purpose,
                    "model_inspection_started",
                    json!({"provider":"ollama","transport":"loopback"}),
                )
            })
            .await?;
            let inspected = policy.inspect(&provider, &settings).await?;
            let v2_allowed = !v2
                || inspected["status"] == "ready_for_probe"
                || (inspected["status"] == "needs_verification"
                    && inspected["issues"].as_array().is_some_and(|a| {
                        !a.is_empty() && a.iter().all(|i| i["code"] == "capability_unknown")
                    }));
            if v2 {
                let loaded = inspected
                    .pointer("/catalog/loaded")
                    .and_then(Value::as_array);
                let model = loaded.and_then(|a| a.iter().find(|m| m["name"] == settings.model));
                load_observation = json!({"status":if loaded.is_none(){"unknown"}else if model.is_some(){"loaded"}else{"not_loaded"},"checkedAt":inspected["checkedAt"],"model":model,"scope":"provider_residency_observed_not_controlled_cold_cache"});
            }
            let recorded = run.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(&recorded, purpose, "model_inspection_completed", inspected)
            })
            .await?;
            if !v2_allowed {
                return Err("模型运行前复核未通过，已保留实际预检记录。".into());
            }
            if v2 {
                resource_attempted = true;
                let setup_started = std::time::Instant::now();
                match crate::benchmark::telemetry::Monitor::start(database.clone(), run.clone())
                    .await
                {
                    Ok(started) => monitor = Some(started),
                    Err(error) => resource_error = Some(error),
                }
                resource_setup_ms = Some(setup_started.elapsed().as_millis() as u64);
            }
            let recorded = run.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(
                    &recorded,
                    purpose,
                    "model_request_started",
                    json!({"endpoint":"/api/chat"}),
                )
            })
            .await?;
            let request_started = std::time::Instant::now();
            sent_at = Some(request_started);
            let mut response = provider.chat(&settings, &prepared.request).await?;
            header_ms = Some(request_started.elapsed().as_millis() as u64);
            let mut tool_entries = 0usize;
            let mut tool_bytes = 0usize;
            let recorded = run.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(
                    &recorded,
                    purpose,
                    "model_response_received",
                    json!({"status":200}),
                )
            })
            .await?;
            let mut parser = if v2 {
                Ndjson::strict()
            } else {
                Ndjson::default()
            };
            let mut checkpoint = tokio::time::interval(Duration::from_millis(250));
            let mut dirty = false;
            loop {
                let chunk = tokio::select! {
                    chunk=response.chunk()=>chunk.map_err(|_| "模型连接中断，已保留收到的内容。请重试。".to_string())?,
                    _=checkpoint.tick()=>{
                        if dirty {
                            let (recorded,text)=(run.clone(),content.clone());
                            blocking(database.clone(),move|db|db.execution_checkpoint(&recorded,purpose,&text)).await?;
                            dirty=false;
                        }
                        continue;
                    }
                };
                let frames = match chunk.as_ref() {
                    Some(bytes) => parser.push(bytes),
                    None => parser.finish(),
                };
                for frame in frames {
                    let frame = frame?;
                    if let Some(message) = frame.message.as_ref() {
                        if !message.content.trim().is_empty() && first_text_ms.is_none() {
                            first_text_ms = Some(request_started.elapsed().as_millis() as u64);
                        }
                        content.push_str(&message.content);
                        dirty |= !message.content.is_empty();
                        if v2
                            && let Some(calls) =
                                message.tool_calls.as_ref().filter(|c| !c.is_empty())
                        {
                            let bytes = serde_json::to_vec(calls)
                                .map_err(|_| "无法记录模型工具字段。")?
                                .len();
                            tool_bytes += bytes;
                            tool_entries += calls.len();
                            if tool_bytes > 1024 * 1024 || tool_entries > 128 {
                                return Err("模型工具输出超出安全长度。".into());
                            }
                            if first_tool_ms.is_none() {
                                first_tool_ms = Some(request_started.elapsed().as_millis() as u64);
                            }
                            let recorded = run.clone();
                            let calls = calls.clone();
                            blocking(database.clone(), move |db| {
                                db.execution_event(
                                    &recorded,
                                    purpose,
                                    "native_tool_calls_received",
                                    json!({"provider":"ollama","frame":seq,"calls":calls}),
                                )
                            })
                            .await?;
                        }
                    }
                    if content.len() > 1024 * 1024 {
                        return Err("模型输出超出安全长度，已停止生成。".into());
                    }
                    seq += 1;
                    notify(update(&run, seq, &content, "running", None));
                    if frame.done {
                        if content.trim().is_empty() && !(v2 && tool_entries > 0) {
                            return Err("模型没有返回可用文字，请换一个模型或重试。".into());
                        }
                        return Ok(frame.metrics());
                    }
                }
                if chunk.is_none() {
                    return Err("回答流提前结束，已保留收到的内容。请重试。".into());
                }
            }
        };
        tokio::select! {
            biased;
            _=cancel.changed()=>Err("__cancelled__".into()),
            _=tokio::time::sleep(Duration::from_secs(settings.timeout_seconds))=>Err("生成超时，已保留收到的内容。可以增加超时、缩短问题或换较小模型后重试。".into()),
            result=generation=>result,
        }
    };
    let response_ms = sent_at.map(|t| t.elapsed().as_millis() as u64);
    let operation_ms = operation_started.elapsed().as_millis() as u64;
    if let Some(monitor) = monitor {
        if let Err(error) = monitor.finish().await {
            resource_error = Some(error);
        }
    }
    let (status, mut message, mut metrics) = match result {
        Ok(metrics) => ("completed", None, metrics),
        Err(error) if error == "__cancelled__" => ("cancelled", None, json!({})),
        Err(error) => ("failed", Some(error), json!({})),
    };
    if v2 {
        metrics["measurementVersion"] = json!(1);
        metrics["clientTiming"] = json!({"clock":"monotonic","headerMs":header_ms,"firstTextMs":first_text_ms,"firstToolMs":first_tool_ms,"responseMs":response_ms,"completeMs":if status=="completed"{response_ms}else{None},"operationMs":operation_ms,"terminalStatus":status});
        metrics["loadObservation"] = load_observation;
        metrics["resourceCollection"] = json!({"status":if resource_error.is_some(){"error"}else if sent_at.is_some(){"collected"}else if resource_attempted{"interrupted_before_request"}else{"not_started"},"setupMs":resource_setup_ms,"error":resource_error,"scope":"system_and_whole_gpu_not_model_exclusive"});
    }
    let (saved_run, saved_content, saved_message) = (run.clone(), content.clone(), message.clone());
    let mut saved = true;
    if let Err(error) = blocking(database, move |db| {
        db.execution_finish(
            &saved_run,
            purpose,
            &saved_content,
            status,
            saved_message.as_deref(),
            metrics,
        )
    })
    .await
    {
        saved = false;
        message = Some(if matches!(purpose, ExecutionPurpose::Chat) {
            format!("{error} 当前回答仍在窗口中，可先复制；请检查磁盘空间。")
        } else {
            format!(
                "{error} 评测结果未完整保存，请保留数据并检查磁盘空间；仅已写入的内容可以恢复。"
            )
        });
    }
    Completion {
        saved,
        update: update(
            &run,
            seq + 1,
            &content,
            if message.is_some() { "failed" } else { status },
            message,
        ),
    }
}

fn update(
    run: &RunSummary,
    seq: u64,
    content: &str,
    status: &str,
    error_message: Option<String>,
) -> StreamUpdate {
    StreamUpdate {
        workspace_id: run.workspace_id.clone(),
        conversation_id: run.conversation_id.clone(),
        run_id: run.id.clone(),
        message_id: run.assistant_message_id.clone(),
        seq,
        content: content.into(),
        status: status.into(),
        error_message,
    }
}
