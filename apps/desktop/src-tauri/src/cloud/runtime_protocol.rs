use super::{
    ConnectionState, Provider, StreamChunk,
    completion::{self, CompletionInput, CompletionOutput, post_json},
    provider::{Model, catalog},
};
use crate::{
    model_runtime::normalized_embeddings,
    models::{ChatMessage, GenerationOptions, MessageImage},
};
use reqwest::Url;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) type CloudResult<T> = Result<T, ConnectionState>;
const INVALID: ConnectionState = ConnectionState::ServiceError;

pub(super) enum Endpoint {
    Official(Provider),
    Compatible { base: Url, rerank: bool },
}

fn image_data_url(image: &MessageImage) -> String {
    format!("data:{};base64,{}", image.mime_type, image.data_base64)
}

impl Endpoint {
    pub(super) fn compatible(address: &str, rerank: bool) -> Result<Self, String> {
        let mut base = Url::parse(address).map_err(|_| "请输入完整的 HTTPS 服务地址。")?;
        if address.len() > 500
            || base.scheme() != "https"
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.host_str().is_none()
        {
            return Err("请使用不含账号、密钥或查询参数的 HTTPS 服务地址。".into());
        }
        let path = base.path().to_owned();
        if !path.ends_with('/') {
            base.set_path(&format!("{path}/"));
        }
        Ok(Self::Compatible { base, rerank })
    }

    pub(super) fn base(&self) -> Option<&Url> {
        match self {
            Self::Official(_) => None,
            Self::Compatible { base, .. } => Some(base),
        }
    }

    pub(super) fn is_deepseek(&self) -> bool {
        self.base().is_some_and(|base| {
            base.host_str() == Some("api.deepseek.com")
                && base.port().is_none()
                && matches!(base.path(), "/" | "/v1/")
        })
    }

    fn protocol(&self) -> Provider {
        match self {
            Self::Official(provider) => *provider,
            Self::Compatible { .. } => Provider::Openai,
        }
    }

    pub(super) fn generation_tokens(&self, usage: &Value) -> (Option<u64>, Option<u64>) {
        completion::tokens(self.protocol(), usage)
    }

    pub(super) async fn catalog(&self, key: &str) -> CloudResult<Vec<Model>> {
        let mut models = catalog(self.protocol(), key, self.base()).await?;
        if matches!(self, Self::Compatible { rerank: false, .. }) {
            for model in &mut models {
                model.reranking = "unsupported";
            }
        }
        Ok(models)
    }

    pub(super) async fn probe_generation(
        &self,
        key: &str,
        input: &CompletionInput,
        timeout: Duration,
    ) -> CloudResult<CompletionOutput> {
        let messages = [ChatMessage {
            role: "user".into(),
            content: input.prompt.clone(),
            images: Vec::new(),
        }];
        let (url, mut request) = self.chat_wire(&input.model, &messages, input.max_output_tokens)?;
        // Leave the production request at the model's default thinking setting.
        // The 64-token probe needs room for the visible answer on DeepSeek.
        if self.is_deepseek() && matches!(
            input.model.as_str(),
            "deepseek-flash" | "deepseek-v4-pro" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
        )
        {
            request["thinking"] = json!({"type":"disabled"});
        }
        self.send_generation_stream(key, &url, &request, timeout, &mut |_| {}).await
    }

    #[cfg(test)]
    pub(super) fn generation_wire(&self, input: &CompletionInput) -> CloudResult<(String, Value)> {
        let (mut url, mut request) =
            completion::wire(self.protocol(), input).map_err(|_| INVALID)?;
        if let Some(base) = self.base() {
            url = base
                .join("chat/completions")
                .map_err(|_| INVALID)?
                .to_string();
            request = json!({
                "model":input.model,
                "messages":[{"role":"system","content":input.system},{"role":"user","content":input.prompt}],
                "max_tokens":input.max_output_tokens,
            });
            if let Some(schema) = &input.schema {
                request["response_format"] = json!({"type":"json_schema","json_schema":{"name":"cortex_answer","strict":true,"schema":schema}});
            }
            if !input.tools.is_empty() {
                request["tools"] = json!(input.tools);
            }
        }
        Ok((url, request))
    }

    pub(super) fn generation_chat_wire(
        &self,
        model: &str,
        messages: &[ChatMessage],
        max_output_tokens: u32,
        options: Option<GenerationOptions>,
    ) -> CloudResult<(String, Value)> {
        let (url, mut body) = self.chat_wire(model, messages, max_output_tokens)?;
        let Some(options) = options else {
            return Ok((url, body));
        };
        match self.protocol() {
            Provider::Openai => {
                body["temperature"] = json!(options.temperature);
                body["top_p"] = json!(options.top_p);
                if options.deep_thinking {
                    if self.is_deepseek() {
                        body["thinking"] = json!({"type": "enabled"});
                    } else if self.base().is_some() {
                        body["reasoning_effort"] = json!("medium");
                    } else {
                        body["reasoning"] = json!({"effort": "medium"});
                    }
                }
            }
            Provider::Anthropic => {
                if options.deep_thinking {
                    const THINKING_BUDGET: u32 = 1024;
                    body["thinking"] = json!({"type": "enabled", "budget_tokens": THINKING_BUDGET});
                    body["max_tokens"] = json!(max_output_tokens.max(THINKING_BUDGET + 1));
                } else {
                    body["temperature"] = json!(options.temperature.clamp(0.0, 1.0));
                }
            }
            Provider::Gemini => {
                let config = &mut body["generationConfig"];
                config["temperature"] = json!(options.temperature);
                config["topP"] = json!(options.top_p);
                if options.deep_thinking {
                    config["thinkingConfig"] = json!({"thinkingBudget": 1024});
                }
            }
        }
        Ok((url, body))
    }

    fn chat_wire(
        &self,
        model: &str,
        messages: &[ChatMessage],
        max_output_tokens: u32,
    ) -> CloudResult<(String, Value)> {
        let system = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let turns: Vec<_> = messages.iter().filter(|m| m.role != "system").collect();
        if let Some(base) = self.base() {
            let url = base
                .join("chat/completions")
                .map_err(|_| INVALID)?
                .to_string();
            let wire_messages: Vec<Value> = messages
                .iter()
                .map(|message| {
                    let content = if message.images.is_empty() {
                        json!(message.content)
                    } else {
                        let mut parts = vec![json!({"type": "text", "text": message.content})];
                        parts.extend(message.images.iter().map(|image| {
                            json!({"type": "image_url", "image_url": {"url": image_data_url(image)}})
                        }));
                        json!(parts)
                    };
                    json!({"role": message.role, "content": content})
                })
                .collect();
            let request = json!({
                "model": model,
                "messages": wire_messages,
                "stream": true,
                "stream_options": {"include_usage":true},
                "max_tokens": max_output_tokens,
            });
            return Ok((url, request));
        }
        match self.protocol() {
            Provider::Openai => {
                let input: Vec<Value> = turns
                    .iter()
                    .map(|message| {
                        let content = if message.role == "user" && !message.images.is_empty() {
                            let mut parts = vec![json!({"type": "input_text", "text": message.content})];
                            parts.extend(message.images.iter().map(|image| {
                                json!({"type": "input_image", "image_url": image_data_url(image)})
                            }));
                            json!(parts)
                        } else {
                            json!(message.content)
                        };
                        json!({"role": message.role, "content": content})
                    })
                    .collect();
                Ok((
                    "https://api.openai.com/v1/responses".into(),
                    json!({
                        "model": model,
                        "stream": true,
                        "instructions": system,
                        "input": input,
                        "max_output_tokens": max_output_tokens,
                        "store": false
                    }),
                ))
            }
            Provider::Anthropic => {
                let wire_msgs: Vec<Value> = turns
                    .iter()
                    .map(|message| {
                        let content = if message.images.is_empty() {
                            json!(message.content)
                        } else {
                            let mut parts: Vec<Value> = message.images.iter().map(|image| {
                                json!({"type": "image", "source": {
                                    "type": "base64", "media_type": image.mime_type, "data": image.data_base64
                                }})
                            }).collect();
                            parts.push(json!({"type": "text", "text": message.content}));
                            json!(parts)
                        };
                        json!({"role": message.role, "content": content})
                    })
                    .collect();
                Ok((
                    "https://api.anthropic.com/v1/messages".into(),
                    json!({
                        "model": model,
                        "system": system,
                        "stream": true,
                        "messages": wire_msgs,
                        "max_tokens": max_output_tokens
                    }),
                ))
            }
            Provider::Gemini => {
                let clean = model.strip_prefix("models/").unwrap_or(model);
                if clean.contains('/') || clean.contains(':') || clean.starts_with('.') {
                    return Err(INVALID);
                }
                let contents: Vec<Value> = turns
                    .iter()
                    .map(|message| {
                        let role = if message.role == "assistant" { "model" } else { "user" };
                        let mut parts = vec![json!({"text": message.content})];
                        parts.extend(message.images.iter().map(|image| {
                            json!({"inlineData": {"mimeType": image.mime_type, "data": image.data_base64}})
                        }));
                        json!({"role": role, "parts": parts})
                    })
                    .collect();
                Ok((
                    format!("https://generativelanguage.googleapis.com/v1beta/models/{clean}:streamGenerateContent?alt=sse"),
                    json!({
                        "systemInstruction": {"parts": [{"text": system}]},
                        "contents": contents,
                        "generationConfig": {"maxOutputTokens": max_output_tokens}
                    }),
                ))
            }
        }
    }

    pub(super) async fn send_generation_stream(
        &self,
        key: &str,
        url: &str,
        request: &Value,
        timeout: Duration,
        on_delta: &mut (dyn FnMut(StreamChunk<'_>) + Send),
    ) -> CloudResult<CompletionOutput> {
        let hash =
            crate::benchmark::dataset::hash(&serde_json::to_vec(request).map_err(|_| INVALID)?);
        let start = Instant::now();
        let client = super::transport::JSON_CLIENT
            .as_ref()
            .map_err(|error| *error)?;
        let mut response = completion::authenticate(
            client.post(url).timeout(timeout).json(request), self.protocol(), key,
        )?
            .send()
            .await
            .map_err(|_| ConnectionState::NetworkUnavailable)?;
        let status = response.status().as_u16();
        if status != 200 {
            let mut err_bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| ConnectionState::NetworkUnavailable)?
            {
                if err_bytes.len() + chunk.len() > (1 << 20) {
                    break;
                }
                err_bytes.extend_from_slice(&chunk);
            }
            let body = serde_json::from_slice(&err_bytes).unwrap_or(Value::Null);
            return Err(super::provider::classify(status, &body));
        }
        let is_sse = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.to_ascii_lowercase().contains("text/event-stream"));
        if !is_sse {
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| ConnectionState::NetworkUnavailable)?
            {
                if bytes.len() + chunk.len() > (2 << 20) {
                    return Err(INVALID);
                }
                bytes.extend_from_slice(&chunk);
            }
            let raw: Value = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
            let adapted = if self.base().is_some() { compatible_completion(&raw)? } else { raw };
            let output = completion::parse(
                self.protocol(),
                &adapted,
                start.elapsed().as_millis() as u64,
                hash,
            )?;
            let checked = completion::checked_output(output, key)?;
            if let Some(thinking) = checked.thinking.as_deref() {
                on_delta(StreamChunk::Thinking(thinking));
            }
            if !checked.text.is_empty() {
                on_delta(StreamChunk::Content(&checked.text));
            }
            return Ok(checked);
        }
        super::chat_stream::read(response, self.base().is_none().then(|| self.protocol()), key, start, hash, on_delta).await
    }

    #[cfg(test)]
    pub(super) async fn send_generation(
        &self,
        key: &str,
        url: &str,
        request: &Value,
        timeout: Duration,
    ) -> CloudResult<CompletionOutput> {
        let hash =
            crate::benchmark::dataset::hash(&serde_json::to_vec(request).map_err(|_| INVALID)?);
        let start = Instant::now();
        let mut response = post_json(self.protocol(), key, url, request, timeout, 2 << 20).await?;
        if self.base().is_some() {
            response = compatible_completion(&response)?;
        }
        let output = completion::parse(
            self.protocol(),
            &response,
            start.elapsed().as_millis() as u64,
            hash,
        )?;
        completion::checked_output(output, key)
    }

    pub(super) async fn embed(
        &self,
        key: &str,
        model: &str,
        input: &[String],
        dimensions: Option<usize>,
        timeout: Duration,
    ) -> CloudResult<(Vec<Vec<f32>>, Option<u64>)> {
        if !valid_model(model)
            || !(1..=16).contains(&input.len())
            || input.iter().any(|text| text.is_empty())
            || input.iter().map(String::len).sum::<usize>() > 128 * 1024
            || dimensions.is_some_and(|n| !(1..=4096).contains(&n))
        {
            return Err(INVALID);
        }
        let (url, request) = match self {
            Self::Official(Provider::Anthropic) => return Err(ConnectionState::ModelDenied),
            Self::Official(Provider::Gemini) => {
                let model = model.strip_prefix("models/").unwrap_or(model);
                if model.contains(['/', ':']) || model.starts_with('.') {
                    return Err(INVALID);
                }
                let model = format!("models/{model}");
                let requests: Vec<_> = input
                    .iter()
                    .map(|text| {
                        let mut request =
                            json!({"model":model,"content":{"parts":[{"text":text}]}});
                        if let Some(n) = dimensions {
                            request["outputDimensionality"] = json!(n);
                        }
                        request
                    })
                    .collect();
                (
                    format!(
                        "https://generativelanguage.googleapis.com/v1beta/{model}:batchEmbedContents"
                    ),
                    json!({"requests":requests}),
                )
            }
            _ => {
                let url = match self.base() {
                    Some(base) => base.join("embeddings").map_err(|_| INVALID)?.to_string(),
                    None => "https://api.openai.com/v1/embeddings".into(),
                };
                let mut request = json!({"model":model,"input":input,"encoding_format":"float"});
                if let Some(n) = dimensions {
                    request["dimensions"] = json!(n);
                }
                (url, request)
            }
        };
        let response = post_json(self.protocol(), key, &url, &request, timeout, 4 << 20).await?;
        decode_embeddings(self.protocol(), &response, input.len(), dimensions)
    }

    pub(super) async fn rerank(
        &self,
        key: &str,
        model: &str,
        query: &str,
        documents: &[String],
        timeout: Duration,
    ) -> CloudResult<(Vec<f64>, Option<u64>)> {
        let Self::Compatible { base, rerank: true } = self else {
            return Err(ConnectionState::ModelDenied);
        };
        if !valid_model(model)
            || query.trim().is_empty()
            || !(1..=64).contains(&documents.len())
            || query.len() + documents.iter().map(String::len).sum::<usize>() > 128 * 1024
        {
            return Err(INVALID);
        }
        let url = base.join("rerank").map_err(|_| INVALID)?;
        let request =
            json!({"model":model,"query":query,"documents":documents,"top_n":documents.len()});
        let response = post_json(
            Provider::Openai,
            key,
            url.as_str(),
            &request,
            timeout,
            1 << 20,
        )
        .await?;
        let scores = ordered(&response["results"], documents.len(), "relevance_score")?
            .into_iter()
            .map(|score| score.as_f64().filter(|n| n.is_finite()).ok_or(INVALID))
            .collect::<CloudResult<Vec<_>>>()?;
        Ok((
            scores,
            response
                .pointer("/meta/billed_units/input_tokens")
                .and_then(Value::as_u64)
                .or_else(|| {
                    response
                        .pointer("/tokens/input_tokens")
                        .and_then(Value::as_u64)
                }),
        ))
    }
}

pub(super) fn valid_model(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
}

fn compatible_completion(response: &Value) -> CloudResult<Value> {
    let choice = response["choices"]
        .as_array()
        .and_then(|choices| choices.first())
        .ok_or(INVALID)?;
    let message = &choice["message"];
    let mut output = Vec::new();
    if let Some(text) = message["content"].as_str() {
        output.push(json!({"type":"message","content":[{"type":"output_text","text":text}]}));
    }
    if let Some(thinking) = message["reasoning_content"]
        .as_str()
        .or_else(|| message["reasoning"].as_str())
    {
        output.push(json!({"type":"reasoning","summary":[{"type":"summary_text","text":thinking}]}));
    }
    if let Some(calls) = message["tool_calls"].as_array() {
        for call in calls {
            if call["type"] != "function" || !call["function"]["name"].is_string() {
                return Err(INVALID);
            }
            output.push(json!({
                "type":"function_call","name":call["function"]["name"],"arguments":call["function"]["arguments"]
            }));
        }
    }
    let (status, reason) = match choice["finish_reason"].as_str() {
        Some("stop" | "tool_calls") => ("completed", Value::Null),
        Some("length") => ("incomplete", json!({"reason":"max_output_tokens"})),
        _ => return Err(INVALID),
    };
    Ok(json!({
        "status":status,"incomplete_details":reason,"output":output,
        "usage":{"input_tokens":response["usage"]["prompt_tokens"],"output_tokens":response["usage"]["completion_tokens"]}
    }))
}

fn decode_embeddings(
    provider: Provider,
    response: &Value,
    count: usize,
    dimensions: Option<usize>,
) -> CloudResult<(Vec<Vec<f32>>, Option<u64>)> {
    let rows: Vec<_> = if provider == Provider::Gemini {
        response["embeddings"]
            .as_array()
            .ok_or(INVALID)?
            .iter()
            .map(|row| &row["values"])
            .collect()
    } else {
        ordered(&response["data"], count, "embedding")?
    };
    let vectors = normalized_embeddings(&json!({"embeddings":rows}), count, dimensions)
        .map_err(|_| INVALID)?;
    Ok((
        vectors,
        response
            .pointer("/usage/prompt_tokens")
            .and_then(Value::as_u64),
    ))
}

fn ordered<'a>(rows: &'a Value, count: usize, field: &str) -> CloudResult<Vec<&'a Value>> {
    let rows = rows
        .as_array()
        .filter(|rows| rows.len() == count)
        .ok_or(INVALID)?;
    let mut indexed = vec![None; count];
    for row in rows {
        let index = row["index"]
            .as_u64()
            .filter(|&n| n < count as u64)
            .ok_or(INVALID)?;
        if indexed[index as usize].replace(&row[field]).is_some() {
            return Err(INVALID);
        }
    }
    indexed
        .into_iter()
        .map(|value| value.ok_or(INVALID))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_options_encode_openai_and_compatible_sampling_and_reasoning() {
        let mut options = GenerationOptions {
            temperature: 0.7,
            top_p: 0.8,
            ..GenerationOptions::default()
        };
        let endpoints = [
            (
                Endpoint::compatible("https://fixture.invalid/v1", false).unwrap(),
                "reasoning_effort",
                json!("medium"),
            ),
            (
                Endpoint::Official(Provider::Openai),
                "reasoning",
                json!({"effort": "medium"}),
            ),
            (
                Endpoint::compatible("https://api.deepseek.com/v1", false).unwrap(),
                "thinking",
                json!({"type": "enabled"}),
            ),
        ];
        for (endpoint, field, expected) in endpoints {
            options.deep_thinking = false;
            let (_, body) = endpoint
                .generation_chat_wire("fixture", &[], 512, Some(options))
                .unwrap();
            assert_eq!(body["temperature"], 0.7);
            assert_eq!(body["top_p"], 0.8);
            assert!(body.get(field).is_none());

            options.deep_thinking = true;
            let (_, body) = endpoint
                .generation_chat_wire("fixture", &[], 512, Some(options))
                .unwrap();
            assert_eq!(body[field], expected);
        }
    }

    #[test]
    fn generation_options_follow_anthropic_sampling_and_thinking_rules() {
        let endpoint = Endpoint::Official(Provider::Anthropic);
        let mut options = GenerationOptions {
            temperature: 1.5,
            top_p: 0.8,
            ..GenerationOptions::default()
        };
        let (_, body) = endpoint
            .generation_chat_wire("fixture", &[], 512, Some(options))
            .unwrap();
        assert_eq!(body["temperature"], 1.0);
        assert_eq!(body["max_tokens"], 512);
        assert!(body.get("top_p").is_none());
        assert!(body.get("thinking").is_none());

        options.deep_thinking = true;
        for (requested, expected) in [(512, 1025), (4096, 4096)] {
            let (_, body) = endpoint
                .generation_chat_wire("fixture", &[], requested, Some(options))
                .unwrap();
            assert!(body.get("temperature").is_none());
            assert!(body.get("top_p").is_none());
            assert_eq!(body["thinking"], json!({"type": "enabled", "budget_tokens": 1024}));
            assert_eq!(body["max_tokens"], expected);
        }
    }

    #[test]
    fn generation_options_encode_gemini_sampling_and_thinking_in_config() {
        let endpoint = Endpoint::Official(Provider::Gemini);
        let mut options = GenerationOptions {
            temperature: 0.7,
            top_p: 0.8,
            ..GenerationOptions::default()
        };
        let (_, body) = endpoint
            .generation_chat_wire("fixture", &[], 512, Some(options))
            .unwrap();
        assert_eq!(
            body["generationConfig"],
            json!({"maxOutputTokens": 512, "temperature": 0.7, "topP": 0.8})
        );

        options.deep_thinking = true;
        let (_, body) = endpoint
            .generation_chat_wire("fixture", &[], 512, Some(options))
            .unwrap();
        assert_eq!(
            body["generationConfig"]["thinkingConfig"],
            json!({"thinkingBudget": 1024})
        );
    }

    #[test]
    fn multimodal_wire_encodes_openai_compatible_anthropic_gemini_and_ollama() {
        let messages = [
            ChatMessage {
                role: "assistant".into(),
                content: "Earlier answer".into(),
                images: Vec::new(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Describe this image".into(),
                images: vec![MessageImage {
                    mime_type: "image/png".into(),
                    data_base64: "AA==".into(),
                }],
            },
        ];
        let text_only = json!({"role": "assistant", "content": "Earlier answer"});
        assert_eq!(serde_json::to_value(&messages[0]).unwrap(), text_only);
        let text = &messages[1].content;
        let data_url = "data:image/png;base64,AA==";

        let compatible = Endpoint::compatible("https://fixture.invalid/v1", false).unwrap();
        let (url, body) = compatible
            .generation_chat_wire("fixture", &messages, 512, Some(GenerationOptions::default()))
            .unwrap();
        assert_eq!(url, "https://fixture.invalid/v1/chat/completions");
        assert_eq!(body["messages"][0], text_only);
        assert_eq!(
            body["messages"][1]["content"],
            json!([
                {"type": "text", "text": text},
                {"type": "image_url", "image_url": {"url": data_url}}
            ])
        );

        let (url, body) = Endpoint::Official(Provider::Openai)
            .generation_chat_wire("fixture", &messages, 512, Some(GenerationOptions::default()))
            .unwrap();
        assert_eq!(url, "https://api.openai.com/v1/responses");
        assert_eq!(body["input"][0], text_only);
        assert_eq!(
            body["input"][1]["content"],
            json!([
                {"type": "input_text", "text": text},
                {"type": "input_image", "image_url": data_url}
            ])
        );

        let (url, body) = Endpoint::Official(Provider::Anthropic)
            .generation_chat_wire("fixture", &messages, 512, Some(GenerationOptions::default()))
            .unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert_eq!(body["messages"][0], text_only);
        assert_eq!(
            body["messages"][1]["content"],
            json!([
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AA=="}},
                {"type": "text", "text": text}
            ])
        );

        let (url, body) = Endpoint::Official(Provider::Gemini)
            .generation_chat_wire("fixture", &messages, 512, Some(GenerationOptions::default()))
            .unwrap();
        assert!(url.ends_with("/fixture:streamGenerateContent?alt=sse"));
        assert_eq!(
            body["contents"][0],
            json!({"role": "model", "parts": [{"text": "Earlier answer"}]})
        );
        assert_eq!(
            body["contents"][1],
            json!({"role": "user", "parts": [
                {"text": text},
                {"inlineData": {"mimeType": "image/png", "data": "AA=="}}
            ]})
        );

        let body = crate::ollama::chat_request(&crate::models::ModelSettings::default(), &messages);
        assert_eq!(body["messages"][0], text_only);
        assert_eq!(
            body["messages"][1],
            json!({"role": "user", "content": text, "images": ["AA=="]})
        );
    }

    #[tokio::test]
    async fn runtime_cloud_compatible_routes_against_http_fixture() {
        use crate::model_runtime::http_fixture::serve_with_authorization;

        let fixed_destinations = [
            ("https://api.deepseek.com", "", "deepseek-flash"),
            (
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "/compatible-mode/v1",
                "qwen-plus",
            ),
            (
                "https://api.siliconflow.cn/v1",
                "/v1",
                "deepseek-ai/DeepSeek-V4-Flash",
            ),
        ];
        let (fixture, server) = serve_with_authorization(8, move |index, path, auth, body| {
            assert_eq!(auth, Some("Bearer fixture-key"));
            let response = match index {
                0..=5 => {
                    let (_, prefix, model) = fixed_destinations[index / 2];
                    if index % 2 == 0 {
                        assert_eq!(path, format!("{prefix}/models"));
                        assert!(body.is_null());
                        json!({"data":[{"id":model}]})
                    } else {
                        assert_eq!(path, format!("{prefix}/chat/completions"));
                        assert_eq!(
                            body,
                            json!({"model":model,"messages":[{"role":"system","content":"Synthetic instructions."},{"role":"user","content":"Answer with OK."}],"max_tokens":64})
                        );
                        json!({"choices":[{"finish_reason":"stop","message":{"content":"OK","reasoning_content":"fixture reasoning"}}],"usage":{"prompt_tokens":8,"completion_tokens":3}})
                    }
                }
                6 => {
                    assert_eq!(path, "/v1/embeddings");
                    assert_eq!(
                        body,
                        json!({"model":"BAAI/bge-large-zh-v1.5","input":["first","second"],"encoding_format":"float"})
                    );
                    json!({"data":[{"index":1,"embedding":[0,1]},{"index":0,"embedding":[3,4]}],"usage":{"prompt_tokens":8}})
                }
                7 => {
                    assert_eq!(path, "/v1/rerank");
                    assert_eq!(
                        body,
                        json!({"model":"Qwen/Qwen3-Reranker-8B","documents":["first","second"],"query":"fixture","top_n":2})
                    );
                    json!({"results":[{"index":1,"relevance_score":0.9},{"index":0,"relevance_score":0.1}],"tokens":{"input_tokens":12,"output_tokens":0}})
                }
                _ => unreachable!(),
            };
            (200, response)
        });
        let timeout = Duration::from_secs(5);
        for (address, prefix, model) in fixed_destinations {
            let endpoint = Endpoint::compatible(address, false).unwrap();
            let request = CompletionInput {
                model: model.into(),
                system: "Synthetic instructions.".into(),
                prompt: "Answer with OK.".into(),
                max_output_tokens: 64,
                schema: None,
                tools: vec![],
            };
            let (url, body) = endpoint.generation_wire(&request).unwrap();
            assert_eq!(url, format!("{address}/chat/completions"));
            let loopback = Endpoint::Compatible {
                base: fixture.join(&format!("{prefix}/")).unwrap(),
                rerank: false,
            };
            let models = loopback.catalog("fixture-key").await.unwrap();
            assert_eq!(models[0].id, model);
            assert_eq!(models[0].generation, "unknown");
            let url = loopback.base().unwrap().join("chat/completions").unwrap();
            let output = endpoint
                .send_generation("fixture-key", url.as_str(), &body, timeout)
                .await
                .unwrap();
            assert_eq!(output.text, "OK");
            assert!(
                !serde_json::to_string(&output)
                    .unwrap()
                    .contains("fixture reasoning")
            );
        }
        let siliconflow = Endpoint::Compatible {
            base: fixture.join("v1/").unwrap(),
            rerank: true,
        };
        let documents = ["first".to_owned(), "second".to_owned()];
        assert_eq!(
            siliconflow
                .embed(
                    "fixture-key",
                    "BAAI/bge-large-zh-v1.5",
                    &documents,
                    None,
                    timeout
                )
                .await
                .unwrap(),
            (vec![vec![0.6, 0.8], vec![0.0, 1.0]], Some(8))
        );
        assert_eq!(
            siliconflow
                .rerank(
                    "fixture-key",
                    "Qwen/Qwen3-Reranker-8B",
                    "fixture",
                    &documents,
                    timeout
                )
                .await
                .unwrap(),
            (vec![0.1, 0.9], Some(12))
        );
        server.join().unwrap();
    }

    #[test]
    fn runtime_cloud_protocol_checks_object_shape_and_order() {
        assert!(Endpoint::compatible("http://example.com/v1", true).is_err());
        assert!(Endpoint::compatible("https://name:secret@example.com/v1", true).is_err());
        assert!(Endpoint::compatible("https://example.com/v1?token=secret", true).is_err());
        let base = Endpoint::compatible("https://example.com/gateway/v1", true).unwrap();
        assert_eq!(
            base.base().unwrap().join("rerank").unwrap().as_str(),
            "https://example.com/gateway/v1/rerank"
        );
        let raw = json!({"data":[{"index":1,"embedding":[0,1]},{"index":0,"embedding":[3,4]}],"usage":{"prompt_tokens":8}});
        let (vectors, tokens) = decode_embeddings(Provider::Openai, &raw, 2, Some(2)).unwrap();
        assert_eq!(vectors, vec![vec![0.6, 0.8], vec![0., 1.]]);
        assert_eq!(tokens, Some(8));
        let duplicate =
            json!({"data":[{"index":1,"embedding":[0,1]},{"index":1,"embedding":[3,4]}]});
        assert!(decode_embeddings(Provider::Openai, &duplicate, 2, Some(2)).is_err());
        let response = json!({"embeddings":[{"values":[3,4]}]});
        assert_eq!(
            decode_embeddings(Provider::Gemini, &response, 1, None).unwrap(),
            (vec![vec![0.6, 0.8]], None)
        );
        let raw = json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"type":"function","function":{"name":"test","arguments":"{\"n\":2}"}}]}}],"usage":{"prompt_tokens":8,"completion_tokens":3}});
        let adapted = compatible_completion(&raw).unwrap();
        let output = completion::parse(Provider::Openai, &adapted, 0, "fixture".into()).unwrap();
        assert_eq!(output.calls[0]["function"]["arguments"]["n"], 2);
        assert_eq!(
            completion::tokens(Provider::Openai, &output.usage),
            (Some(8), Some(3))
        );
    }
}
