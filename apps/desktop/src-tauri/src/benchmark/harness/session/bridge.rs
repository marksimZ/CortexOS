//! Explicit, text/function-only Responses adapter. No network access lives here.
use crate::{benchmark::evaluation_json, models::ModelSettings};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) const REQUESTS: u64 = 4;
pub(super) const WALL_SECONDS: u64 = 180;
pub(super) const BYTES: usize = 65536;
pub(super) fn settings(model: &str) -> ModelSettings {
    ModelSettings {
        model: model.into(),
        num_ctx: 4096,
        num_predict: 512,
        temperature: 0.0,
        timeout_seconds: 120,
        ..Default::default()
    }
}
pub(super) fn policy() -> Value {
    json!({"version":"codex-local-chat-1","maxRequests":REQUESTS,"wallSeconds":WALL_SECONDS,"requestSeconds":120,"requestBytes":BYTES,"responseBytes":BYTES,"contextTokens":4096,"outputTokensPerRequest":512,"temperature":0,"keepAliveSeconds":0,"transport":"docker_stdio_container_loopback","protocol":"responses_text_functions_to_native_chat","streaming":"buffered_response","retry":false})
}
fn text(parts: &Value) -> Result<String, String> {
    if let Some(s) = parts.as_str() {
        return Ok(s.into());
    }
    let mut s = String::new();
    for p in parts.as_array().ok_or("unsupported_content")? {
        if !matches!(p["type"].as_str(), Some("input_text" | "output_text")) {
            return Err("unsupported_content".into());
        }
        s.push_str(p["text"].as_str().ok_or("invalid_content")?);
        s.push('\n');
    }
    Ok(s)
}
pub(super) fn schema(rule: &Value) -> Value {
    let mut result = rule.clone();
    if let Some(object) = result.as_object_mut() {
        for key in ["description", "title", "default"] {
            object.remove(key);
        }
        if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
            for value in properties.values_mut() {
                *value = schema(value);
            }
        }
        if let Some(items) = object.get_mut("items") {
            *items = schema(items);
        }
    }
    result
}
pub(super) fn request(body: &Value, model: &str) -> Result<Value, String> {
    if body.to_string().len() > BYTES
        || body["model"] != model
        || body["stream"] != true
        || body
            .get("previous_response_id")
            .is_some_and(|v| !v.is_null())
        || body.get("conversation").is_some_and(|v| !v.is_null())
    {
        return Err("model_request_scope_mismatch".into());
    }
    let mut messages = vec![];
    if let Some(s) = body["instructions"].as_str() {
        messages.push(json!({"role":"system","content":s}));
    }
    let input = body["input"]
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or("unsupported_input")?;
    let mut calls = BTreeSet::new();
    for item in input {
        match item["type"].as_str().unwrap_or("message") {
            "message" => {
                let role = match item["role"].as_str() {
                    Some("developer" | "system") => "system",
                    Some("user") => "user",
                    Some("assistant") => "assistant",
                    _ => return Err("unsupported_role".into()),
                };
                messages.push(json!({"role":role,"content":text(&item["content"])?}));
            }
            "function_call" => {
                let id = item["call_id"].as_str().ok_or("invalid_call_id")?;
                if !calls.insert(id.to_string()) {
                    return Err("duplicate_call_id".into());
                }
                let args = evaluation_json::decode(
                    item["arguments"].as_str().ok_or("invalid_arguments")?,
                )?;
                if !args.is_object() {
                    return Err("invalid_arguments".into());
                }
                messages.push(json!({"role":"assistant","content":"","tool_calls":[{"function":{"name":item["name"],"arguments":args}}]}));
            }
            "function_call_output" => {
                let id = item["call_id"].as_str().ok_or("invalid_call_id")?;
                if !calls.contains(id) {
                    return Err("unbound_tool_output".into());
                }
                let name = input
                    .iter()
                    .find(|c| c["type"] == "function_call" && c["call_id"] == id)
                    .ok_or("unbound_tool_output")?["name"]
                    .clone();
                messages
                    .push(json!({"role":"tool","tool_name":name,"content":text(&item["output"])?}));
            }
            _ => return Err("unsupported_input_item".into()),
        }
    }
    let tools = body["tools"]
        .as_array()
        .filter(|a| a.len() <= 8)
        .ok_or("unsupported_tools")?;
    let mut names = BTreeSet::new();
    let mut native_tools = vec![];
    for tool in tools {
        let name = tool["name"].as_str().ok_or("unsupported_tool")?;
        if tool["type"] != "function"
            || ![
                "exec_command",
                "write_stdin",
                "request_user_input",
                "view_image",
            ]
            .contains(&name)
            || !names.insert(name)
            || tool["parameters"]["type"] != "object"
        {
            return Err("unsupported_tool".into());
        }
        // This slice has text/source files only; do not advertise image or UI
        // interaction tools as capabilities of the local coding model channel.
        if ["view_image", "request_user_input"].contains(&name) {
            continue;
        }
        // Validate vocabulary before forwarding. Descriptions are annotations, not constraints.
        evaluation_json::schema_valid(&schema(&tool["parameters"]), &json!({}))
            .map_err(|_| "unsupported_tool_schema")?;
        native_tools.push(json!({"type":"function","function":{"name":name,"description":tool["description"],"parameters":tool["parameters"]}}));
    }
    let s = settings(model);
    Ok(
        json!({"model":s.model,"messages":messages,"tools":native_tools,"stream":false,"think":false,"keep_alive":0,"options":{"num_ctx":s.num_ctx,"num_predict":s.num_predict,"temperature":s.temperature}}),
    )
}

pub(super) fn response(body: &Value, native: &Value, sequence: u64) -> Result<String, String> {
    if native["done"] != true
        || native.get("error").is_some()
        || native["message"]["role"] != "assistant"
    {
        return Err("invalid_model_response".into());
    }
    if native["done_reason"] == "length" || native["eval_count"].as_u64().is_some_and(|n| n > 512) {
        return Err("model_output_limit".into());
    }
    let mut output = vec![];
    if let Some(s) = native["message"]["content"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        output.push(json!({"id":format!("msg_{sequence}"),"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":s,"annotations":[]}]}));
    }
    if let Some(calls) = native["message"].get("tool_calls").filter(|v| !v.is_null()) {
        for (i, call) in calls
            .as_array()
            .filter(|a| a.len() <= 4)
            .ok_or("invalid_tool_calls")?
            .iter()
            .enumerate()
        {
            let f = &call["function"];
            if f["name"] == "request_user_input" {
                return Err("tool_approval_required".into());
            }
            if f["name"] == "view_image" {
                return Err("unsupported_image_tool".into());
            }
            let tool = body["tools"]
                .as_array()
                .ok_or("unsupported_tools")?
                .iter()
                .find(|t| t["type"] == "function" && t["name"] == f["name"])
                .ok_or("unknown_tool_call")?;
            if !f["arguments"].is_object()
                || !evaluation_json::schema_valid(&schema(&tool["parameters"]), &f["arguments"])?
            {
                return Err("invalid_tool_arguments".into());
            }
            if f["arguments"]["sandbox_permissions"] == "require_escalated" {
                return Err("tool_approval_required".into());
            }
            output.push(json!({"id":format!("fc_{sequence}_{i}"),"type":"function_call","status":"completed","call_id":format!("call_{sequence}_{i}"),"name":f["name"],"arguments":f["arguments"].to_string()}));
        }
    }
    if output.is_empty() {
        return Err("empty_model_response".into());
    }
    let id = format!("resp_cortexos_{sequence}");
    let mut result = String::new();
    let mut emit = |value: Value| {
        result.push_str(&format!(
            "event: {}\ndata: {}\n\n",
            value["type"].as_str().unwrap(),
            value
        ));
    };
    emit(
        json!({"type":"response.created","response":{"id":id,"object":"response","status":"in_progress","output":[]}}),
    );
    for (index, item) in output.iter().enumerate() {
        emit(json!({"type":"response.output_item.added","output_index":index,"item":item}));
        emit(json!({"type":"response.output_item.done","output_index":index,"item":item}));
    }
    emit(
        json!({"type":"response.completed","response":{"id":id,"object":"response","status":"completed","output":output,"usage":{"input_tokens":native["prompt_eval_count"],"output_tokens":native["eval_count"],"total_tokens":native["prompt_eval_count"].as_u64().zip(native["eval_count"].as_u64()).map(|(a,b)|a+b)}}}),
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_bridge_limits_identity_and_uses_only_native_structured_calls() {
        let b = json!({"model":"fixture","stream":true,"input":[{"role":"user","content":[{"type":"input_text","text":"Task"}]}],"tools":[{"type":"function","name":"exec_command","parameters":{"type":"object","properties":{"cmd":{"type":"string","description":"Command text"}},"required":["cmd"],"additionalProperties":false}}]});
        let r = request(&b, "fixture").unwrap();
        assert_eq!(r["options"]["num_ctx"], 4096);
        assert_eq!(r["options"]["num_predict"], 512);
        assert_eq!(r["keep_alive"], 0);
        assert!(request(&b, "other").is_err());
        let mut bad = b.clone();
        bad["tools"][0]["type"] = json!("web_search");
        assert!(request(&bad, "fixture").is_err());
        bad = b.clone();
        bad["input"][0]["content"][0]["type"] = json!("input_image");
        assert!(request(&bad, "fixture").is_err());
        let plain = json!({"done":true,"message":{"role":"assistant","content":"exec_command({cmd: unsafe})"}});
        assert!(
            !response(&b, &plain, 1)
                .unwrap()
                .contains("\"type\":\"function_call\"")
        );
        let native = json!({"done":true,"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"exec_command","arguments":{"cmd":"pwd"}}}]}});
        assert!(response(&b, &native, 1).unwrap().contains("call_1_0"));
        let mut bad = native.clone();
        bad["message"]["tool_calls"][0]["function"]["arguments"]["cmd"] = json!(3);
        assert!(response(&b, &bad, 1).is_err());
        bad = native.clone();
        bad["message"]["tool_calls"][0]["function"]["name"] = json!("unknown");
        assert!(response(&b, &bad, 1).is_err());
        bad = native;
        bad["done_reason"] = json!("length");
        assert_eq!(response(&b, &bad, 1).unwrap_err(), "model_output_limit");
    }
}
