use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pylos_application::guardrails::{compute_moderation_scores, normalize_for_detection};
use pylos_application::use_cases::inference::map_model_for_provider;
use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};

fn bench_routing_map_model(c: &mut Criterion) {
    let allowed = vec![
        "claude-sonnet-4-6".to_string(),
        "gemini-3.8-flash-medium".to_string(),
    ];
    c.bench_function("routing/map_model_for_provider", |b| {
        b.iter(|| {
            black_box(map_model_for_provider(
                black_box("antigravity"),
                black_box("claude-3-5-sonnet"),
                black_box(&allowed),
            ))
        })
    });
}

fn bench_serialization_chat_request(c: &mut Criterion) {
    let payload = r#"{
        "model": "gpt-4o",
        "messages": [
            {"role": "system", "content": "You are a concise Rust assistant."},
            {"role": "user", "content": "Explain how async streams work in Tokio with Pin and Poll."}
        ],
        "temperature": 0.2,
        "max_tokens": 512,
        "stream": true
    }"#;

    c.bench_function("serialization/chat_request_roundtrip", |b| {
        b.iter(|| {
            let req: ChatCompletionRequest =
                serde_json::from_str(black_box(payload)).expect("valid json");
            let bytes = serde_json::to_vec(&req).expect("serialize");
            black_box(bytes)
        })
    });
}

fn bench_guardrails_hot_path(c: &mut Criterion) {
    let input = "Hello, can you help me write a Rust parser for JSON streams without blocking?";
    c.bench_function("guardrails/normalize_and_moderate", |b| {
        b.iter(|| {
            let norm = normalize_for_detection(black_box(input));
            let scores = compute_moderation_scores(black_box(&norm));
            black_box(scores)
        })
    });
}

fn bench_cache_key_hash(c: &mut Criterion) {
    let req = ChatCompletionRequest {
        model: "gpt-4o".into(),
        messages: vec![ChatCompletionMessage {
            role: MessageRole::User,
            content: Some("Summarize the quarterly architecture review.".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    c.bench_function("cache/serialize_cache_key", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&req)).expect("serialize");
            black_box(bytes.len())
        })
    });
}

criterion_group!(
    hot_path,
    bench_routing_map_model,
    bench_serialization_chat_request,
    bench_guardrails_hot_path,
    bench_cache_key_hash
);
criterion_main!(hot_path);
