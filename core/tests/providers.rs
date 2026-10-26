//! Pruebas de los adaptadores: Claude contra una salida real capturada, OpenRouter contra un servidor falso.

use comodel_core::claude::parse_line;
use comodel_core::events::ProviderEvent;
use comodel_core::openrouter::OpenRouter;
use comodel_core::provider::{Provider, TurnRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[test]
fn parses_real_claude_stream() {
    let raw = std::fs::read_to_string("tests/fixtures/claude_stream.jsonl").unwrap();
    let events: Vec<ProviderEvent> = raw.lines().flat_map(parse_line).collect();

    assert!(matches!(&events[0], ProviderEvent::Started { model: Some(m), .. } if m.contains("haiku")));
    assert!(events.contains(&ProviderEvent::Text("ok".into())));
    // Uso restante de las ventanas de 5 horas y 7 días.
    let windows: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ProviderEvent::Quota { window, used_fraction, .. } => Some((window.as_str(), *used_fraction)),
            _ => None,
        })
        .collect();
    assert!(windows.iter().any(|(w, f)| *w == "five_hour" && *f > 0.0 && *f < 1.0), "{windows:?}");
    assert!(windows.iter().any(|(w, _)| *w == "seven_day"));
    // Ventana de contexto y coste.
    assert!(events.iter().any(|e| matches!(e, ProviderEvent::Usage { context_window: Some(w), cost_usd: Some(_), .. } if *w >= 200_000)));
    assert_eq!(events.last(), Some(&ProviderEvent::Finished));
}

#[test]
fn detects_claude_limits() {
    let blocked = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792968000,"rateLimitType":"five_hour"}}"#;
    assert!(matches!(parse_line(blocked)[..], [ProviderEvent::LimitReached { resets_at: Some(1792968000), .. }]));

    let by_429 = r#"{"type":"result","subtype":"error","is_error":true,"api_error_status":429,"result":"Too many requests","usage":{}}"#;
    assert!(parse_line(by_429).iter().any(|e| matches!(e, ProviderEvent::LimitReached { .. })));

    let by_text = r#"{"type":"result","is_error":true,"result":"Claude AI usage limit reached","usage":{}}"#;
    assert!(parse_line(by_text).iter().any(|e| matches!(e, ProviderEvent::LimitReached { .. })));

    let other = r#"{"type":"result","is_error":true,"result":"algo se rompió","usage":{}}"#;
    assert!(parse_line(other).iter().any(|e| matches!(e, ProviderEvent::Error(_))));
}

/// Servidor HTTP mínimo: responde `status`/`body` a cualquier petición a `/chat/completions`
/// y un saldo fijo a `/key`.
async fn fake_openrouter(status: u16, content_type: &'static str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let (head_end, content_len) = loop {
                    let n = sock.read(&mut tmp).await.unwrap();
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        break (p + 4, len);
                    }
                };
                while buf.len() < head_end + content_len {
                    let n = sock.read(&mut tmp).await.unwrap();
                    buf.extend_from_slice(&tmp[..n]);
                }
                let first = String::from_utf8_lossy(&buf).lines().next().unwrap_or("").to_string();
                let (st, ct, b) = if first.contains("/key") {
                    (200, "application/json", r#"{"data":{"limit":10.0,"usage":2.5}}"#.to_string())
                } else {
                    (status, content_type, body)
                };
                let resp = format!(
                    "HTTP/1.1 {st} X\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                    b.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

async fn run(or: &OpenRouter) -> Vec<ProviderEvent> {
    let (tx, mut rx) = mpsc::channel(64);
    let req = TurnRequest { prompt: "hola".into(), ..Default::default() };
    or.run_turn(req, tx).await.unwrap();
    let mut out = Vec::new();
    while let Some(e) = rx.recv().await {
        out.push(e);
    }
    out
}

#[tokio::test]
async fn openrouter_streams_text_usage_and_quota() {
    let sse = concat!(
        ": OPENROUTER PROCESSING\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Ho\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"la\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{}}],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":3,\"cost\":0.0001}}\n\n",
        "data: [DONE]\n\n",
    );
    let mut or = OpenRouter::new("openrouter", "test-key");
    or.base_url = fake_openrouter(200, "text/event-stream", sse.into()).await;
    let events = run(&or).await;

    let text: String = events.iter().filter_map(|e| if let ProviderEvent::Text(t) = e { Some(t.as_str()) } else { None }).collect();
    assert_eq!(text, "Hola");
    assert!(events.iter().any(|e| matches!(e, ProviderEvent::Usage { input_tokens: 12, output_tokens: 3, .. })));
    assert!(events.iter().any(|e| matches!(e, ProviderEvent::Quota { window, used_fraction, .. } if window == "credits" && (*used_fraction - 0.25).abs() < 1e-9)));
    assert_eq!(events.last(), Some(&ProviderEvent::Finished));
}

#[tokio::test]
async fn openrouter_maps_429_and_402_to_limit() {
    for status in [429u16, 402] {
        let mut or = OpenRouter::new("openrouter", "test-key");
        or.base_url = fake_openrouter(status, "application/json", r#"{"error":{"message":"sin cupo"}}"#.into()).await;
        let events = run(&or).await;
        assert!(matches!(events.as_slice(), [ProviderEvent::LimitReached { reason, .. }] if reason.contains("sin cupo")), "{status}: {events:?}");
    }
}

#[tokio::test]
async fn openrouter_other_http_error_is_not_a_limit() {
    let mut or = OpenRouter::new("openrouter", "test-key");
    or.base_url = fake_openrouter(500, "application/json", r#"{"error":{"message":"caído"}}"#.into()).await;
    let events = run(&or).await;
    assert!(matches!(events.as_slice(), [ProviderEvent::Error(_)]));
}

#[test]
fn events_serialize_for_the_ui() {
    use comodel_core::orchestrator::UiEvent;
    let ev = UiEvent::Provider { provider: "claude".into(), event: ProviderEvent::Text("hola".into()) };
    assert_eq!(
        serde_json::to_value(&ev).unwrap(),
        serde_json::json!({"kind": "Provider", "provider": "claude", "event": {"type": "Text", "data": "hola"}})
    );
    let fin = serde_json::to_value(UiEvent::Provider { provider: "a".into(), event: ProviderEvent::Finished }).unwrap();
    assert_eq!(fin["event"], serde_json::json!({"type": "Finished"}));
    let h = serde_json::to_value(UiEvent::Handoff { from: "a".into(), to: "b".into(), reason: "x".into() }).unwrap();
    assert_eq!(h["kind"], "Handoff");
}
