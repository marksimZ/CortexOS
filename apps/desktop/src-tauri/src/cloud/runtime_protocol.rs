use super::{
    ConnectionState, Provider,
    completion::{self, CompletionInput, CompletionOutput, post_json},
    provider::{Model, catalog},
};
use crate::model_runtime::normalized_embeddings;
use reqwest::Url;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) type CloudResult<T> = Result<T, ConnectionState>;
const INVALID: ConnectionState = ConnectionState::ServiceError;

pub(super) enum Endpoint {
    Official(Provider),
    Compatible { base: Url, rerank: bool },
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
        let (url, mut request) = self.generation_wire(input)?;
        // Leave the production request at the model's default thinking setting.
        // The 64-token probe needs room for the visible answer on DeepSeek.
        if self.base().is_some_and(|base| {
            base.host_str() == Some("api.deepseek.com")
                && base.port().is_none()
                && matches!(base.path(), "/" | "/v1/")
        }) && matches!(
            input.model.as_str(),
            "deepseek-flash" | "deepseek-v4-pro" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
        )
        {
            request["thinking"] = json!({"type":"disabled"});
        }
        self.send_generation(key, &url, &request, timeout).await
    }

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
