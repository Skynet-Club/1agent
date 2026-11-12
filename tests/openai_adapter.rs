use std::sync::{Arc, Mutex};
use std::time::Duration;

use first_principles_agent::{
    CognitiveMode, MessageRole, ModelMessage, ModelProvider, ModelRequest, ModelTurn,
    OpenAiCompatibleProvider, ProviderErrorCode,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local loopback binding; run outside the managed sandbox"]
async fn openai_compatible_adapter_translates_a_structured_answer() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let answer = serde_json::json!({
        "status": "completed",
        "conclusion": "Use A under the stated constraint.",
        "claims": [{
            "id": "claim-1",
            "text": "A has lower cost.",
            "status": "verified",
            "sources": ["fixture"],
            "scope": "stated constraints"
        }],
        "evidence": [{
            "source": "fixture",
            "statement": "A is cheaper.",
            "claim_ids": ["claim-1"]
        }],
        "assumptions": [],
        "constraints": ["read only"],
        "unknowns": [],
        "flip_conditions": [{
            "if_observed": "B becomes cheaper",
            "then": "switch to B"
        }],
        "action_receipts": []
    })
    .to_string();
    let response_body = serde_json::json!({
        "choices": [{
            "message": {
                "content": answer,
                "tool_calls": null
            }
        }],
        "usage": {
            "prompt_tokens": 42,
            "completion_tokens": 17
        }
    })
    .to_string();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        assert!(request.contains("\"model\":\"test-model\""));
        assert!(request.contains("Compare A and B"));
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();
    });

    let provider = OpenAiCompatibleProvider::new(
        "test-provider",
        "test-model",
        &format!("http://{address}/v1"),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    let request = ModelRequest {
        run_id: Uuid::new_v4(),
        mode: CognitiveMode::DecisionAnalysis,
        messages: vec![
            ModelMessage {
                role: MessageRole::System,
                content: "Return JSON.".into(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            },
            ModelMessage::user("Compare A and B"),
        ],
        tools: Vec::new(),
    };
    let response = provider
        .complete(request, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(response.provider_id, "test-provider");
    assert_eq!(response.model, "test-model");
    assert_eq!(response.usage.input_tokens, 42);
    match response.turn {
        ModelTurn::Final { answer } => {
            assert_eq!(answer.conclusion, "Use A under the stated constraint.");
            assert_eq!(answer.flip_conditions[0].then, "switch to B");
        }
        ModelTurn::InvalidFinal { error, .. } => {
            panic!("expected final answer, got invalid output: {error}")
        }
        ModelTurn::ToolCalls { .. } => panic!("expected final answer"),
    }

    server.await.unwrap();
}

fn streaming_request() -> ModelRequest {
    ModelRequest {
        run_id: Uuid::new_v4(),
        mode: CognitiveMode::DirectAnswer,
        messages: vec![
            ModelMessage {
                role: MessageRole::System,
                content: "Return JSON.".into(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            },
            ModelMessage::user("hello"),
        ],
        tools: Vec::new(),
    }
}

#[tokio::test]
#[ignore = "requires local loopback binding; run outside the managed sandbox"]
async fn openai_compatible_adapter_streams_text_deltas() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let answer = serde_json::json!({
        "status": "completed",
        "conclusion": "Streamed answer.",
        "claims": [],
        "evidence": [],
        "assumptions": [],
        "constraints": [],
        "unknowns": [],
        "flip_conditions": [],
        "action_receipts": []
    })
    .to_string();
    let (first, second) = answer.split_at(answer.len() / 2);
    let first_event = serde_json::json!({
        "choices": [{"delta": {"content": first}}]
    })
    .to_string();
    let second_event = serde_json::json!({
        "choices": [{"delta": {"content": second}}]
    })
    .to_string();
    let usage_event = serde_json::json!({
        "choices": [],
        "usage": {"prompt_tokens": 5, "completion_tokens": 9}
    })
    .to_string();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        assert!(request.contains("\"stream\":true"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        // Split one SSE line across two TCP writes to exercise line reassembly.
        let first_line = format!("data: {first_event}\n\n");
        let (line_head, line_tail) = first_line.split_at(first_line.len() / 2);
        stream.write_all(line_head.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        stream.write_all(line_tail.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        stream
            .write_all(
                format!("data: {second_event}\n\ndata: {usage_event}\n\ndata: [DONE]\n\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        stream.shutdown().await.unwrap();
    });

    let provider = OpenAiCompatibleProvider::new(
        "test-provider",
        "test-model",
        &format!("http://{address}/v1"),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    let deltas = Arc::new(Mutex::new(Vec::new()));
    let collected = deltas.clone();
    let mut on_text = move |text: &str| collected.lock().unwrap().push(text.to_string());
    let response = provider
        .complete_streaming(streaming_request(), &CancellationToken::new(), &mut on_text)
        .await
        .unwrap();

    assert_eq!(deltas.lock().unwrap().len(), 2);
    assert_eq!(deltas.lock().unwrap().concat(), answer);
    assert_eq!(response.usage.input_tokens, 5);
    assert_eq!(response.usage.output_tokens, 9);
    match response.turn {
        ModelTurn::Final { answer: value } => {
            assert_eq!(value.conclusion, "Streamed answer.");
        }
        ModelTurn::InvalidFinal { error, .. } => {
            panic!("expected a streamed final answer, got invalid output: {error}")
        }
        ModelTurn::ToolCalls { .. } => panic!("expected a streamed final answer"),
    }
    server.await.unwrap();
}

#[tokio::test]
#[ignore = "requires local loopback binding; run outside the managed sandbox"]
async fn openai_compatible_adapter_streams_tool_call_deltas() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        assert!(request.contains("\"stream\":true"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        for event in [
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"f.txt\"}"}}]}}]}"#,
            "[DONE]",
        ] {
            stream
                .write_all(format!("data: {event}\n\n").as_bytes())
                .await
                .unwrap();
        }
        stream.shutdown().await.unwrap();
    });

    let provider = OpenAiCompatibleProvider::new(
        "test-provider",
        "test-model",
        &format!("http://{address}/v1"),
        None,
        Duration::from_secs(5),
    )
    .unwrap();
    let mut deltas = Vec::new();
    let mut on_text = |text: &str| deltas.push(text.to_string());
    let response = provider
        .complete_streaming(streaming_request(), &CancellationToken::new(), &mut on_text)
        .await
        .unwrap();

    assert!(deltas.is_empty());
    match response.turn {
        ModelTurn::ToolCalls { calls } => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].id, "call-1");
            assert_eq!(calls[0].name, "read_file");
            assert_eq!(calls[0].arguments["path"], "f.txt");
        }
        other => panic!("expected streamed tool calls, got {other:?}"),
    }
    server.await.unwrap();
}

#[tokio::test]
#[ignore = "requires local loopback binding; run outside the managed sandbox"]
async fn openai_compatible_adapter_cancels_a_stream_after_first_delta() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _request = read_http_request(&mut stream).await;
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let event = r#"{"choices":[{"delta":{"content":"partial"}}]}"#;
        stream
            .write_all(format!("data: {event}\n\n").as_bytes())
            .await
            .unwrap();
        stream.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let provider = OpenAiCompatibleProvider::new(
        "test-provider",
        "test-model",
        &format!("http://{address}/v1"),
        None,
        Duration::from_secs(30),
    )
    .unwrap();
    let cancellation = CancellationToken::new();
    let cancel = cancellation.clone();
    let mut on_text = move |_: &str| cancel.cancel();
    let error = provider
        .complete_streaming(streaming_request(), &cancellation, &mut on_text)
        .await
        .unwrap_err();
    assert_eq!(error.code, ProviderErrorCode::Cancelled);
    server.abort();
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let header_end;
    loop {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "connection closed before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_subslice(&bytes, b"\r\n\r\n") {
            header_end = position + 4;
            break;
        }
    }
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    while bytes.len() < header_end + content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "connection closed before body");
        bytes.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8(bytes).unwrap()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
