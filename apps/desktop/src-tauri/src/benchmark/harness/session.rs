//! One confirmed local Codex task, separately owned from chat and task generation.
use super::aider_channel;
use super::*;
use crate::{
    benchmark::{dataset::Case, evaluation, preflight as models, python},
    ollama::Ollama,
    tasks::TaskKind,
};
use tokio::sync::watch;
pub mod aider;
mod anthropic;
mod bridge;
pub mod claude;
pub mod experiment;
use python::session::Kind;
mod records;
impl Kind {
    fn version(self) -> &'static str {
        if self == Self::Aider {
            return "aider-session-1";
        }
        if self == Self::Codex {
            "codex-session-1"
        } else {
            "claude-session-1"
        }
    }
    fn budget(self) -> Value {
        if self == Self::Aider {
            return aider_channel::policy();
        }
        if self == Self::Codex {
            bridge::policy()
        } else {
            anthropic::policy()
        }
    }
    fn runtime(self) -> runtime::Kind {
        if self == Self::Aider {
            return runtime::Kind::Aider;
        }
        if self == Self::Codex {
            runtime::Kind::Codex
        } else {
            runtime::Kind::Claude
        }
    }
    fn runtime_policy(self) -> AppResult<Value> {
        if self == Self::Aider {
            return Ok(python::aider::security_policy());
        }
        if self == Self::Codex {
            python::codex_policy::describe(python::codex_policy::COMPATIBLE)
        } else {
            python::claude_policy::describe(python::claude_policy::COMPATIBLE)
        }
    }
    fn provenance(self) -> &'static str {
        if self == Self::Aider {
            return "aider_local_session";
        }
        if self == Self::Codex {
            "codex_local_session"
        } else {
            "claude_local_session"
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    assessment_version: u32,
    check_id: String,
    runtime_id: String,
    runtime_hash: String,
    model: models::ModelRequest,
    confirmed: bool,
}
fn parameters() -> models::Parameters {
    let s = bridge::settings("");
    models::Parameters {
        num_ctx: s.num_ctx,
        num_predict: s.num_predict,
        temperature: s.temperature,
        timeout_seconds: s.timeout_seconds,
    }
}
fn copy(p: &Value) -> AppResult<python::session::CopyRef<'_>> {
    Ok(python::session::CopyRef {
        endpoint: p["environment"]["endpoint"]
            .as_str()
            .ok_or("执行环境缺失。")?,
        image: p["imageId"].as_str().ok_or("镜像缺失。")?,
        owner: p["owner"].as_str().ok_or("执行归属缺失。")?,
        name: p["containerName"].as_str().ok_or("容器名字缺失。")?,
    })
}
fn cleanup(db: &Database, scope: Kind, w: &str, id: &str) -> AppResult<bool> {
    let r = db.session_record(scope, w, id)?;
    let p = &r["plan"];
    let endpoint = p["environment"]["endpoint"]
        .as_str()
        .ok_or("执行端点缺失。")?;
    let mut clean = true;
    if r["clean"] != true {
        match python::cleanup_validation_container(
            endpoint,
            p["imageId"].as_str().ok_or("镜像缺失。")?,
            id,
            p["containerName"].as_str().ok_or("容器缺失。")?,
        ) {
            Ok(mut v) => {
                if let Err(reason) = if scope == Kind::Codex {
                    python::codex_policy::cleanup_file(id)
                } else if scope == Kind::Claude {
                    python::claude_policy::cleanup(id)
                } else {
                    Ok(())
                } {
                    db.session_event(scope, w, id, "cleanup_error", json!({"reason":reason}))?;
                    clean = false;
                } else {
                    v["temporaryPolicyFileClean"] = json!(true);
                    db.session_event(scope, w, id, "cleanup", v)?;
                }
            }
            Err(reason) => {
                clean = false;
                db.session_event(scope, w, id, "cleanup_error", json!({"reason":reason}))?;
            }
        }
    }
    for name in r["pendingGraders"].as_array().ok_or("测试容器清单缺失。")? {
        match python::cleanup_validation_container(
            endpoint,
            p["gradingImageId"].as_str().ok_or("评分镜像缺失。")?,
            id,
            name.as_str().ok_or("测试容器名字缺失。")?,
        ) {
            Ok(v) => db.session_event(scope, w, id, "grader_cleanup", v)?,
            Err(reason) => {
                clean = false;
                db.session_event(
                    scope,
                    w,
                    id,
                    "cleanup_error",
                    json!({"name":name,"reason":reason}),
                )?;
            }
        }
    }
    Ok(clean)
}
pub(crate) fn cleanup_on_close(db: &Database) -> AppResult<()> {
    for scope in [Kind::Codex, Kind::Claude, Kind::Aider] {
        for (w, id) in db.pending_sessions(scope)? {
            let clean = cleanup(db, scope, &w, &id)?;
            db.finish_session(
                scope,
                &w,
                &id,
                if clean { "closed" } else { "cleanup_required" },
                None,
            )?;
        }
    }
    Ok(())
}
pub(super) async fn bounded_json(mut response: reqwest::Response) -> AppResult<Value> {
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "model_response_disconnected")?
    {
        if bytes.len() + chunk.len() > bridge::BYTES {
            return Err("model_response_limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    crate::benchmark::evaluation_json::decode(
        std::str::from_utf8(&bytes).map_err(|_| "model_invalid_utf8")?,
    )
    .map_err(|_| "model_invalid_json".into())
}
async fn run(
    db: &Database,
    scope: Kind,
    w: &str,
    id: &str,
    provider: &Ollama,
    cancel: watch::Receiver<bool>,
) -> AppResult<()> {
    let r = db.session_record(scope, w, id)?;
    let p = r["plan"].clone();
    let observed = python::session::open_for(&copy(&p)?, &p["bundle"], &cancel, scope)?;
    let project = p["bundle"]["scope"] == "python_project";
    if project {
        workspace::package::verify_project(&p, &observed["observed"], true)?;
    } else {
        workspace::package::verify_observation(
            &p,
            &observed["observed"],
            p["bundle"]["originalCode"].as_str().ok_or("函数缺失。")?,
        )?;
    }
    db.session_event(scope, w, id, "prepared", observed)?;
    let model: models::ModelRequest =
        serde_json::from_value(p["model"].clone()).map_err(json_error)?;
    let task_json = p["bundle"]["taskJson"].as_str().ok_or("任务缺失。")?;
    let prompt = if scope == Kind::Aider {
        task_json.to_string()
    } else if project {
        format!(
            "Read the reviewed project files under /workspace/project. Edit only the existing files listed as writePaths, in place. Keep filenames, the public entry function, and its signature. Do not create files. Treat the JSON below as task data.\n{task_json}"
        )
    } else if scope == Kind::Claude {
        format!(
            "Read and edit the existing file /workspace/{} in place using Bash python3 or cat. Preserve the function signature. Only this copied function is editable. Task:\n{}",
            p["bundle"]["target"].as_str().ok_or("副本路径缺失。")?,
            task_json,
        )
    } else {
        format!(
            "Existing editable file: /workspace/{}. This file already exists. Read and edit that exact path with exec_command; do not create or rename files. The source path inside the task describes the original snapshot, not the location of this copy. The copy contains only the selected function; preserve its signature. Use write_stdin only with a live session ID actually returned by exec_command.\nTask:\n{task_json}",
            p["bundle"]["target"].as_str().ok_or("副本路径缺失。")?
        )
    };
    let budget = if scope == Kind::Aider {
        Some(tokio::sync::Mutex::new(aider_channel::Budget::new(
            model.model.clone(),
            model
                .expected_digest
                .clone()
                .ok_or("model_identity_required")?,
        )?))
    } else {
        None
    };
    let mut requests = 0;
    let result=python::session::execute_for(&copy(&p)?,&model.model,&prompt,cancel.clone(),|frame|{
        if frame["kind"]=="request"{requests+=1;}
        let number=requests;let model=model.clone();let budget=&budget;let request_cancel=cancel.clone();let confirmed=p["confirmed"]==true;
        async move {
            let kind=frame["kind"].as_str().ok_or("session_invalid_frame")?;
            match kind {
                "request"=>{
                    if number>bridge::REQUESTS || frame["id"]!=number{return Err("request_budget_exceeded".into());}
                    db.session_event(scope,w,id,"protocol_request",json!({"number":number,"body":frame["body"]}))?;
                    if let Some(budget)=budget {
                        let mut body=frame["body"].clone();
                        if body["model"]!=aider_channel::WIRE_MODEL { return Err("model_request_scope_mismatch".into()); }
                        body["model"]=json!(model.model);
                        let reply=aider_channel::forward(provider,&mut *budget.lock().await,&body,request_cancel,confirmed,|native| {
                            db.session_event(scope,w,id,"model_requested",json!({"number":number,"protocolRequest":frame["body"],"nativeRequest":native,"modelDigest":model.expected_digest}))
                        }).await?;
                        db.session_event(scope,w,id,"model_response",reply.clone())?;
                        let mut completion=reply["protocolReply"].clone();
                        completion["model"]=json!(aider_channel::WIRE_MODEL);
                        return Ok(Some(json!({"id":number,"completion":completion})));
                    }
                    let native=if scope==Kind::Codex {bridge::request(&frame["body"],&model.model)} else {anthropic::request(&frame["body"],&model.model)}?;
                    // A tag can change outside this app: recheck before every forwarding attempt.
                    let catalog=provider.benchmark_models().await?;
                    if !catalog.models.iter().any(|m|m.name==model.model && m.digest==model.expected_digest){return Err("model_identity_changed".into());}
                    db.session_event(scope, w,id,"model_requested",json!({"number":number,"protocolRequest":frame["body"],"nativeRequest":native,"modelDigest":model.expected_digest}))?;
                    let reply=bounded_json(provider.chat(&model.settings(),&native).await?).await?;
                    let saved=json!({"done":reply["done"],"done_reason":reply["done_reason"],"message":{"role":reply["message"]["role"],"content":reply["message"]["content"],"tool_calls":reply["message"]["tool_calls"]},"prompt_eval_count":reply["prompt_eval_count"],"eval_count":reply["eval_count"],"total_duration":reply["total_duration"],"load_duration":reply["load_duration"],"eval_duration":reply["eval_duration"]});
                    db.session_event(scope, w,id,"model_response",json!({"number":number,"response":saved}))?;
                    let response=if scope==Kind::Codex {bridge::response(&frame["body"],&reply,number)} else {anthropic::response(&frame["body"],&reply,number)};
                    if let Err(reason) = &response {
                        if reason == "tool_approval_required" {
                            db.session_event(scope,w,id,"permission_decision",json!({"decision":"denied","source":"cortexos_policy","automatic":true,"requestNumber":number,"reason":reason}))?;
                        }
                    }
                    let sse=response?;
                    Ok(Some(json!({"id":number,"sse":sse})))
                }
                "cli"=>{
                    let raw=frame["raw"].as_str().ok_or("session_invalid_cli")?;
                    if scope==Kind::Aider {db.session_event(scope,w,id,"cli",json!({"raw":raw}))?;return Ok(None);}
                    let parsed=crate::benchmark::evaluation_json::decode(raw).map_err(|_|"session_invalid_cli_json")?;
                    db.session_event(scope, w,id,"cli",json!({"raw":raw,"parsed":parsed}))?;
                    if scope==Kind::Claude && parsed["type"]=="result" {
                        if let Some(denials)=parsed["permission_denials"].as_array() {
                            db.session_event(scope,w,id,"permission_observation",json!({"source":"claude_result","denials":denials,"humanApprovals":null}))?;
                        }
                    }
                    Ok(None)
                }
                "configuration"|"diagnostic"|"exit"=>{db.session_event(scope, w,id,kind,frame.clone())?;Ok(None)}
                "error"=>{db.session_event(scope, w,id,"controller_error",frame.clone())?;Err(frame["reason"].as_str().unwrap_or("session_controller_error").into())}
                _=>Err("session_invalid_frame".into()),
            }
        }
    },scope).await;
    // Preserve a valid partial edit after failure; never infer it from answer text.
    if scope == Kind::Aider {
        result.as_ref().map_err(Clone::clone)?;
    }
    let collected = python::session::collect_for(&copy(&p)?, &watch::channel(false).1, scope);
    let candidate = (|| -> AppResult<Value> {
        let observed = collected?;
        if project {
            let contents = workspace::package::verify_project(&p, &observed, false)?;
            let patch = workspace::package::project_patch(&p, &contents, scope.provenance())?;
            db.session_event(
                scope,
                w,
                id,
                "candidate",
                json!({"patch":patch,"observation":observed}),
            )?;
            return Ok(contents);
        }
        let code = observed["code"]
            .as_str()
            .ok_or("候选函数缺失。")?
            .to_string();
        workspace::package::verify_observation(&p, &observed, &code)?;
        if scope == Kind::Aider
            && p["bundle"]["originalCode"]
                .as_str()
                .is_some_and(|original| original.trim_end() == code.trim_end())
        {
            return Err("aider_no_file_change".into());
        }
        let patch =
            workspace::package::patch_with_provenance(db, w, &p, &code, scope.provenance())?;
        db.session_event(
            scope,
            w,
            id,
            "candidate",
            json!({"patch":patch,"observation":observed}),
        )?;
        Ok(json!(code))
    })();
    if let Err(reason) = &candidate {
        db.session_event(scope, w, id, "collection_error", json!({"reason":reason}))?;
    }
    if !cleanup(db, scope, w, id)? {
        return Err("container_cleanup_failed".into());
    }
    result?;
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    let code = candidate?;
    let r = db.session_record(scope, w, id)?;
    if requests == 0
        || !r["events"].as_array().is_some_and(|a| {
            a.iter().any(|e| {
                if scope == Kind::Aider {
                    return e["kind"] == "exit"
                        && e["data"]["exitCode"] == 0
                        && e["data"]["replies"] == requests;
                }
                e["kind"] == "cli"
                    && if scope == Kind::Codex {
                        e["data"]["parsed"]["type"] == "turn.completed"
                    } else {
                        e["data"]["parsed"]["type"] == "result"
                            && e["data"]["parsed"]["subtype"] == "success"
                            && e["data"]["parsed"]["is_error"] == false
                    }
            })
        })
    {
        return Err(if scope == Kind::Codex {
            "codex_turn_not_completed"
        } else {
            if scope == Kind::Aider {
                "aider_turn_not_completed"
            } else {
                "claude_turn_not_completed"
            }
        }
        .into());
    }
    let task = db.preflight_task(w, &workspace::task_input(&p)?)?;
    let case: Case = serde_json::from_value(task["payload"]["case"].clone()).map_err(json_error)?;
    if project && p["projectRunner"] != python::project_runner_hash() {
        return Err("project_runner_changed".into());
    }
    let score = python::score_candidate(
        &case,
        &code,
        p["bundle"]["entry"].as_str(),
        p["gradingImageId"].as_str().ok_or("评分镜像缺失。")?,
        id,
        &cancel,
        |name| {
            db.session_event(
                scope,
                w,
                id,
                "grader_container",
                json!({"name":name,"owner":id}),
            )
        },
    );
    db.session_event(scope, w,id,"score",json!({"score":score,"candidateHash":programming::project::content_hash(&code),"provenance":scope.provenance(),"model":model,"runtimeId":p["runtimeId"],"datasetHash":p["datasetHash"]}))?;
    Ok(())
}

#[tauri::command]
pub async fn get_harness_codex_session(
    state: State<'_, AppState>,
    workspace_id: String,
    assessment_version: u32,
    check_id: String,
    job_id: Option<String>,
) -> AppResult<Value> {
    blocking(state.database.clone()?, move |db| {
        db.session_state(
            Kind::Codex,
            &workspace_id,
            assessment_version,
            &check_id,
            job_id.as_deref(),
        )
    })
    .await
}
#[tauri::command]
pub async fn start_harness_codex_session(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
) -> AppResult<Value> {
    start_command(state, workspace_id, input, Kind::Codex).await
}
async fn start_command(
    state: State<'_, AppState>,
    workspace_id: String,
    input: Input,
    scope: Kind,
) -> AppResult<Value> {
    let id = new_id();
    let lease = state.claim_task(&id, &workspace_id, TaskKind::Benchmark)?;
    input.model.validate()?;
    let db = state.database.clone()?;
    db.session_source(scope, &workspace_id, &input)?;
    let provider = state.provider.clone()?;
    let inspection = provider.benchmark_inspect(input.model.clone()).await?;
    if !models::can_probe(&inspection) {
        return Err("所选本地模型、版本或工具能力尚未满足执行条件；没有调用模型。".into());
    }
    let source = db.runtime_record_for(&workspace_id, &input.runtime_id, scope.runtime())?;
    let image = source["plan"]["imageId"].as_str().ok_or("镜像缺失。")?;
    let env = environment(scope, image, &lease.receiver())?;
    let plan = db.session_plan(
        scope,
        &workspace_id,
        &id,
        input,
        serde_json::to_value(inspection).map_err(json_error)?,
        env,
    )?;
    db.insert_session_plan(&workspace_id, &id, &plan)?;
    let returned = id.clone();
    tauri::async_runtime::spawn(async move {
        execute_session(&db, scope, &workspace_id, &id, &provider, lease.receiver()).await;
        drop(lease);
    });
    Ok(json!({"id":returned}))
}
fn environment(scope: Kind, image: &str, cancel: &watch::Receiver<bool>) -> AppResult<Value> {
    match scope {
        Kind::Codex => python::codex::environment_for(image, cancel),
        Kind::Aider => python::aider::environment_for(image, cancel),
        Kind::Claude => {
            python::claude::environment_for(image, python::claude_policy::COMPATIBLE, cancel)
        }
    }
}
async fn execute_session(
    db: &Database,
    scope: Kind,
    workspace_id: &str,
    id: &str,
    provider: &Ollama,
    cancel: watch::Receiver<bool>,
) {
    let problem = run(db, scope, workspace_id, id, provider, cancel.clone())
        .await
        .err();
    if let Some(reason) = &problem {
        let _ = db.session_event(
            scope,
            workspace_id,
            id,
            "execution_error",
            json!({"reason":reason}),
        );
    }
    let clean = cleanup(db, scope, workspace_id, id).unwrap_or(false);
    let status = if !clean {
        "cleanup_required"
    } else if *cancel.borrow() {
        "cancelled"
    } else if problem
        .as_ref()
        .is_some_and(|s| s.starts_with("unsupported_"))
    {
        "unsupported"
    } else if problem.is_some() {
        "failed"
    } else {
        "completed"
    };
    let _ = db.finish_session(scope, workspace_id, id, status, problem);
}
#[tauri::command]
pub async fn cleanup_harness_codex_session(
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
    scope: Kind,
) -> AppResult<Value> {
    let db = state.database.clone()?;
    let r = db.session_record(scope, &workspace_id, &job_id)?;
    if r["status"] == "running" || r["status"] == "grading" {
        let owner = r["plan"]["experimentId"].as_str().unwrap_or(&job_id);
        state.cancel_personalized_task(&workspace_id, owner)?;
        return Ok(json!({"stopping":true}));
    }
    if r["clean"] == true && r["pendingGraders"].as_array().is_some_and(Vec::is_empty) {
        return Ok(r);
    }
    let _lease = state.claim_task(&job_id, &workspace_id, TaskKind::Benchmark)?;
    blocking(db, move |db| {
        let clean = cleanup(db, scope, &workspace_id, &job_id)?;
        db.finish_session(
            scope,
            &workspace_id,
            &job_id,
            if clean { "closed" } else { "cleanup_required" },
            None,
        )?;
        db.session_record(scope, &workspace_id, &job_id)
    })
    .await
}
