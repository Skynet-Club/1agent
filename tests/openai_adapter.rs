use std::time::Duration;

use first_principles_agent::{
    CognitiveMode, MessageRole, ModelMessage, ModelProvider, ModelRequest, ModelTurn,
    OpenAiCompatibleProvider,
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
