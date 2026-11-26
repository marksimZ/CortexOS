//! Text-only Aider boundary and bounded forwarding through the existing Provider.
//! Kept outside session.rs so existing recorded workflow hashes remain stable.
use serde_json::{Value, json};
use std::time::{Duration, Instant};
const LIMIT: usize = 65536;
pub(super) const WIRE_MODEL: &str = "cortexos-aider-local";
pub(super) fn policy() -> Value {
    json!({"version":"aider-local-chat-1","maxRequests":4,"wallSeconds":180,"requestSeconds":120,"requestBytes":LIMIT,"responseBytes":LIMIT,"contextTokens":4096,"outputTokensPerRequest":512,"temperature":0,"keepAliveSeconds":0,"transport":"docker_stdio_container_loopback","protocol":"chat_completions_text_to_native_chat","streaming":"buffered_response","retry":false})
}

pub(crate) struct Budget {
    started: Instant,
    requests: u32,
    model: String,
    fingerprint: String,
}
impl Budget {
    pub(crate) fn new(model: String, fingerprint: String) -> Result<Self, String> {
        if model.is_empty() || fingerprint.is_empty() {
            return Err("model_identity_required".into());
        }
        Ok(Self {
            started: Instant::now(),
            requests: 0,
            model,
            fingerprint,
        })
    }
    /// Consumes a request before validation: malformed attempts cannot bypass the cap.
    pub(crate) fn request(
        &mut self,
        body: &Value,
        current_fingerprint: &str,
        cancelled: bool,
    ) -> Result<Value, String> {
        if cancelled {
            return Err("cancelled".into());
        }
        if self.started.elapsed() >= Duration::from_secs(180) {
            return Err("wall_timeout".into());
        }
        if self.requests >= 4 {
            return Err("request_budget_exceeded".into());
        }
        self.requests += 1;
        if current_fingerprint != self.fingerprint {
            return Err("model_identity_changed".into());
        }
        let object = body.as_object().ok_or("invalid_request")?;
        if body.to_string().len() > LIMIT
            || body["model"] != self.model
            || object
                .keys()
                .any(|k| !matches!(k.as_str(), "model" | "messages" | "temperature" | "stream"))
            || object.get("stream").is_some_and(|v| v != false)
            || object
                .get("temperature")
                .is_some_and(|v| v.as_f64() != Some(0.0))
        {
            return Err("model_request_scope_mismatch".into());
        }
        let messages = body["messages"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 128)
            .ok_or("invalid_messages")?;
        for message in messages {
            let fields = message.as_object().ok_or("invalid_message")?;
            if fields.len() != 2
                || !matches!(
                    message["role"].as_str(),
                    Some("system" | "user" | "assistant")
                )
                || !message["content"].is_string()
            {
                return Err("unsupported_message".into());
            }
        }
        Ok(
            json!({"model":self.model,"messages":messages,"stream":false,"options":{"num_ctx":4096,"num_predict":512,"temperature":0},"keep_alive":0}),
        )
    }
    pub(crate) fn remaining(&self) -> Duration {
        Duration::from_secs(180)
            .saturating_sub(self.started.elapsed())
            .min(Duration::from_secs(120))
    }
}
pub(crate) fn response(raw: &Value, model: &str, id: u32) -> Result<Value, String> {
    if raw.to_string().len() > LIMIT
        || raw["model"] != model
        || raw["done"] != true
        || raw.get("error").is_some()
        || raw["message"]["role"] != "assistant"
        || raw["message"]
            .get("tool_calls")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err("unsupported_model_response".into());
    }
    if raw["eval_count"].as_u64().is_some_and(|n| n > 512) {
        return Err("model_output_limit".into());
    }
    let content = raw["message"]["content"]
        .as_str()
        .ok_or("invalid_model_content")?;
    let finish = match raw["done_reason"].as_str() {
        Some("stop") => "stop",
        Some("length") => "length",
        _ => return Err("unsupported_finish_reason".into()),
    };
    let usage = |key: &str| -> Result<Value, String> {
        match raw.get(key) {
            None | Some(Value::Null) => Ok(Value::Null),
            Some(v) if v.as_u64().is_some() => Ok(v.clone()),
            _ => Err("invalid_usage".into()),
        }
    };
    Ok(
        json!({"id":format!("cortexos-aider-{id}"),"object":"chat.completion","created":0,"model":model,"choices":[{"index":0,"finish_reason":finish,"message":{"role":"assistant","content":content}}],"usage":{"prompt_tokens":usage("prompt_eval_count")?,"completion_tokens":usage("eval_count")?}}),
    )
}
/// Actual Provider forwarding. Call only from a confirmed, exclusively leased task.
/// Returns the request and response together for append-only event storage by caller.
pub(crate) async fn forward(
    provider: &crate::ollama::Ollama,
    budget: &mut Budget,
    body: &Value,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    confirmed: bool,
    on_send: impl FnOnce(&Value) -> Result<(), String>,
) -> Result<Value, String> {
    if !confirmed {
        return Err("model_confirmation_required".into());
    }
    if *cancel.borrow() {
        return Err("cancelled".into());
    }
    let fingerprint = budget.fingerprint.clone();
    let native = budget.request(body, &fingerprint, false)?;
    let remaining = budget.remaining();
    if remaining.is_zero() {
        return Err("wall_timeout".into());
    }
    let model = budget.model.clone();
    let number = budget.requests;
    let work = async {
        let catalog = provider.benchmark_models().await?;
        if !catalog
            .models
            .iter()
            .any(|m| m.name == model && m.digest.as_deref() == Some(fingerprint.as_str()))
        {
            return Err("model_identity_changed".into());
        }
        let settings = crate::models::ModelSettings {
            model: model.clone(),
            num_ctx: 4096,
            num_predict: 512,
            temperature: 0.0,
            timeout_seconds: 120,
            ..Default::default()
        };
        on_send(&native)?;
        let mut reply = provider.chat(&settings, &native).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = reply
            .chunk()
            .await
            .map_err(|_| "model_response_disconnected")?
        {
            if bytes.len() + chunk.len() > LIMIT {
                return Err("model_response_limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let raw = crate::benchmark::evaluation_json::decode(
            std::str::from_utf8(&bytes).map_err(|_| "model_invalid_utf8")?,
        )
        .map_err(|_| "model_invalid_json")?;
        let protocol = response(&raw, &model, number)?;
        // Preserve observable text/usage only; do not export private thinking fields.
        Ok(
            json!({"number":number,"nativeRequest":native,"protocolReply":protocol,"modelDigest":fingerprint,"usage":{"promptTokens":raw["prompt_eval_count"],"outputTokens":raw["eval_count"],"duration":raw["total_duration"]}}),
        )
    };
    let cancelled = async {
        loop {
            if *cancel.borrow_and_update() {
                return;
            }
            if cancel.changed().await.is_err() {
                return;
            }
        }
    };
    tokio::select! {
        biased;
        _=cancelled=>Err("cancelled".into()),
        result=tokio::time::timeout(remaining,work)=>result.map_err(|_|"model_request_timeout".to_string())?,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Requires the isolated HTTP fixture driver; never connects to a model service"]
    async fn aider_provider_fixture() {
        assert_eq!(
            std::env::var("CORTEXOS_AIDER_PROVIDER_FIXTURE").unwrap(),
            "1"
        );
        let endpoint = std::env::var("CORTEXOS_TEST_OLLAMA_URL").unwrap();
        assert!(endpoint.starts_with("http://127.0.0.1:") && !endpoint.ends_with(":11434"));
        let provider = crate::ollama::Ollama::new().unwrap();
        let request = json!({"model":"fixture-only","messages":[{"role":"user","content":"edit"}],"temperature":0});
        let mut budget = Budget::new("fixture-only".into(), "a".repeat(64)).unwrap();
        let (tx, rx) = tokio::sync::watch::channel(false);
        assert_eq!(
            forward(&provider, &mut budget, &request, rx.clone(), false, |_| Ok(
                ()
            ))
            .await
            .unwrap_err(),
            "model_confirmation_required"
        );
        tx.send(true).unwrap();
        assert_eq!(
            forward(&provider, &mut budget, &request, rx.clone(), true, |_| Ok(
                ()
            ))
            .await
            .unwrap_err(),
            "cancelled"
        );
        tx.send(false).unwrap();
        let result = forward(&provider, &mut budget, &request, rx, true, |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(
            result["protocolReply"]["choices"][0]["message"]["content"],
            "fixture edit"
        );
        assert_eq!(result["number"], 1);
    }
    #[test]
    fn aider_boundary_budget_and_response() {
        let request = json!({"model":"local-fixture","messages":[{"role":"user","content":"edit"}],"temperature":0});
        let mut budget = Budget::new("local-fixture".into(), "fingerprint".into()).unwrap();
        assert_eq!(
            budget.request(&request, "fingerprint", false).unwrap()["options"]["num_ctx"],
            4096
        );
        assert!(budget.remaining() <= Duration::from_secs(120));
        let mut wrong = request.clone();
        wrong["tools"] = json!([]);
        assert!(budget.request(&wrong, "fingerprint", false).is_err());
        assert!(budget.request(&request, "different", false).is_err());
        assert!(budget.request(&request, "fingerprint", false).is_ok());
        assert_eq!(
            budget.request(&request, "fingerprint", false).unwrap_err(),
            "request_budget_exceeded"
        );
        let mut stopped = Budget::new("local-fixture".into(), "fingerprint".into()).unwrap();
        assert_eq!(
            stopped.request(&request, "fingerprint", true).unwrap_err(),
            "cancelled"
        );
        stopped.started = Instant::now() - Duration::from_secs(181);
        assert_eq!(
            stopped.request(&request, "fingerprint", false).unwrap_err(),
            "wall_timeout"
        );
        let good = json!({"model":"local-fixture","done":true,"done_reason":"stop","message":{"role":"assistant","content":"solution.py"}});
        assert_eq!(
            response(&good, "local-fixture", 1).unwrap()["usage"]["prompt_tokens"],
            Value::Null
        );
        let mut bad = good.clone();
        bad["message"]["tool_calls"] = json!([{"name":"shell"}]);
        assert!(response(&bad, "local-fixture", 1).is_err());
        assert!(response(&good, "wrong-model", 1).is_err());
    }
}
