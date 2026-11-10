//! Shared fixtures for the cancellation/pause-governor integration suites.
//! Some builders serve only one of the binaries; no dead code below.
//! a real request-counting SSE mock, the fixtures that fill a session past
//! the compaction thresholds, and the small sync bridge. Tests own their
//! assertions; this module only builds the stage.

#![allow(dead_code)] // see module doc: per-binary linking flags shared builders

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use sprachspiel::chat::session::ChatSession;
use sprachspiel::config::ModelConfig;
use sprachspiel::db::Database;
use sprachspiel::chat::llm_event::LlmEvent;
use sprachspiel::provider::OpenAICompatibleProvider;
use std::sync::Mutex;

pub struct MockServer {
    pub base_url: String,
    requests: Arc<AtomicUsize>,
    _handle: JoinHandle<()>,
}

impl MockServer {
    /// One-thread OpenAI-compatible SSE server. `on_request` receives the
    /// 0-based request index before the response is written, so cancelling
    /// a token inside it reproduces Ctrl+C racing an in-flight call.
    pub fn spawn(on_request: impl Fn(usize) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let port = listener.local_addr().expect("local addr").port();
        let requests = Arc::new(AtomicUsize::new(0));
        let reqs = Arc::clone(&requests);
        let handler = Arc::new(on_request);
        let handle = std::thread::spawn(move || {
            for conn in listener.incoming() {
                let mut stream = match conn {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut raw = Vec::new();
                let mut buf = [0u8; 16384];
                loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let idx = reqs.fetch_add(1, Ordering::SeqCst);
                handler(idx);
                let body = concat!(
                    "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"S\"}}]}\n\n",
                    "data: [DONE]\n\n"
                );
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                     Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                );
                let chunk = format!("{:x}\r\n{}\r\n", body.len(), body);
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(chunk.as_bytes());
                let _ = stream.write_all(b"0\r\n\r\n");
                let _ = stream.flush();
            }
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requests,
            _handle: handle,
        }
    }

    /// Successful-request counter — what "zero later calls" asserts on.
    pub fn count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    /// A server that rejects every request outright (HTTP 400): the model
    /// failure path without retries, so a compaction lands on its Failed
    /// outcome immediately.
    pub fn spawn_rejecting() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let port = listener.local_addr().expect("local addr").port();
        let requests = Arc::new(AtomicUsize::new(0));
        let reqs = Arc::clone(&requests);
        let handle = std::thread::spawn(move || {
            for conn in listener.incoming() {
                let mut stream = match conn {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut buf = [0u8; 16384];
                let mut raw = Vec::new();
                loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                reqs.fetch_add(1, Ordering::SeqCst);
                let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.flush();
            }
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requests,
            _handle: handle,
        }
    }
}

/// A 4096-token window makes the middle-compaction thresholds reachable
/// with ordinary test text: COMPACTION_PROMPT_OVERHEAD alone leaves the
/// window unable to hold a handful of padded messages, forcing the layered
/// path, while four short exchanges still fit in one pass.
pub fn test_model(context_window: u32) -> ModelConfig {
    ModelConfig {
        model_id: "test-model".to_string(),
        num_ctx: context_window,
        temperature: 0.3,
        top_p: Some(0.9),
        thinking: false,
    }
}

pub fn provider_mock(server: &MockServer) -> OpenAICompatibleProvider {
    OpenAICompatibleProvider::new_local(server.base_url.clone(), 0)
}

/// A live view on a kept channel: no rendering assertions here, the
/// receiver just outlives the view so sends never close.
pub fn channel_view() -> sprachspiel::chat::channel_view::ChannelView {
    sprachspiel::chat::channel_view::ChannelView::new(tokio::sync::mpsc::channel(64).0)
}

/// Six padded exchanges: ~800 estimated tokens each (tokens = words / 0.75)
/// — far past the window with overhead (Layer 2 territory), inside a
/// ~2.4K-token chunk budget, so middle compaction splits into two chunks.
pub fn long_session() -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..6 {
        session.add_user_message(format!("topic {i}: {}", "field ".repeat(600)));
        session.add_assistant_message(format!("answer {i}: {}", "note ".repeat(300)), None, None);
    }
    session
}

/// Four short exchanges — everything fits in the single-pass layer.
pub fn short_session() -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..4 {
        session.add_user_message(format!("question {i}"));
        session.add_assistant_message(format!("short answer {i}"), None, None);
    }
    session
}

/// A non-anonymous session with an in-memory database attached, so save
/// paths are observable through the conversation metadata.
pub fn session_with_db() -> (ChatSession, Arc<Database>) {
    let db = Arc::new(Database::in_memory().expect("in-memory db"));
    let mut session = ChatSession::new("test-model".to_string(), None, false);
    session.db = Some(Arc::clone(&db));
    (session, db)
}

pub fn persisted_summary(db: &Database, session_id: &str) -> Option<Option<String>> {
    db.get_conversation_metadata(session_id)
        .map(|meta| meta.compacted_summary)
        .ok()
}

pub fn llm_channel() -> tokio::sync::mpsc::Sender<LlmEvent> {
    tokio::sync::mpsc::channel(64).0
}

/// Fill a 4096-token session far past the compaction buffer
/// (needs_buffered_compaction triggers at ~3.1K estimated tokens).
pub fn fill_past_buffer(session: &mut ChatSession) {
    session.add_user_message("word ".repeat(6_000));
    session.add_assistant_message("note ".repeat(3_000), None, None);
}

/// Shared fact mirror matching `spawn_llm_task`'s per-turn fallback type.
pub type TestMirror = Arc<Mutex<sprachspiel::chat::fact_tracker::SessionFactTracker>>;

pub fn fresh_mirror() -> TestMirror {
    Arc::new(Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()))
}

pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}
