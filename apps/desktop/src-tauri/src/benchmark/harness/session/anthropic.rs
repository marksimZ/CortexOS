//! Bounded Messages text/Bash adapter. All network access stays in the Provider.
use super::bridge;
use crate::benchmark::evaluation_json;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn policy() -> Value {
    let mut p = bridge::policy();
    p["version"] = json!("claude-local-chat-1");
    p["protocol"] = json!("messages_text_bash_to_native_chat");
    p
}
fn text(v: &Value) -> Result<String, String> {
    if let Some(s) = v.as_str() {
        return Ok(s.into());
    }
    let mut out = String::new();
    for p in v.as_array().ok_or("unsupported_content")? {
        if p["type"] != "text" {
            return Err("unsupported_content".into());
        }
        out.push_str(p["text"].as_str().ok_or("invalid_content")?);
        out.push('\n');
    }
    Ok(out)
}
fn arguments(body: &Value, args: &Value) -> Result<(), String> {
    let tool = body["tools"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["name"] == "Bash"))
        .ok_or("unknown_tool_call")?;
    if args["dangerouslyDisableSandbox"] == true {
        return Err("tool_approval_required".into());
    }
    if !args.is_object()
        || !evaluation_json::schema_valid(&bridge::schema(&tool["input_schema"]), args)?
        || args["command"].as_str().is_none_or(|s| s.is_empty())
        || args.get("run_in_background").is_some_and(|v| v != false)
        || args
            .get("dangerouslyDisableSandbox")
            .is_some_and(|v| v != false)
    {
        return Err("invalid_tool_arguments".into());
    }
    Ok(())
}
pub(super) fn request(body: &Value, model: &str) -> Result<Value, String> {
    if body.to_string().len() > bridge::BYTES
        || body["model"] != model
        || body["stream"] != true
        || body
            .get("thinking")
            .is_some_and(|v| !v.is_null() && v["type"] != "disabled")
        || body
            .get("tool_choice")
            .is_some_and(|v| !v.is_null() && v["type"] != "auto")
    {
        return Err("model_request_scope_mismatch".into());
    }
    let tools = body["tools"]
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or("unsupported_tools")?;
    if tools[0]["name"] != "Bash" || tools[0]["input_schema"]["type"] != "object" {
        return Err("unsupported_tool".into());
    }
    evaluation_json::schema_valid(&bridge::schema(&tools[0]["input_schema"]), &json!({}))?;
    let mut messages = vec![];
    if let Some(system) = body.get("system") {
        messages.push(json!({"role":"system","content":text(system)?}));
    }
    let mut pending = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for m in body["messages"]
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or("unsupported_input")?
    {
        let role = m["role"]
            .as_str()
            .filter(|s| matches!(*s, "user" | "assistant"))
            .ok_or("unsupported_role")?;
        if let Some(s) = m["content"].as_str() {
            if !pending.is_empty() {
                return Err("missing_tool_output".into());
            }
            messages.push(json!({"role":role,"content":s}));
            continue;
        }
        let mut content = String::new();
        let mut calls = vec![];
        for p in m["content"].as_array().ok_or("unsupported_content")? {
            match p["type"].as_str() {
                Some("text") => {
                    content.push_str(p["text"].as_str().ok_or("invalid_content")?);
                    content.push('\n');
                }
                Some("tool_use") if role == "assistant" => {
                    let id = p["id"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .ok_or("invalid_call_id")?;
                    if p["name"] != "Bash" || !seen.insert(id.to_string()) {
                        return Err("invalid_tool_call".into());
                    }
                    arguments(body, &p["input"])?;
                    pending.insert(id.to_string());
                    calls.push(json!({"function":{"name":"Bash","arguments":p["input"]}}));
                }
                Some("tool_result") if role == "user" => {
                    if !pending.remove(p["tool_use_id"].as_str().ok_or("invalid_call_id")?) {
                        return Err("unbound_tool_output".into());
                    }
                    messages.push(
                        json!({"role":"tool","tool_name":"Bash","content":text(&p["content"])?}),
                    );
                }
                _ => return Err("unsupported_content".into()),
            }
        }
        if !content.is_empty() || !calls.is_empty() {
            messages.push(json!({"role":role,"content":content,"tool_calls":calls}));
        }
    }
    if !pending.is_empty() {
        return Err("missing_tool_output".into());
    }
    let s = bridge::settings(model);
    Ok(
        json!({"model":model,"messages":messages,"tools":[{"type":"function","function":{"name":"Bash","description":tools[0]["description"],"parameters":tools[0]["input_schema"]}}],"stream":false,"think":false,"keep_alive":0,"options":{"num_ctx":s.num_ctx,"num_predict":s.num_predict,"temperature":s.temperature}}),
    )
}
pub(super) fn response(body: &Value, native: &Value, sequence: u64) -> Result<String, String> {
    if native.to_string().len() > bridge::BYTES
        || native["done"] != true
        || native.get("error").is_some()
        || native["message"]["role"] != "assistant"
    {
        return Err("invalid_model_response".into());
    }
    let input = native["prompt_eval_count"]
        .as_u64()
        .ok_or("model_usage_missing")?;
    let output = native["eval_count"].as_u64().ok_or("model_usage_missing")?;
    if native["done_reason"] == "length" || output > 512 {
        return Err("model_output_limit".into());
    }
    let mut blocks = vec![];
    if let Some(s) = native["message"]["content"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        blocks.push(json!({"type":"text","text":s}));
    }
    if let Some(calls) = native["message"].get("tool_calls").filter(|v| !v.is_null()) {
        for (i, c) in calls
            .as_array()
            .filter(|a| a.len() <= 4)
            .ok_or("invalid_tool_calls")?
            .iter()
            .enumerate()
        {
            if c["function"]["name"] != "Bash" {
                return Err("unknown_tool_call".into());
            }
            arguments(body, &c["function"]["arguments"])?;
            blocks.push(json!({"type":"tool_use","id":format!("toolu_cortexos_{sequence}_{i}"),"name":"Bash","input":c["function"]["arguments"]}));
        }
    }
    if blocks.is_empty() {
        return Err("empty_model_response".into());
    }
    let has_tools = blocks.iter().any(|b| b["type"] == "tool_use");
    let mut result = String::new();
    let mut emit = |v: Value| {
        result.push_str(&format!(
            "event: {}\ndata: {}\n\n",
            v["type"].as_str().unwrap(),
            v
        ))
    };
    emit(
        json!({"type":"message_start","message":{"id":format!("msg_cortexos_{sequence}"),"type":"message","role":"assistant","model":body["model"],"content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":input,"output_tokens":0}}}),
    );
    for (index, b) in blocks.iter().enumerate() {
        let mut start = b.clone();
        let delta = if b["type"] == "text" {
            start["text"] = json!("");
            json!({"type":"text_delta","text":b["text"]})
        } else {
            start["input"] = json!({});
            json!({"type":"input_json_delta","partial_json":b["input"].to_string()})
        };
        emit(json!({"type":"content_block_start","index":index,"content_block":start}));
        emit(json!({"type":"content_block_delta","index":index,"delta":delta}));
        emit(json!({"type":"content_block_stop","index":index}));
    }
    emit(
        json!({"type":"message_delta","delta":{"stop_reason":if has_tools{"tool_use"}else{"end_turn"},"stop_sequence":null},"usage":{"output_tokens":output}}),
    );
    emit(json!({"type":"message_stop"}));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_bridge_preserves_bounds_and_rejects_unbound_or_unsafe_tools() {
        let b = json!({"model":"fixture","stream":true,"messages":[{"role":"user","content":"Task"}],"tools":[{"name":"Bash","input_schema":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false}}]});
        let r = request(&b, "fixture").unwrap();
        assert_eq!(r["options"]["num_predict"], 512);
        assert_eq!(r["keep_alive"], 0);
        assert!(request(&b, "other").is_err());
        let n = json!({"done":true,"prompt_eval_count":12,"eval_count":4,"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"Bash","arguments":{"command":"cat /workspace/project/task.py"}}}]}});
        assert!(response(&b, &n, 1).unwrap().contains("toolu_cortexos_1_0"));
        let mut bad = n.clone();
        bad["message"]["tool_calls"][0]["function"]["name"] = json!("Other");
        assert!(response(&b, &bad, 1).is_err());
        bad = n.clone();
        bad["message"]["tool_calls"][0]["function"]["arguments"]["command"] = json!(7);
        assert!(response(&b, &bad, 1).is_err());
        bad = n.clone();
        bad["message"]["tool_calls"][0]["function"]["arguments"]["dangerouslyDisableSandbox"] =
            json!(true);
        assert!(response(&b, &bad, 1).is_err());
        bad = n.clone();
        bad["eval_count"] = Value::Null;
        assert!(response(&b, &bad, 1).is_err());
        bad = n;
        bad["done_reason"] = json!("length");
        assert!(response(&b, &bad, 1).is_err());
        let mut bad = b.clone();
        bad["messages"] = json!([{"role":"user","content":[{"type":"tool_result","tool_use_id":"missing","content":"x"}]}]);
        assert!(request(&bad, "fixture").is_err());
        let plain = json!({"done":true,"prompt_eval_count":12,"eval_count":4,"message":{"role":"assistant","content":"Bash(command)"}});
        assert!(!response(&b, &plain, 1).unwrap().contains("tool_use"));
    }
}
