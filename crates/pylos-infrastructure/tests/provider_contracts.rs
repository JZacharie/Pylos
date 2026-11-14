use pylos_core::domain::openai::ChatCompletionResponse;
use serde_json::Value;

#[test]
fn test_openai_contract_fixture() {
    let raw = include_str!("fixtures/openai/chat_completion.json");
    let val: Value = serde_json::from_str(raw).expect("valid json");
    assert_eq!(val["schema_version"], "1.0");
    let resp: ChatCompletionResponse = serde_json::from_value(val).expect("valid OpenAI contract");
    assert_eq!(resp.model, "gpt-4o");
    assert_eq!(
        resp.choices[0].message.content.as_deref(),
        Some("Hello from OpenAI contract fixture.")
    );
    assert_eq!(resp.usage.as_ref().map(|u| u.total_tokens), Some(19));
}

#[test]
fn test_anthropic_contract_fixture() {
    let raw = include_str!("fixtures/anthropic/messages_response.json");
    let val: Value = serde_json::from_str(raw).expect("valid json");
    assert_eq!(val["schema_version"], "1.0");
    assert_eq!(val["role"], "assistant");
    assert_eq!(val["content"][0]["text"], "Hello from Anthropic contract fixture.");
    assert_eq!(val["usage"]["input_tokens"], 15);
    assert_eq!(val["usage"]["output_tokens"], 8);
}

#[test]
fn test_gemini_contract_fixture() {
    let raw = include_str!("fixtures/gemini/generate_content_response.json");
    let val: Value = serde_json::from_str(raw).expect("valid json");
    assert_eq!(val["schema_version"], "1.0");
    assert_eq!(
        val["candidates"][0]["content"]["parts"][0]["text"],
        "Hello from Gemini contract fixture."
    );
    assert_eq!(val["usageMetadata"]["totalTokenCount"], 16);
}

#[test]
fn test_bedrock_contract_fixture() {
    let raw = include_str!("fixtures/bedrock/converse_response.json");
    let val: Value = serde_json::from_str(raw).expect("valid json");
    assert_eq!(val["schema_version"], "1.0");
    assert_eq!(
        val["output"]["message"]["content"][0]["text"],
        "Hello from Bedrock contract fixture."
    );
    assert_eq!(val["usage"]["inputTokens"], 11);
}

#[test]
fn test_cohere_contract_fixture() {
    let raw = include_str!("fixtures/cohere/chat_response.json");
    let val: Value = serde_json::from_str(raw).expect("valid json");
    assert_eq!(val["schema_version"], "1.0");
    assert_eq!(
        val["message"]["content"][0]["text"],
        "Hello from Cohere contract fixture."
    );
    assert_eq!(val["usage"]["tokens"]["input_tokens"], 9);
}
