use super::{ConnectionState, Provider};
use serde::Serialize;
use serde_json::Value;
use std::{future::Future, pin::Pin, time::Duration};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub generation: &'static str,
}
pub type CatalogFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<Model>, ConnectionState>> + Send + 'a>>;
// One narrow contract, shared by three real protocols and the test provider.
pub trait ProviderAdapter: Send + Sync {
    fn catalog<'a>(&'a self, provider: Provider, key: &'a str) -> CatalogFuture<'a>;
    fn complete<'a>(
        &'a self,
        _provider: Provider,
        _key: &'a str,
        _input: &'a super::completion::CompletionInput,
    ) -> super::completion::CompletionFuture<'a> {
        Box::pin(async { Err(ConnectionState::ServiceError) })
    }
}
pub struct OfficialProvider;
impl ProviderAdapter for OfficialProvider {
    fn complete<'a>(
        &'a self,
        provider: Provider,
        key: &'a str,
        input: &'a super::completion::CompletionInput,
    ) -> super::completion::CompletionFuture<'a> {
        Box::pin(super::completion::official(provider, key, input))
    }
    fn catalog<'a>(&'a self, provider: Provider, key: &'a str) -> CatalogFuture<'a> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|_| ConnectionState::ServiceError)?;
            let mut models = Vec::new();
            let mut cursor: Option<String> = None;
            // Bounded pagination, no automatic retry or inference requests.
            for _ in 0..10 {
                let mut url = reqwest::Url::parse(match provider {
                    Provider::Openai => "https://api.openai.com/v1/models",
                    Provider::Anthropic => "https://api.anthropic.com/v1/models",
                    Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/models",
                })
                .map_err(|_| ConnectionState::ServiceError)?;
                if provider == Provider::Anthropic {
                    url.query_pairs_mut().append_pair("limit", "1000");
                }
                if provider == Provider::Gemini {
                    url.query_pairs_mut().append_pair("pageSize", "1000");
                }
                if let Some(ref value) = cursor {
                    url.query_pairs_mut().append_pair(
                        if provider == Provider::Gemini {
                            "pageToken"
                        } else {
                            "after_id"
                        },
                        value,
                    );
                }
                let mut secret = reqwest::header::HeaderValue::from_str(key)
                    .map_err(|_| ConnectionState::InvalidCredential)?;
                secret.set_sensitive(true);
                let mut request = client.get(url);
                request = match provider {
                    Provider::Openai => request.bearer_auth(key),
                    Provider::Anthropic => request
                        .header("x-api-key", secret)
                        .header("anthropic-version", "2023-06-01"),
                    Provider::Gemini => request.header("x-goog-api-key", secret),
                };
                let mut response = request
                    .send()
                    .await
                    .map_err(|_| ConnectionState::NetworkUnavailable)?;
                let status = response.status().as_u16();
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| ConnectionState::NetworkUnavailable)?
                {
                    if bytes.len() + chunk.len() > 1024 * 1024 {
                        return Err(ConnectionState::ServiceError);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                if status != 200 {
                    return Err(classify(status, &body));
                }
                let (page, next) = parse(provider, &body)?;
                if page.iter().any(|model| model.id.contains(key)) {
                    return Err(ConnectionState::ServiceError);
                }
                models.extend(page);
                if models.len() > 5000 {
                    return Err(ConnectionState::ServiceError);
                }
                if next.is_none() {
                    models.sort_by(|a, b| a.id.cmp(&b.id));
                    models.dedup_by(|a, b| a.id == b.id);
                    return Ok(models);
                }
                if next == cursor {
                    return Err(ConnectionState::ServiceError);
                }
                cursor = next;
            }
            Err(ConnectionState::ServiceError)
        })
    }
}
pub fn classify(status: u16, body: &Value) -> ConnectionState {
    let code = body
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");
    let kind = body
        .pointer("/error/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let bad_key = body
        .pointer("/error/details")
        .and_then(Value::as_array)
        .is_some_and(|a| {
            a.iter()
                .any(|v| v["reason"] == "API_KEY_INVALID" || v["reason"] == "API_KEY_EXPIRED")
        });
    if status == 401 || bad_key {
        ConnectionState::InvalidCredential
    } else if status == 402 || code == "insufficient_quota" || kind == "insufficient_quota" {
        ConnectionState::QuotaExceeded
    } else if status == 403 || status == 404 {
        ConnectionState::ModelDenied
    } else if status == 429 {
        ConnectionState::RateLimited
    } else {
        ConnectionState::ServiceError
    }
}
pub fn parse(
    provider: Provider,
    body: &Value,
) -> Result<(Vec<Model>, Option<String>), ConnectionState> {
    let array = body[if provider == Provider::Gemini {
        "models"
    } else {
        "data"
    }]
    .as_array()
    .ok_or(ConnectionState::ServiceError)?;
    let mut models = Vec::new();
    for value in array {
        let id = value[if provider == Provider::Gemini {
            "name"
        } else {
            "id"
        }]
        .as_str()
        .ok_or(ConnectionState::ServiceError)?;
        if id.is_empty()
            || id.len() > 200
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
        {
            return Err(ConnectionState::ServiceError);
        }
        let generation = if provider == Provider::Gemini {
            if !value["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == "generateContent"))
            {
                continue;
            }
            "listed"
        } else {
            "unknown"
        };
        models.push(Model {
            id: id.into(),
            generation,
        });
    }
    let next = if provider == Provider::Gemini {
        body["nextPageToken"].as_str().map(str::to_owned)
    } else if provider == Provider::Anthropic && body["has_more"] == true {
        Some(
            body["last_id"]
                .as_str()
                .ok_or(ConnectionState::ServiceError)?
                .to_owned(),
        )
    } else {
        None
    };
    if next
        .as_ref()
        .is_some_and(|s| s.is_empty() || s.len() > 2048)
    {
        return Err(ConnectionState::ServiceError);
    }
    Ok((models, next))
}
