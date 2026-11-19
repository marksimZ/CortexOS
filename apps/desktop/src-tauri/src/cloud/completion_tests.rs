use super::completion::*;
use super::*;
use serde_json::json;
fn request() -> CompletionInput {
    CompletionInput {
        model: "fixture-model".into(),
        system: "system".into(),
        prompt: "prompt".into(),
        max_output_tokens: 512,
        schema: None,
        tools: vec![],
    }
}

#[tokio::test]
async fn sequential_cloud_posts_reuse_a_connection_with_request_scoped_auth() {
    use std::{io::{BufRead, BufReader, Read, Write}, net::TcpListener, thread, time::Duration};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut keys = Vec::new();
        let mut connections = 0;
        while keys.len() < 3 {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            connections += 1;
            let mut reader = BufReader::new(&mut socket);
            while keys.len() < 3 {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 { break; }
                let mut length = 0;
                let mut key = String::new();
                loop {
                    line.clear();
                    assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                    if line == "\r\n" { break; }
                    if let Some((field, value)) = line.split_once(':') {
                        if field.eq_ignore_ascii_case("content-length") { length = value.trim().parse().unwrap(); }
                        if field.eq_ignore_ascii_case("authorization") { key = value.trim().to_owned(); }
                    }
                }
                reader.read_exact(&mut vec![0; length]).unwrap();
                keys.push(key);
                let status = if keys.len() == 3 { "302 Found\r\nLocation: /never-follow" } else { "200 OK" };
                write!(reader.get_mut(), "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}").unwrap();
            }
        }
        (connections, keys)
    });
    for key in ["test-key-a", "test-key-b"] {
        assert_eq!(post_json(Provider::Openai, key, &url, &json!({}), Duration::from_secs(3), 1024).await, Ok(json!({})));
    }
    assert_eq!(post_json(Provider::Openai, "test-key-a", &url, &json!({}), Duration::from_secs(3), 1024).await, Err(ConnectionState::ServiceError));
    let (connections, keys) = server.join().unwrap();
    println!("three sequential requests used {connections} TCP connection(s)");
    assert_eq!(keys, ["Bearer test-key-a", "Bearer test-key-b", "Bearer test-key-a"]);
    assert_eq!(connections, 1);
}
#[test]
fn personalized_cloud_wire_and_native_object_parsers_cover_three_official_protocols() {
    let mut input = request();
    input.schema = Some(
        json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}),
    );
    input.tools = vec![
        json!({"type":"function","function":{"name":"lookup","description":"read only","parameters":{"type":"object","properties":{"id":{"type":"integer"}},"required":["id"],"additionalProperties":false}}}),
    ];
    for provider in [Provider::Openai, Provider::Anthropic, Provider::Gemini] {
        let (url, body) = wire(provider, &input).unwrap();
        assert!(url.starts_with("https://"));
        assert!(!body.to_string().contains("api_key"));
        assert!(body.get("tools").is_some());
    }
    input.model = "models/../../escape".into();
    assert!(wire(Provider::Gemini, &input).is_err());
    let cases = [
        (
            Provider::Openai,
            json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]},{"type":"function_call","name":"lookup","arguments":"{\"id\":3}"}],"usage":{"input_tokens":20,"output_tokens":5}}),
        ),
        (
            Provider::Anthropic,
            json!({"content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"lookup","input":{"id":3}}],"stop_reason":"tool_use","usage":{"input_tokens":20,"output_tokens":5}}),
        ),
        (
            Provider::Gemini,
            json!({"candidates":[{"content":{"parts":[{"text":"private thought","thought":true},{"text":"ok"},{"functionCall":{"name":"lookup","args":{"id":3}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":5}}),
        ),
    ];
    for (provider, body) in cases {
        let output = parse(provider, &body, 100, "a".repeat(64)).unwrap();
        assert_eq!(output.text, "ok");
        assert_eq!(output.calls[0]["function"]["arguments"]["id"], 3);
        assert_eq!(tokens(provider, &output.usage), (Some(20), Some(5)));
        assert!(parse(provider, &json!({}), 0, String::new()).is_err());
    }
    let prose=parse(Provider::Openai,&json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"lookup(id=3)"}]}]}),0,String::new()).unwrap();
    assert!(prose.calls.is_empty());
}
