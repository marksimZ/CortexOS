use super::{
    ConnectionState, Provider,
    completion::{self, CompletionOutput},
};
use serde_json::{Value, json};
use std::time::Instant;

const INVALID: ConnectionState = ConnectionState::ServiceError;

#[derive(Default)]
struct Answer {
    text: String,
    published: usize,
    usage: Value,
    finish: Option<&'static str>,
}

impl Answer {
    fn event(&mut self, provider: Option<Provider>, data: &[u8]) -> Result<(), ConnectionState> {
        if provider.is_none() && data == b"[DONE]" {
            self.finish.get_or_insert("stop");
            return Ok(());
        }
        let event: Value = serde_json::from_slice(data).map_err(|_| INVALID)?;
        let kind = event["type"].as_str().unwrap_or("");
        if event["error"].is_object() || matches!(kind, "error" | "response.failed") {
            return Err(INVALID);
        }
        match provider {
            None => {
                if event["usage"].is_object() {
                    self.usage = json!({"input_tokens":event["usage"]["prompt_tokens"], "output_tokens":event["usage"]["completion_tokens"]});
                }
                let choice = &event["choices"][0];
                if let Some(delta) = choice["delta"]["content"].as_str() {
                    self.text.push_str(delta);
                }
                if let Some(reason) = choice["finish_reason"].as_str() {
                    self.finish = Some(match reason {
                        "stop" => "stop",
                        "length" => "length",
                        _ => return Err(INVALID),
                    });
                }
            }
            Some(Provider::Openai) => match kind {
                "response.output_text.delta" => {
                    self.text.push_str(event["delta"].as_str().ok_or(INVALID)?)
                }
                "response.completed" | "response.incomplete" => {
                    self.usage = event["response"]["usage"].clone();
                    self.finish = Some(if kind == "response.completed" {
                        "stop"
                    } else if event["response"]["incomplete_details"]["reason"]
                        == "max_output_tokens"
                    {
                        "length"
                    } else {
                        return Err(INVALID);
                    });
                }
                _ => {}
            },
            Some(Provider::Anthropic) => match kind {
                "message_start" => self.usage = event["message"]["usage"].clone(),
                "content_block_delta" if event["delta"]["type"] == "text_delta" => {
                    self.text
                        .push_str(event["delta"]["text"].as_str().ok_or(INVALID)?);
                }
                "message_delta" => {
                    self.usage["output_tokens"] = event["usage"]["output_tokens"].clone();
                    self.finish = Some(match event["delta"]["stop_reason"].as_str() {
                        Some("max_tokens") => "length",
                        Some("end_turn" | "stop_sequence") => "stop",
                        _ => return Err(INVALID),
                    });
                }
                _ => {}
            },
            Some(Provider::Gemini) => {
                if event["usageMetadata"].is_object() {
                    self.usage = event["usageMetadata"].clone();
                }
                let candidate = &event["candidates"][0];
                for part in candidate["content"]["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if part["thought"] != true
                        && let Some(text) = part["text"].as_str()
                    {
                        self.text.push_str(text);
                    }
                }
                if let Some(reason) = candidate["finishReason"].as_str() {
                    self.finish = Some(match reason {
                        "STOP" => "stop",
                        "MAX_TOKENS" => "length",
                        _ => return Err(INVALID),
                    });
                }
            }
        }
        Ok(())
    }

    fn publish(
        &mut self,
        key: &str,
        final_chunk: bool,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<(), ConnectionState> {
        let pending = &self.text[self.published..];
        if self.text.len() > 1 << 20 || pending.contains(key) {
            return Err(INVALID);
        }
        // A credential may span events. Hold only the suffix that could still complete it.
        let held = if final_chunk {
            0
        } else {
            key.char_indices()
                .rev()
                .map(|(end, _)| end)
                .find(|&end| pending.ends_with(&key[..end]))
                .unwrap_or(0)
        };
        let end = self.text.len() - held;
        if end > self.published {
            on_delta(&self.text[self.published..end]);
            self.published = end;
        }
        Ok(())
    }
}

pub(super) async fn read(
    mut response: reqwest::Response,
    provider: Option<Provider>,
    key: &str,
    start: Instant,
    hash: String,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<CompletionOutput, ConnectionState> {
    let mut answer = Answer::default();
    let mut line = Vec::new();
    let mut data = Vec::new();
    let mut received = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ConnectionState::NetworkUnavailable)?
    {
        received += chunk.len();
        if received > 16 << 20 {
            return Err(INVALID);
        }
        for segment in chunk.split_inclusive(|byte| *byte == b'\n') {
            line.extend_from_slice(segment);
            if line.len() + data.len() > 2 << 20 {
                return Err(INVALID);
            }
            if !line.ends_with(b"\n") {
                continue;
            }
            let field = line.strip_suffix(b"\n").unwrap();
            let field = field.strip_suffix(b"\r").unwrap_or(field);
            if field.is_empty() && !data.is_empty() {
                answer.event(provider, &data)?;
                answer.publish(key, false, on_delta)?;
                data.clear();
            } else if let Some(value) = field.strip_prefix(b"data:") {
                if !data.is_empty() {
                    data.push(b'\n');
                }
                data.extend_from_slice(value.strip_prefix(b" ").unwrap_or(value));
            }
            line.clear();
        }
    }
    let finish_reason = answer.finish.ok_or(INVALID)?;
    if answer.text.is_empty() || !line.is_empty() || !data.is_empty() {
        return Err(INVALID);
    }
    answer.publish(key, true, on_delta)?;
    completion::checked_output(
        CompletionOutput {
            text: answer.text,
            calls: vec![],
            usage: answer.usage,
            finish_reason: finish_reason.into(),
            elapsed_ms: start.elapsed().as_millis() as u64,
            request_hash: hash,
        },
        key,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener};

    // Deliver actual HTTP chunks, including a split inside a Chinese code point.
    async fn sample(
        provider: Option<Provider>,
        frames: &[&str],
    ) -> (Result<CompletionOutput, ConnectionState>, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = frames
            .iter()
            .map(|s| format!("data: {s}\r\n\r\n"))
            .collect::<String>();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            use std::io::BufRead;
            for header in std::io::BufReader::new(&socket).lines() {
                if header.unwrap().is_empty() {
                    break;
                }
            }
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
            for chunk in body.as_bytes().chunks(2) {
                write!(socket, "{:x}\r\n", chunk.len()).unwrap();
                socket.write_all(chunk).unwrap();
                socket.write_all(b"\r\n").unwrap();
            }
            socket.write_all(b"0\r\n\r\n").unwrap();
        });
        let response = reqwest::get(format!("http://{address}/")).await.unwrap();
        server.join().unwrap();
        let mut visible = String::new();
        let output = read(
            response,
            provider,
            "test-fixture-secret",
            Instant::now(),
            "fixture".into(),
            &mut |text| visible.push_str(text),
        )
        .await;
        (output, visible)
    }

    #[tokio::test]
    async fn cloud_chat_stream_protocols_and_interruptions() {
        for (provider, frames) in [
            (
                None,
                vec![
                    r#"{"choices":[{"delta":{"content":"你好"}}]}"#,
                    r#"{"choices":[{"finish_reason":"stop"}]}"#,
                    "[DONE]",
                ],
            ),
            (
                Some(Provider::Openai),
                vec![
                    r#"{"type":"response.output_text.delta","delta":"你好"}"#,
                    r#"{"type":"response.completed","response":{"usage":{"input_tokens":2}}}"#,
                ],
            ),
            (
                Some(Provider::Anthropic),
                vec![
                    r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"你好"}}"#,
                    r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}"#,
                ],
            ),
            (
                Some(Provider::Gemini),
                vec![
                    r#"{"candidates":[{"content":{"parts":[{"thought":true,"text":"private"},{"text":"你好"}]},"finishReason":"STOP"}]}"#,
                ],
            ),
        ] {
            let (output, visible) = sample(provider, &frames).await;
            assert_eq!(output.unwrap().text, "你好");
            assert_eq!(visible, "你好");
        }
        let (truncated, partial) =
            sample(None, &[r#"{"choices":[{"delta":{"content":"未完成"}}]}"#]).await;
        assert!(truncated.is_err());
        assert_eq!(partial, "未完成");
        let (echo, visible) = sample(
            None,
            &[
                r#"{"choices":[{"delta":{"content":"开始test-fixture-"}}]}"#,
                r#"{"choices":[{"delta":{"content":"secret"}}]}"#,
            ],
        )
        .await;
        assert!(echo.is_err());
        assert_eq!(visible, "开始");
    }
}
