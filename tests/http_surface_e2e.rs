use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use first_principles_agent::{
    AuthContext, AuthorityProfile, GovernedTaskGraph, HttpSurfaceEnvelope, SurfaceEventBus,
    SurfaceMethod, SurfaceRequest, SurfaceService, TicketDraft, bind_surface_http, serve_bound,
};

fn request(ticket_id: &str) -> SurfaceRequest {
    SurfaceRequest {
        protocol_version: "v0".into(),
        request_id: "request-1".into(),
        trace_id: "trace-1".into(),
        idempotency_key: Some("idem-1".into()),
        method: SurfaceMethod::TicketApprove {
            ticket_id: ticket_id.into(),
            actor_id: "owner-1".into(),
            decision: "allow_once".into(),
            expires_ms: None,
        },
    }
}

fn auth() -> AuthContext {
    AuthContext {
        actor_id: "owner-1".into(),
        scopes: vec!["ticket:approve".into()],
    }
}

fn setup() -> (SurfaceService, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("task-graph.jsonl");
    let mut graph = GovernedTaskGraph::open(&path).unwrap();
    graph
        .register_profile(AuthorityProfile {
            profile_id: "owner-profile".into(),
            actor_id: "owner-1".into(),
            scopes: vec!["read*".into()],
            max_strength: first_principles_agent::MandateStrength::ReadOnly,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            max_risk: first_principles_agent::RiskLevel::Medium,
            expires_ms: None,
        })
        .unwrap();
    graph.bind_actor("owner-1", "owner-profile").unwrap();
    let thread_id = graph.open_thread("http", 10).unwrap();
    let trigger_id = graph
        .ingest_trigger(
            &thread_id,
            first_principles_agent::Trigger {
                trigger_id: "trigger-1".into(),
                source: first_principles_agent::TriggerSource::Web,
                source_event_id: "event-1".into(),
                summary: "http".into(),
                received_ms: 1,
            },
            11,
        )
        .unwrap()
        .trigger_id;
    let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
    graph
        .decide_eval(&eval_id, first_principles_agent::Decision::Accept, "ok", 13)
        .unwrap();
    let ticket_id = graph
        .create_ticket_from_eval(
            &eval_id,
            "owner-1",
            TicketDraft {
                owner_id: "owner-1".into(),
                scope: "read".into(),
                strength: first_principles_agent::MandateStrength::ReadOnly,
                policy_version: "v1".into(),
                requires_approval: true,
            },
            14,
        )
        .unwrap();
    (SurfaceService::from_graph(graph), ticket_id, dir)
}

fn send(addr: std::net::SocketAddr, token: Option<&str>, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = format!(
        "POST /v0/surface HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let head = std::str::from_utf8(&response[..split]).unwrap();
    let status = head
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, response[split + 4..].to_vec())
}

#[test]
#[ignore = "requires loopback binding; run outside the managed sandbox"]
fn surface_http_approves_with_bearer_and_rejects_missing_token() {
    let (service, ticket_id, _dir) = setup();
    let listener = bind_surface_http("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    let events = Arc::new(Mutex::new(SurfaceEventBus::default()));
    let server_events = events.clone();
    let server = std::thread::spawn(move || {
        serve_bound(
            listener,
            "test-token".into(),
            auth(),
            Arc::new(Mutex::new(service)),
            server_events,
            flag,
        )
    });

    let envelope = HttpSurfaceEnvelope {
        auth: auth(),
        request: request(&ticket_id),
    };
    let body = serde_json::to_vec(&envelope).unwrap();
    let (status, response) = send(addr, Some("test-token"), &body);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&response));
    let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["ok"], true);

    let (status, _) = send(addr, None, &body);
    assert_eq!(status, 401);

    let mut sse = TcpStream::connect(addr).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sse.write_all(
        format!(
            "GET /v0/events HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer test-token\r\nConnection: keep-alive\r\n\r\n"
        )
        .as_bytes(),
    )
    .unwrap();
    let mut headers = Vec::new();
    let mut chunk = [0u8; 1024];
    while !headers.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = sse.read(&mut chunk).unwrap();
        assert!(read > 0);
        headers.extend_from_slice(&chunk[..read]);
    }
    assert!(String::from_utf8_lossy(&headers).contains("text/event-stream"));
    for _ in 0..100 {
        if events.lock().unwrap().subscribers() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    events
        .lock()
        .unwrap()
        .publish("message", serde_json::json!({"text": "hello"}))
        .unwrap();
    let mut event = Vec::new();
    while !event.windows(2).any(|window| window == b"\n\n") {
        let read = sse.read(&mut chunk).unwrap();
        assert!(read > 0);
        event.extend_from_slice(&chunk[..read]);
    }
    let event = String::from_utf8_lossy(&event);
    assert!(event.contains("event: message"), "{event}");
    assert!(event.contains("hello"), "{event}");
    drop(sse);

    shutdown.store(true, Ordering::Release);
    server.join().unwrap().unwrap();
}
