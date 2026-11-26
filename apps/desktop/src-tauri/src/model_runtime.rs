use crate::{
    cloud,
    models::*,
    ollama::{Ndjson, Ollama},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    future::{Future, Ready, ready},
};

#[cfg(test)]
pub(crate) mod http_fixture;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelIdentity<'a> {
    pub provider: &'static str,
    pub model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<&'a str>,
}

pub(crate) struct EmbeddingInput<'a> {
    pub model: &'a str,
    pub revision: &'a str,
    pub input: &'a [String],
    pub dimensions: Option<usize>,
    pub meter: Option<&'a crate::runtime_usage::Meter>,
}

pub(crate) trait EmbeddingProvider {
    fn embed(
        &self,
        input: EmbeddingInput<'_>,
    ) -> impl Future<Output = AppResult<Vec<Vec<f32>>>> + Send;
}

pub(crate) fn normalized_embeddings(
    response: &Value,
    count: usize,
    dimensions: Option<usize>,
) -> AppResult<Vec<Vec<f32>>> {
    let invalid = || "向量模型返回了无法使用的维度或数值。".to_owned();
    let rows = response["embeddings"]
        .as_array()
        .filter(|rows| rows.len() == count)
        .ok_or_else(invalid)?;
    let mut expected = dimensions;
    let mut vectors = Vec::with_capacity(count);
    for row in rows {
        let entries = row
            .as_array()
            .filter(|row| {
                (1..=4096).contains(&row.len()) && expected.is_none_or(|n| row.len() == n)
            })
            .ok_or_else(invalid)?;
        let values = entries
            .iter()
            .map(|entry| {
                entry
                    .as_f64()
                    .filter(|n| n.is_finite() && n.abs() <= f32::MAX as f64)
                    .ok_or_else(invalid)
            })
            .collect::<AppResult<Vec<_>>>()?;
        let norm = values.iter().map(|n| n * n).sum::<f64>().sqrt();
        if !norm.is_finite() || norm == 0.0 {
            return Err(invalid());
        }
        expected = Some(values.len());
        vectors.push(
            values
                .into_iter()
                .map(|value| (value / norm) as f32)
                .collect(),
        );
    }
    Ok(vectors)
}

pub(crate) struct RankingInput<'a> {
    pub query: &'a str,
    pub candidates: &'a [(&'a str, &'a str)],
}

pub(crate) trait RerankerProvider {
    type Ranking<'a>: Future<Output = AppResult<Vec<f64>>> + Send
    where
        Self: 'a;
    fn rerank<'a>(&'a self, input: RankingInput<'a>) -> Self::Ranking<'a>;
}

pub(crate) struct TermCoverage;
impl TermCoverage {
    pub const MODEL: &'static str = "local_term_coverage_v1";
}

impl RerankerProvider for TermCoverage {
    type Ranking<'a> = Ready<AppResult<Vec<f64>>>;

    fn rerank<'a>(&'a self, input: RankingInput<'a>) -> Self::Ranking<'a> {
        let (terms, _) = crate::retrieval::query_terms(input.query);
        ready(Ok(input
            .candidates
            .iter()
            .map(|(title, body)| {
                let tokens: BTreeSet<_> = crate::retrieval::terms(&format!("{title} {body}"))
                    .into_iter()
                    .collect();
                terms.iter().filter(|term| tokens.contains(*term)).count() as f64
            })
            .collect()))
    }
}

pub(crate) trait GenerationProvider<Request> {
    type Error;
    fn generate(
        &self,
        input: Request,
    ) -> impl Future<Output = Result<Generated, Self::Error>> + Send;
}

pub(crate) enum Generated {
    Streaming(GenerationStream),
    Complete(cloud::completion::CompletionOutput),
}

pub(crate) struct GenerationFrame {
    pub text: String,
    pub thinking: String,
    pub calls: Vec<Value>,
    pub final_metrics: Option<Value>,
}

pub(crate) struct GenerationStream {
    response: reqwest::Response,
    parser: Ndjson,
    eof: bool,
}

impl GenerationStream {
    pub async fn receive(&mut self) -> AppResult<Option<Vec<AppResult<GenerationFrame>>>> {
        if self.eof {
            return Ok(None);
        }
        let chunk = self
            .response
            .chunk()
            .await
            .map_err(|_| "模型连接中断，已保留收到的内容。请重试。".to_owned())?;
        let frames = match chunk {
            Some(bytes) => self.parser.push(&bytes),
            None => {
                self.eof = true;
                self.parser.finish()
            }
        };
        Ok(Some(
            frames
                .into_iter()
                .map(|frame| {
                    frame.map(|frame| {
                        let final_metrics = frame.done.then(|| frame.metrics());
                        let (text, thinking, calls) = frame
                            .message
                            .map(|item| {
                                (item.content, item.thinking, item.tool_calls.unwrap_or_default())
                            })
                            .unwrap_or_default();
                        GenerationFrame {
                            text,
                            thinking,
                            calls,
                            final_metrics,
                        }
                    })
                })
                .collect(),
        ))
    }
}

pub(crate) struct LocalGeneration<'a> {
    pub settings: &'a ModelSettings,
    pub request: &'a Value,
    pub strict: bool,
}

impl GenerationProvider<LocalGeneration<'_>> for Ollama {
    type Error = String;

    async fn generate(&self, input: LocalGeneration<'_>) -> AppResult<Generated> {
        let response = self.chat(input.settings, input.request).await?;
        Ok(Generated::Streaming(GenerationStream {
            response,
            parser: if input.strict {
                Ndjson::strict()
            } else {
                Ndjson::default()
            },
            eof: false,
        }))
    }
}

pub(crate) struct CloudGeneration<'a> {
    pub connection: &'a cloud::Connection,
    pub request: &'a cloud::completion::CompletionInput,
}

impl GenerationProvider<CloudGeneration<'_>> for cloud::CloudState {
    type Error = cloud::ConnectionState;

    async fn generate(&self, input: CloudGeneration<'_>) -> Result<Generated, Self::Error> {
        self.complete_approved(input.connection, input.request)
            .await
            .map(Generated::Complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
    };

    #[tokio::test]
    async fn model_runtime_local_capabilities_use_separate_requests_and_preserve_stream_metrics() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let digest = "a".repeat(64);
        let expected = digest.clone();
        let fixture = std::thread::spawn(move || {
            let mut loaded = true;
            for expected_path in [
                "/api/show",
                "/api/tags",
                "/api/embed",
                "/api/tags",
                "/api/chat",
                "/api/tags",
                "/api/version",
                "/api/ps",
                "/api/generate",
                "/api/tags",
                "/api/version",
                "/api/ps",
                "/api/tags",
                "/api/version",
                "/api/ps",
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&mut socket);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.split_whitespace().nth(1), Some(expected_path));
                let mut length = 0;
                loop {
                    line.clear();
                    assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut data = vec![0; length];
                reader.read_exact(&mut data).unwrap();
                let request: Value = serde_json::from_slice(&data).unwrap_or_default();
                let body = match expected_path {
                    "/api/show" => {
                        json!({"details":{"format":"gguf"},"capabilities":["embedding"]})
                            .to_string()
                    }
                    "/api/tags" => {
                        json!({"models":[{"name":"vector-fixture","digest":expected}]}).to_string()
                    }
                    "/api/version" => json!({"version":"fixture"}).to_string(),
                    "/api/ps" => {
                        if loaded {
                            json!({"models":[{"name":"vector-fixture","digest":expected,"size":2048,"size_vram":1024}]}).to_string()
                        } else {
                            json!({"models":[]}).to_string()
                        }
                    }
                    "/api/generate" => {
                        assert_eq!(request["model"], "vector-fixture");
                        assert_eq!(request["keep_alive"], 0);
                        assert!(request.get("prompt").is_none());
                        assert!(loaded);
                        loaded = false;
                        json!({"done":true,"done_reason":"unload"}).to_string()
                    }
                    "/api/embed" => {
                        assert_eq!(request["model"], "vector-fixture");
                        assert_eq!(request["input"], json!(["数据库快照"]));
                        assert_eq!(request["keep_alive"], 0);
                        assert_eq!(request["options"]["num_gpu"], 0);
                        json!({"embeddings":[[3.,4.]]}).to_string()
                    }
                    "/api/chat" => {
                        assert_eq!(request["model"], "text-fixture");
                        "{\"message\":{\"role\":\"assistant\",\"content\":\"第一段\"},\"done\":false}\n{\"done\":true,\"prompt_eval_count\":17,\"eval_count\":3,\"done_reason\":\"stop\"}".into()
                    }
                    _ => unreachable!(),
                };
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let provider = Ollama::with_endpoint(&url).unwrap();
        let vector = provider
            .embed(EmbeddingInput {
                model: "vector-fixture",
                revision: &digest,
                input: &["数据库快照".into()],
                dimensions: Some(2),
                meter: None,
            })
            .await
            .unwrap();
        assert_eq!(vector, vec![vec![0.6, 0.8]]);
        assert!(normalized_embeddings(&json!({"embeddings":[[0.0,0.0]]}), 1, Some(2)).is_err());
        let scores = TermCoverage
            .rerank(RankingInput {
                query: "database backup",
                candidates: &[("database", "backup"), ("unrelated", "data")],
            })
            .await
            .unwrap();
        assert_eq!(scores, vec![2., 0.]);
        let settings = ModelSettings {
            model: "text-fixture".into(),
            ..ModelSettings::default()
        };
        let request = crate::ollama::chat_request(
            &settings,
            &[ChatMessage {
                role: "user".into(),
                content: "问题".into(),
                images: Vec::new(),
            }],
        );
        let Generated::Streaming(mut stream) = provider
            .generate(LocalGeneration {
                settings: &settings,
                request: &request,
                strict: true,
            })
            .await
            .unwrap()
        else {
            panic!("stream missing")
        };
        let mut frames = vec![];
        while let Some(batch) = stream.receive().await.unwrap() {
            frames.extend(batch.into_iter().map(Result::unwrap));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].text, "第一段");
        assert!(frames[0].final_metrics.is_none());
        let metrics = frames[1].final_metrics.as_ref().unwrap();
        assert_eq!(metrics["promptEvalCount"], 17);
        assert_eq!(metrics["evalCount"], 3);
        assert_eq!(
            provider
                .release_model("vector-fixture", &digest)
                .await
                .unwrap()["status"],
            "released"
        );
        assert_eq!(
            provider
                .release_model("vector-fixture", &digest)
                .await
                .unwrap()["requestSent"],
            false
        );
        fixture.join().unwrap();
    }
}
