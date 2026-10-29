use super::plan::Binding;
use crate::{
    benchmark::{
        evaluation::{Score, State},
        python,
        repository::{StartInput, Summary},
    },
    cloud::{
        CloudState,
        completion::{self, CompletionInput},
    },
    commands::{AppState, blocking},
    models::{AppResult, ExecutionPurpose, new_id},
    tasks::Lease,
};
use serde_json::json;
use std::sync::Arc;

pub fn request(
    case: &crate::benchmark::dataset::Case,
    system: &str,
    model: &str,
    max: u32,
) -> CompletionInput {
    let mut prompt = case.input.prompt.clone();
    for s in &case.fixture {
        prompt.push_str(&format!("\n\n资料 {}：{}\n", s.id, s.title));
        for (i, line) in s.lines.iter().enumerate() {
            prompt.push_str(&format!("{}: {}\n", i + 1, line));
        }
    }
    CompletionInput {
        model: model.into(),
        system: system.into(),
        prompt,
        max_output_tokens: max,
        schema: if case.scoring.id == "schema_semantic" {
            case.input.response_schema.clone()
        } else {
            None
        },
        tools: case.input.tools.clone(),
    }
}
pub async fn run(
    state: &AppState,
    cloud: &CloudState,
    workspace: &str,
    input: StartInput,
    binding: &Binding,
    lease: Arc<Lease>,
) -> AppResult<Summary> {
    let connection = cloud.guard()?;
    if !binding.matches(&connection) {
        return Err("云端连接已变化，请创建新评测计划。".into());
    }
    let db = state.database.clone()?;
    let intent = db.benchmark_intent(workspace, input)?;
    if let Some(old) = db.benchmark_existing(workspace, &intent)? {
        return Ok(old);
    }
    let request = request(
        &intent.case,
        intent.manifest["prompt_template"]["system"]
            .as_str()
            .ok_or("模板缺失。")?,
        &binding.model,
        intent.input.model.parameters.num_predict,
    );
    let (url, wire) = completion::wire(binding.provider, &request)?;
    let mut profile = json!({"provider":binding.provider,"binding":binding,"request":intent.input.model,"cloudRequest":wire,"endpoint":url,"capabilityStatus":"unverified_until_request","localSettings":"storage_compatibility_only_not_applied"});
    let mut blocked = None;
    if intent.case.scoring.id == "python_assertions" {
        let image = intent.input.python_image.clone();
        let cancel = lease.receiver();
        let observed = tauri::async_runtime::spawn_blocking(move || {
            python::environment(image.as_deref(), Some(&cancel))
        })
        .await
        .map_err(|_| "Docker检查异常。")?;
        match observed {
            Ok(p) => profile["sandbox"] = p,
            Err(e) => {
                profile["sandbox"] = json!({"availability":"unavailable","reason":e});
                blocked = Some(Score::state(State::ExecutionError, e));
            }
        }
    }
    if *lease.receiver().borrow() {
        return Err("评测已停止。".into());
    }
    let id = new_id();
    let prepared = db.prepare_benchmark_record(workspace, &id, &intent, &profile)?;
    lease.set_execution(&prepared.execution.run.id)?;
    let run = prepared.execution.run;
    if let Some(score) = blocked {
        db.execution_finish(
            &run,
            ExecutionPurpose::Benchmark,
            "",
            "failed",
            Some("Docker环境不可用，未发送云端请求。"),
            json!({}),
        )?;
        db.save_benchmark_score(workspace, &id, score)?;
        return Ok(db.benchmark_detail(workspace, &id)?.summary);
    }
    db.execution_event(&run,ExecutionPurpose::Benchmark,"model_request_started",json!({"provider":binding.provider,"endpoint":url,"requestHash":crate::benchmark::dataset::hash(wire.to_string().as_bytes())}))?;
    let mut cancel = lease.receiver();
    let result = tokio::select! {biased;_=cancel.changed()=>Err("用户已停止；服务商可能已经计费，用量未知。".to_string()),r=cloud.complete(&connection,&request)=>r.map_err(|s|format!("云端请求未完成：{s:?}。未自动重试。"))};
    match result {
        Ok(output) => {
            db.execution_event(
                &run,
                ExecutionPurpose::Benchmark,
                "model_response_received",
                json!({"provider":binding.provider,"status":200}),
            )?;
            if !output.calls.is_empty() {
                db.execution_event(&run,ExecutionPurpose::Benchmark,"native_tool_calls_received",json!({"calls":output.calls,"source":"provider_native_objects","toolsExecuted":false}))?;
            }
            let (i, o) = completion::tokens(binding.provider, &output.usage);
            let metrics = json!({"provider":binding.provider,"finishReason":output.finish_reason,"promptEvalCount":i,"evalCount":o,"rawUsage":output.usage,"clientTiming":{"totalMs":output.elapsed_ms},"tokensPerSecond":o.filter(|_|output.elapsed_ms>0).map(|n|n as f64*1000./output.elapsed_ms as f64),"requestHash":output.request_hash,"memoryAvailability":"unavailable"});
            db.execution_finish(
                &run,
                ExecutionPurpose::Benchmark,
                &output.text,
                "completed",
                None,
                metrics,
            )?;
        }
        Err(e) => {
            db.execution_finish(
                &run,
                ExecutionPurpose::Benchmark,
                "",
                if *cancel.borrow() {
                    "cancelled"
                } else {
                    "failed"
                },
                Some(&e),
                json!({"usageAvailability":"unknown","providerMayHaveBilled":true}),
            )?;
        }
    }
    let scope = workspace.to_string();
    let task = id.clone();
    let receiver = lease.receiver();
    blocking(db.clone(), move |db| {
        db.finish_benchmark_v2(&scope, &task, Some(receiver))
    })
    .await?;
    Ok(db.benchmark_detail(workspace, &id)?.summary)
}
