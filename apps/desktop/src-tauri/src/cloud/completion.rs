use super::{ConnectionState, Provider};
use crate::models::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletionInput {
    pub model: String,
    pub system: String,
    pub prompt: String,
    pub max_output_tokens: u32,
    pub schema: Option<Value>,
    pub tools: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionOutput {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    pub calls: Vec<Value>,
    pub usage: Value,
    pub finish_reason: String,
    pub elapsed_ms: u64,
    pub request_hash: String,
}
pub type CompletionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CompletionOutput, ConnectionState>> + Send + 'a>>;
pub fn wire(provider: Provider, input: &CompletionInput) -> AppResult<(String, Value)> {
    if input.model.is_empty()
        || input.model.len() > 200
        || !input
            .model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
        || !(64..=16384).contains(&input.max_output_tokens)
        || input.prompt.len() + input.system.len() > 128 * 1024
    {
        return Err("模型、输出长度或输入大小无效。".into());
    }
    let (url, mut body) = match provider {
        Provider::Openai => (
            "https://api.openai.com/v1/responses".to_string(),
            json!({"model":input.model,"instructions":input.system,"input":input.prompt,"max_output_tokens":input.max_output_tokens,"store":false}),
        ),
        Provider::Anthropic => (
            "https://api.anthropic.com/v1/messages".to_string(),
            json!({"model":input.model,"system":input.system,"messages":[{"role":"user","content":input.prompt}],"max_tokens":input.max_output_tokens}),
        ),
        Provider::Gemini => {
            let model = input.model.strip_prefix("models/").unwrap_or(&input.model);
            if model.contains('/') || model.contains(':') || model.starts_with('.') {
                return Err("模型标识无效。".into());
            }
            (
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
                ),
                json!({"systemInstruction":{"parts":[{"text":input.system}]},"contents":[{"role":"user","parts":[{"text":input.prompt}]}],"generationConfig":{"maxOutputTokens":input.max_output_tokens}}),
            )
        }
    };
    if let Some(schema) = &input.schema {
        match provider {
            Provider::Openai => {
                body["text"] = json!({"format":{"type":"json_schema","name":"benchmark_answer","schema":schema,"strict":true}})
            }
            Provider::Anthropic => {
                body["output_config"] = json!({"format":{"type":"json_schema","schema":schema}})
            }
            Provider::Gemini => {
                body["generationConfig"]["responseMimeType"] = json!("application/json");
                body["generationConfig"]["responseJsonSchema"] = schema.clone();
            }
        }
    }
    if !input.tools.is_empty() {
        match provider {
        Provider::Openai=>body["tools"]=json!(input.tools.iter().map(|t|json!({"type":"function","name":t["function"]["name"],"description":t["function"]["description"],"parameters":t["function"]["parameters"]})).collect::<Vec<_>>()),
        Provider::Anthropic=>body["tools"]=json!(input.tools.iter().map(|t|json!({"name":t["function"]["name"],"description":t["function"]["description"],"input_schema":t["function"]["parameters"]})).collect::<Vec<_>>()),
        Provider::Gemini=>body["tools"]=json!([{"functionDeclarations":input.tools.iter().map(|t|json!({"name":t["function"]["name"],"description":t["function"]["description"],"parameters":t["function"]["parameters"]})).collect::<Vec<_>>()}]),
    }
    }
    Ok((url, body))
}
pub fn parse(
    provider: Provider,
    body: &Value,
    elapsed_ms: u64,
    request_hash: String,
) -> Result<CompletionOutput, ConnectionState> {
    let mut text = String::new();
    let mut thinking = String::new();
    let mut calls = vec![];
    let call = |name: &Value, args: Value| json!({"function":{"name":name,"arguments":args}});
    let (usage, finish) = match provider {
        Provider::Openai => {
            for item in body["output"]
                .as_array()
                .ok_or(ConnectionState::ServiceError)?
            {
                if item["type"] == "message" {
                    for c in item["content"]
                        .as_array()
                        .ok_or(ConnectionState::ServiceError)?
                    {
                        if c["type"] == "output_text" {
                            text.push_str(c["text"].as_str().ok_or(ConnectionState::ServiceError)?);
                        }
                    }
                } else if item["type"] == "reasoning" {
                    for summary in item["summary"].as_array().into_iter().flatten() {
                        if let Some(text) = summary["text"].as_str() {
                            thinking.push_str(text);
                        }
                    }
                } else if item["type"] == "function_call" {
                    calls.push(call(
                        &item["name"],
                        crate::benchmark::evaluation_json::decode(
                            item["arguments"]
                                .as_str()
                                .ok_or(ConnectionState::ServiceError)?,
                        )
                        .map_err(|_| ConnectionState::ServiceError)?,
                    ));
                }
            }
            (
                body["usage"].clone(),
                if body["status"] == "completed" {
                    "stop"
                } else if body.pointer("/incomplete_details/reason")
                    == Some(&json!("max_output_tokens"))
                {
                    "length"
                } else {
                    return Err(ConnectionState::ServiceError);
                },
            )
        }
        Provider::Anthropic => {
            for item in body["content"]
                .as_array()
                .ok_or(ConnectionState::ServiceError)?
            {
                if item["type"] == "text" {
                    text.push_str(item["text"].as_str().ok_or(ConnectionState::ServiceError)?);
                } else if item["type"] == "thinking" {
                    if let Some(text) = item["thinking"].as_str() {
                        thinking.push_str(text);
                    }
                } else if item["type"] == "tool_use" {
                    calls.push(call(&item["name"], item["input"].clone()));
                }
            }
            (
                body["usage"].clone(),
                match body["stop_reason"].as_str() {
                    Some("max_tokens") => "length",
                    Some("end_turn" | "tool_use" | "stop_sequence") => "stop",
                    _ => return Err(ConnectionState::ServiceError),
                },
            )
        }
        Provider::Gemini => {
            let first = body["candidates"]
                .as_array()
                .and_then(|a| a.first())
                .ok_or(ConnectionState::ServiceError)?;
            for part in first["content"]["parts"]
                .as_array()
                .ok_or(ConnectionState::ServiceError)?
            {
                if part["thought"] == true {
                    if let Some(text) = part["text"].as_str() {
                        thinking.push_str(text);
                    }
                    continue;
                }
                if let Some(t) = part["text"].as_str() {
                    text.push_str(t);
                }
                if part["functionCall"].is_object() {
                    calls.push(call(
                        &part["functionCall"]["name"],
                        part["functionCall"]["args"].clone(),
                    ));
                }
            }
            (
                body["usageMetadata"].clone(),
                match first["finishReason"].as_str() {
                    Some("MAX_TOKENS") => "length",
                    Some("STOP") => "stop",
                    _ => return Err(ConnectionState::ServiceError),
                },
            )
        }
    };
    if text.is_empty() && calls.is_empty() {
        return Err(ConnectionState::ServiceError);
    }
    if calls.len() > 32 || thinking.len() > 1 << 20 {
        return Err(ConnectionState::ServiceError);
    }
    Ok(CompletionOutput {
        text,
        thinking: (!thinking.is_empty()).then_some(thinking),
        calls,
        usage,
        finish_reason: finish.into(),
        elapsed_ms,
        request_hash,
    })
}
pub async fn official(
    provider: Provider,
    key: &str,
    input: &CompletionInput,
) -> Result<CompletionOutput, ConnectionState> {
    let (url, body) = wire(provider, input).map_err(|_| ConnectionState::ServiceError)?;
    let start = Instant::now();
    let request_hash = crate::benchmark::dataset::hash(
        &serde_json::to_vec(&body).map_err(|_| ConnectionState::ServiceError)?,
    );
    let body = post_json(
        provider,
        key,
        &url,
        &body,
        Duration::from_secs(120),
        2 << 20,
    )
    .await?;
    let output = parse(
        provider,
        &body,
        start.elapsed().as_millis() as u64,
        request_hash,
    )?;
    checked_output(output, key)
}

pub(super) fn checked_output(
    output: CompletionOutput,
    key: &str,
) -> Result<CompletionOutput, ConnectionState> {
    if output.text.contains(key)
        || output
            .thinking
            .as_deref()
            .is_some_and(|text| text.contains(key))
        || serde_json::to_string(&output)
            .map_err(|_| ConnectionState::ServiceError)?
            .contains(key)
    {
        return Err(ConnectionState::ServiceError);
    }
    Ok(output)
}

pub(super) async fn post_json(
    provider: Provider,
    key: &str,
    url: &str,
    body: &Value,
    timeout: Duration,
    response_limit: usize,
) -> Result<Value, ConnectionState> {
    let client = super::transport::JSON_CLIENT.as_ref().map_err(|error| *error)?;
    let request = authenticate(client.post(url).timeout(timeout).json(body), provider, key)?;
    let mut response = request
        .send()
        .await
        .map_err(|_| ConnectionState::NetworkUnavailable)?;
    let status = response.status().as_u16();
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ConnectionState::NetworkUnavailable)?
    {
        if bytes.len() + chunk.len() > response_limit {
            return Err(ConnectionState::ServiceError);
        }
        bytes.extend_from_slice(&chunk);
    }
    if status != 200 {
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        return Err(super::provider::classify(status, &body));
    }
    let body: Value = serde_json::from_slice(&bytes).map_err(|_| ConnectionState::ServiceError)?;
    if serde_json::to_string(&body)
        .map_err(|_| ConnectionState::ServiceError)?
        .contains(key)
    {
        return Err(ConnectionState::ServiceError);
    }
    Ok(body)
}
pub(super) fn authenticate(request: reqwest::RequestBuilder, provider: Provider, key: &str) -> Result<reqwest::RequestBuilder, ConnectionState> {
    let mut secret = reqwest::header::HeaderValue::from_str(key).map_err(|_| ConnectionState::InvalidCredential)?;
    secret.set_sensitive(true);
    Ok(match provider {
        Provider::Openai => request.bearer_auth(key),
        Provider::Anthropic => request.header("x-api-key", secret).header("anthropic-version", "2023-06-01"),
        Provider::Gemini => request.header("x-goog-api-key", secret),
    })
}

pub fn tokens(provider: Provider, usage: &Value) -> (Option<u64>, Option<u64>) {
    match provider {
        Provider::Openai => (
            usage["input_tokens"].as_u64(),
            usage["output_tokens"].as_u64(),
        ),
        Provider::Anthropic => (
            usage["input_tokens"].as_u64().and_then(|n| {
                n.checked_add(usage["cache_creation_input_tokens"].as_u64().unwrap_or(0))?
                    .checked_add(usage["cache_read_input_tokens"].as_u64().unwrap_or(0))
            }),
            usage["output_tokens"].as_u64(),
        ),
        Provider::Gemini => (
            usage["promptTokenCount"].as_u64(),
            usage["candidatesTokenCount"]
                .as_u64()
                .and_then(|n| n.checked_add(usage["thoughtsTokenCount"].as_u64().unwrap_or(0))),
        ),
    }
}
