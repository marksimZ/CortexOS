use crate::{
    benchmark::preflight::ModelRequest,
    commands::blocking,
    db::Database,
    model_runtime::{Generated, GenerationProvider, LocalGeneration, ModelIdentity},
    models::*,
    ollama::Ollama,
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

struct Attempt {
    content: String,
    seq: u64,
    status: &'static str,
    message: Option<String>,
    metrics: Value,
}

impl Attempt {
    async fn save(
        self,
        database: Arc<Database>,
        run: RunSummary,
        purpose: ExecutionPurpose,
    ) -> Completion {
        finish(
            database,
            run,
            purpose,
            self.seq + 1,
            self.content,
            self.status,
            self.message,
            self.metrics,
        )
        .await
    }
}

pub async fn execute(
    database: Arc<Database>,
    provider: Ollama,
    prepared: PreparedRun,
    settings: ModelSettings,
    policy: InspectionPolicy,
    cancel: watch::Receiver<bool>,
    mut notify: impl FnMut(StreamUpdate),
) -> Completion {
    let purpose = policy.purpose();
    let attempt = attempt_local(
        database.clone(),
        &provider,
        &prepared,
        &settings,
        &policy,
        cancel,
        0,
        &mut notify,
    )
    .await;
    attempt.save(database, prepared.run, purpose).await
}

async fn attempt_local(
    database: Arc<Database>,
    provider: &Ollama,
    prepared: &PreparedRun,
    settings: &ModelSettings,
    policy: &InspectionPolicy,
    mut cancel: watch::Receiver<bool>,
    mut seq: u64,
    notify: &mut impl FnMut(StreamUpdate),
) -> Attempt {
    let purpose = policy.purpose();
    let v2 = matches!(&policy, InspectionPolicy::BenchmarkV2(_));
    let run = &prepared.run;
    let mut content = String::new();
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
    let mut sampler = None;
    let call_id = new_id();
    let already_cancelled = *cancel.borrow();
    let result = if already_cancelled {
        Err("__cancelled__".into())
    } else {
        let generation = async {
            let recorded = run.clone();
            let recorded_call = call_id.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(
                    &recorded,
                    purpose,
                    "model_inspection_started",
                    json!({"provider":"ollama","transport":"loopback"}),
                )
            })
            .await?;
            let inspected = policy.inspect(provider, settings).await?;
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
            let revision = inspected["digest"].as_str().or_else(|| {
                inspected
                    .pointer("/catalog/models")?
                    .as_array()?
                    .iter()
                    .find(|item| item["name"] == settings.model)?["digest"]
                    .as_str()
            });
            let target = json!(ModelIdentity {
                provider: "ollama",
                model: &settings.model,
                revision
            });
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
                    json!({"endpoint":"/api/chat","target":target,"localCallId":recorded_call}),
                )
            })
            .await?;
            let request_started = std::time::Instant::now();
            sent_at = Some(request_started);
            if matches!(policy, InspectionPolicy::Chat) {
                let mut baseline = crate::benchmark::resources::Sampler::new();
                baseline.system();
                sampler = Some(baseline);
            }
            let Generated::Streaming(mut response) = provider
                .generate(LocalGeneration {
                    settings,
                    request: &prepared.request,
                    strict: v2,
                })
                .await?
            else {
                return Err("本地生成未返回回答流。".into());
            };
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
            let mut checkpoint = tokio::time::interval(Duration::from_millis(250));
            let mut dirty = false;
            loop {
                let frames = tokio::select! {
                    frames=response.receive()=>frames?,
                    _=checkpoint.tick()=>{
                        if dirty {
                            let (recorded,text)=(run.clone(),content.clone());
                            blocking(database.clone(),move|db|db.execution_checkpoint(&recorded,purpose,&text)).await?;
                            dirty=false;
                        }
                        continue;
                    }
                };
                let Some(frames) = frames else {
                    return Err("回答流提前结束，已保留收到的内容。请重试。".into());
                };
                for frame in frames {
                    let frame = frame?;
                    if !frame.text.is_empty() {
                        if !frame.text.trim().is_empty() && first_text_ms.is_none() {
                            first_text_ms = Some(request_started.elapsed().as_millis() as u64);
                        }
                        content.push_str(&frame.text);
                        dirty = true;
                    }
                    if v2 && !frame.calls.is_empty() {
                        let calls = frame.calls;
                        let bytes = serde_json::to_vec(&calls)
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
                    if content.len() > 1024 * 1024 {
                        return Err("模型输出超出安全长度，已停止生成。".into());
                    }
                    seq += 1;
                    notify(update(run, seq, &content, "running", None));
                    if let Some(metrics) = frame.final_metrics {
                        if content.trim().is_empty() && !(v2 && tool_entries > 0) {
                            return Err("模型没有返回可用文字，请换一个模型或重试。".into());
                        }
                        return Ok(metrics);
                    }
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
    let (status, message, mut metrics) = match result {
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
    if matches!(policy, InspectionPolicy::Chat)
        && let Some(mut sampler) = sampler
    {
        let resource = sampler.system();
        let recorded = run.clone();
        let usage = json!({"localCallId":call_id,"provider":"ollama","model":settings.model,"status":status,
            "elapsedMs":response_ms,"inputTokens":metrics["promptEvalCount"],"outputTokens":metrics["evalCount"],
            "systemCpuPercent":resource["systemCpuPercent"],"systemMemory":resource["systemMemory"]});
        let _ = blocking(database, move |db| {
            db.execution_event(&recorded, purpose, "local_model_usage", usage)
        })
        .await;
    }
    Attempt {
        content,
        seq,
        status,
        message,
        metrics,
    }
}

async fn attempt_cloud_chat(
    database: Arc<Database>,
    target: &crate::cloud::runtime_connections::CloudTarget,
    prepared: &PreparedRun,
    timeout_seconds: u64,
    mut cancel: watch::Receiver<bool>,
    meter: &crate::runtime_usage::Meter,
) -> Attempt {
    let run = &prepared.run;
    let purpose = ExecutionPurpose::Chat;
    let provider = target.connection.provider.name();
    let timeout = Duration::from_secs(timeout_seconds);
    let result = if *cancel.borrow() {
        None
    } else {
        let call = async {
            let dispatch = &prepared.manifest["cloudTransfer"];
            let knowledge = crate::cloud::privacy::context_versions(&prepared.manifest)?;
            let output = target
                .generate(
                    &prepared.request,
                    timeout,
                    meter,
                    Some(dispatch),
                    &knowledge,
                )
                .await?;
            let recorded = run.clone();
            blocking(database.clone(), move |db| {
                db.execution_event(
                    &recorded,
                    purpose,
                    "model_response_received",
                    json!({"transport":"cloud","provider":provider,"status":200}),
                )
            })
            .await?;
            if output.text.trim().is_empty() {
                return Err("云端模型未返回可用的回答文字。".into());
            }
            Ok(output)
        };
        tokio::select! {
            biased;
            _=cancel.wait_for(|stopped|*stopped)=>None,
            result=call=>Some(result),
        }
    };
    let (content, status, message, metrics) = match result {
        Some(Ok(output)) => {
            let (input, output_tokens) = target.generation_tokens(&output.usage);
            (
                output.text,
                "completed",
                None,
                json!({
                    "provider":provider,"promptEvalCount":input,"evalCount":output_tokens,"totalDurationNs":output.elapsed_ms.saturating_mul(1_000_000),
                    "finishReason":output.finish_reason,"requestBodyHash":output.request_hash
                }),
            )
        }
        Some(Err(error)) => (
            String::new(),
            "failed",
            Some(error),
            json!({"provider":provider}),
        ),
        None => (
            String::new(),
            "cancelled",
            None,
            json!({"provider":provider}),
        ),
    };
    Attempt {
        content,
        seq: 0,
        status,
        message,
        metrics,
    }
}

pub(crate) async fn execute_chat(
    database: Arc<Database>,
    provider: Option<Ollama>,
    cloud: Option<crate::cloud::runtime_connections::CloudTarget>,
    prepared: PreparedRun,
    settings: ModelSettings,
    backup: Option<String>,
    cancel: watch::Receiver<bool>,
    mut notify: impl FnMut(StreamUpdate),
) -> Completion {
    let mut attempt = if let Some(target) = &cloud {
        let meter=crate::runtime_usage::Meter::new(database.clone(),prepared.run.workspace_id.clone(),prepared.run.id.clone(),"chat");
        attempt_cloud_chat(
            database.clone(),
            target,
            &prepared,
            settings.timeout_seconds,
            cancel.clone(),
            &meter,
        )
        .await
    } else if let Some(provider) = &provider {
        attempt_local(
            database.clone(),
            provider,
            &prepared,
            &settings,
            &InspectionPolicy::Chat,
            cancel.clone(),
            0,
            &mut notify,
        )
        .await
    } else {
        Attempt {
            content: String::new(),
            seq: 0,
            status: "failed",
            message: Some("本地模型服务不可用。".into()),
            metrics: json!({}),
        }
    };
    if attempt.status == "failed"
        && attempt.content.is_empty()
        && !*cancel.borrow()
        && let (Some(model), Some(provider)) = (backup, provider.as_ref())
        && (cloud.is_some() || model != settings.model)
    {
        let backup = ModelSettings { model, ..settings };
        let saved = backup.clone();
        let start = prepared.clone();
        let reason = attempt
            .message
            .clone()
            .unwrap_or_else(|| "主模型未返回回答。".into());
        match blocking(database.clone(), move |db| {
            db.prepare_local_backup(start, &saved, &reason)
        })
        .await
        {
            Ok(prepared) => {
                let fallback = attempt_local(
                    database.clone(),
                    provider,
                    &prepared,
                    &backup,
                    &InspectionPolicy::Chat,
                    cancel,
                    attempt.seq,
                    &mut notify,
                )
                .await;
                return fallback
                    .save(database, prepared.run, ExecutionPurpose::Chat)
                    .await;
            }
            Err(failure) => {
                attempt.message = Some(format!(
                    "{} 本机备用模型未开始运行：{failure}",
                    attempt.message.as_deref().unwrap_or_default()
                ));
            }
        }
    }
    attempt
        .save(database, prepared.run, ExecutionPurpose::Chat)
        .await
}

pub(crate) async fn execute_rag_case(
    database: Arc<Database>, provider: Option<&Ollama>,
    cloud: Option<&crate::cloud::runtime_connections::CloudTarget>, prepared: PreparedRun,
    settings: &ModelSettings, meter: &crate::runtime_usage::Meter, cancel: watch::Receiver<bool>,
) -> Completion {
    let attempt = if let Some(target)=cloud {
        attempt_cloud_chat(database.clone(),target,&prepared,settings.timeout_seconds,cancel,meter).await
    } else if let Some(provider)=provider {
        attempt_local(database.clone(),provider,&prepared,settings,&InspectionPolicy::Chat,cancel,0,&mut |_|{}).await
    } else {
        Attempt {content:String::new(),seq:0,status:"failed",message:Some("本地模型服务不可用。".into()),metrics:json!({})}
    };
    attempt.save(database,prepared.run,ExecutionPurpose::Chat).await
}

async fn finish(
    database: Arc<Database>,
    run: RunSummary,
    purpose: ExecutionPurpose,
    seq: u64,
    content: String,
    status: &'static str,
    mut message: Option<String>,
    metrics: Value,
) -> Completion {
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
            seq,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cloud::runtime_connections::CloudTarget,
        model_runtime::http_fixture,
        runtime_profiles::{ChatRuntime, CloudChat, Profile},
    };

    #[tokio::test]
    async fn chat_fallback_keeps_both_snapshots_and_uses_only_local_backup() {
        let db = Arc::new(Database::rag_evaluation_database().unwrap());
        let ws = db.create_workspace("备用请求").unwrap().id;
        let chat = db.create_conversation(&ws).unwrap().id;
        let (remote, served_cloud) = http_fixture::serve(1, |_, path, body| {
            assert_eq!(path, "/chat/completions");
            assert_eq!(body["model"], "remote");
            (503, json!({"error":{"message":"unavailable"}}))
        });
        let target = CloudTarget::loopback_test(remote, "remote");
        let profile: Profile = serde_json::from_value(json!({"id":new_id(),"updatedAt":now(),
            "name":"云端问答","mode":"cloud","generation":{"kind":"cloud","model":"remote","connectionId":target.connection.id},
            "embedding":{"kind":"off"},"reranking":{"kind":"off"}})).unwrap();
        let settings = ModelSettings {
            model: "remote".into(),
            ..ModelSettings::default()
        };
        let prepared = db
            .prepare_chat_runtime(
                &new_id(),
                &ws,
                &chat,
                "固定题目",
                &settings,
                &[],
                false,
                None,
                None,
                false,
                Some(&ChatRuntime {
                    profile: &profile,
                    cloud: Some(CloudChat {
                        target: &target,
                        share_selection: false,
                        granted_versions: &[],
                        public_fixture: false,
                    }),
                    ranking: None,
                }),
                None,
                true,
            )
            .unwrap();
        let original = prepared.request.clone();
        let run = prepared.run.id.clone();
        let (local, served_local) = http_fixture::serve(2, |_, path, body| {
            assert_eq!(body["model"], "backup");
            match path {
                "/api/show" => (
                    200,
                    json!({"details":{"format":"gguf"},"capabilities":["completion"]}),
                ),
                "/api/chat" => {
                    assert_eq!(body["messages"][1]["content"], "固定题目");
                    (
                        200,
                        json!({"done":true,"message":{"role":"assistant","content":"本机备用答案"},"eval_count":6}),
                    )
                }
                _ => panic!("unexpected fixture path {path}"),
            }
        });
        let local = Ollama::with_endpoint(local.as_str()).unwrap();
        let (_keep, rx) = watch::channel(false);
        let finished = execute_chat(
            db.clone(),
            Some(local),
            Some(target),
            prepared,
            settings,
            Some("backup".into()),
            rx,
            |_| {},
        )
        .await;
        assert_eq!(
            finished.update.status, "completed",
            "{:?}",
            finished.update.error_message
        );
        assert_eq!(finished.update.content, "本机备用答案");
        let detail = db.run_detail(&ws, &run).unwrap();
        assert_eq!(detail.run.model_id, "backup");
        assert_eq!(detail.primary_snapshot.unwrap().request, original);
        assert_eq!(detail.snapshot.unwrap().request["model"], "backup");
        assert_eq!(
            detail.context_manifest["modelFallback"]["primaryModel"],
            "remote"
        );
        assert!(
            detail
                .events
                .iter()
                .any(|event| event.kind == "model_fallback")
        );
        assert_eq!(
            detail
                .events
                .iter()
                .filter(|event| event.kind == "model_request_started")
                .count(),
            2
        );
        served_cloud.join().unwrap();
        served_local.join().unwrap();
    }
}
