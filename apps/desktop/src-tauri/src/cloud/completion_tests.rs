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
