//! Cancellation-aware compaction (turn-token lifecycle).
//!
//! Real end-to-end checks against a local SSE mock: the provider layer is
//! the concrete `OpenAICompatibleProvider` driving a real TCP server, so an
//! LLM-call counter measures "no further calls" exactly. The mock's
//! `on_request` hook runs with the request index before the response is
//! sent, which is what reproduces a Ctrl+C racing the model mid-call.
//!
//! Covers the accepted test matrix: cancel per layer → zero later calls;
//! cancel between a complete summary and its commit → discard, never
//! partial; cancel in a continuation → no continuation request; cancel
//! after the response → response preserved; summary/range/token state
//! untouched on discard; DB ≡ live session on terminal; token-less path
//! unchanged.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use tokio_util::sync::CancellationToken;

use sprachspiel::chat::compaction::{CompactionContext, CompactionOutcome};
use sprachspiel::chat::core::{SendMessageResult, compact_conversation};
use sprachspiel::chat::continuation::{ProcessResult, process_send_result};
use sprachspiel::chat::llm_event::LlmEvent;
use sprachspiel::chat::repl::{TurnTerminal, handle_user_message_stream};
use sprachspiel::chat::repl_state::ReplStateBuilder;
use sprachspiel::chat::session::ChatSession;
use sprachspiel::config::ModelConfig;
use sprachspiel::db::Database;
use sprachspiel::provider::OpenAICompatibleProvider;
use sprachspiel::settings::Settings;

// ── Local SSE mock ────────────────────────────────────────────────────

struct MockServer {
    base_url: String,
    requests: Arc<AtomicUsize>,
    _handle: JoinHandle<()>,
}

impl MockServer {
    /// One-thread OpenAI-compatible SSE server. `on_request` receives the
    /// 0-based request index before the response is written, so cancelling
    /// a token inside it reproduces Ctrl+C racing an in-flight call.
    fn spawn(on_request: impl Fn(usize) + Send + Sync + 'static) -> Self {
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

    fn count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

// ── Fixtures ──────────────────────────────────────────────────────────

fn test_model(context_window: u32) -> ModelConfig {
    ModelConfig {
        model_id: "test-model".to_string(),
        num_ctx: context_window,
        temperature: 0.3,
        top_p: Some(0.9),
        thinking: false,
    }
}

fn provider_mock(server: &MockServer) -> OpenAICompatibleProvider {
    OpenAICompatibleProvider::new_local(server.base_url.clone(), 0)
}

/// A live view on a kept channel: no rendering assertions here, the
/// receiver just outlives the view so sends never close.
fn channel_view() -> sprachspiel::chat::channel_view::ChannelView {
    sprachspiel::chat::channel_view::ChannelView::new(tokio::sync::mpsc::channel(64).0)
}

/// Six 1200-char exchanges: ~600 tokens each — far past the window with
/// overhead (Layer 2 territory), inside a ~2.4K-token chunk budget, so the
/// middle-compaction path splits them into two chunks.
fn long_session() -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..6 {
        // ~800 estimated tokens per message (tokens = words / 0.75).
        session.add_user_message(format!("topic {i}: {}", "field ".repeat(600)));
        session.add_assistant_message(format!("answer {i}: {}", "note ".repeat(300)), None, None);
    }
    session
}

/// Four short exchanges — everything fits in the single-pass layer.
fn short_session() -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..4 {
        session.add_user_message(format!("question {i}"));
        session.add_assistant_message(format!("short answer {i}"), None, None);
    }
    session
}

/// A non-anonymous session with an in-memory database attached, so save
/// paths are observable through the conversation metadata.
fn session_with_db() -> (ChatSession, Arc<Database>) {
    let db = Arc::new(Database::in_memory().expect("in-memory db"));
    let mut session = ChatSession::new("test-model".to_string(), None, false);
    session.db = Some(Arc::clone(&db));
    (session, db)
}

fn llm_channel() -> tokio::sync::mpsc::Sender<LlmEvent> {
    tokio::sync::mpsc::channel(64).0
}

fn persisted_summary(db: &Database, session_id: &str) -> Option<Option<String>> {
    db.get_conversation_metadata(session_id)
        .map(|meta| meta.compacted_summary)
        .ok()
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}

/// Fill a 4096-token session far past the compaction buffer
/// (needs_buffered_compaction triggers at ~3.1K estimated tokens).
fn fill_past_buffer(session: &mut ChatSession) {
    session.add_user_message("word ".repeat(6_000));
    session.add_assistant_message("note ".repeat(3_000), None, None);
}

// ── Matrix: cancel per layer → zero later calls ──────────────────────

#[test]
fn cancel_before_compaction_starts_makes_zero_llm_calls() {
    let server = MockServer::spawn(|_| panic!("no LLM call may happen after cancellation"));
    let llm_tx = llm_channel();
    let mut session = long_session();
    let cfg = test_model(4096);
    let token = CancellationToken::new();
    token.cancel();

    let mut view = channel_view();
    let outcome = block_on(CompactionContext {
        provider: &provider_mock(&server),
        model_config: &cfg,
        session: &mut session,
        settings: &Settings::default(),
        agents_md: None,
        context_window: cfg.num_ctx as usize,
        view: &mut view,
        llm_tx,
        cancel_token: Some(&token),
    }
    .compact_if_needed());

    assert_eq!(outcome, CompactionOutcome::Cancelled);
    assert_eq!(server.count(), 0, "cancelled before start: zero model calls");
    assert!(session.compacted_summary.is_none());
}

#[test]
fn cancel_during_first_chunk_never_starts_the_next() {
    // Layer 2 with two chunks: the mock cancels the token when the first
    // chunk request lands. The chunk run must abort there — chunk 2 and
    // the consolidation pass never reach the wire.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn(move |_| cancel_for_hook.cancel());
    let llm_tx = llm_channel();
    let mut session = long_session();
    let cfg = test_model(4096);

    let outcome = block_on(compact_conversation(
        &provider_mock(&server),
        &cfg,
        &session,
        &Settings::default(),
        None,
        llm_tx,
        Some(token),
    ));

    let Err(err) = outcome else {
        panic!("mid-chunk cancellation must surface as a failure");
    };
    assert!(
        err.is_cancelled(),
        "the failure must be the typed cancellation, not a model error"
    );
    assert!(
        server.count() <= 2,
        "only the first chunk ever hit the wire ({} requests)",
        server.count()
    );
    assert!(session.compacted_summary.is_none());
    assert!(session.compacted_range.is_none());
}

#[test]
fn cancel_during_final_pass_discards_the_complete_summary() {
    // Single-pass summary: the mock cancels while the (successful) call is
    // in flight. The summary comes back complete — and must still be
    // discarded as one unit, never committed partially.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn(move |_| cancel_for_hook.cancel());
    let llm_tx = llm_channel();
    let session = short_session();
    let cfg = test_model(4096);

    let outcome = block_on(compact_conversation(
        &provider_mock(&server),
        &cfg,
        &session,
        &Settings::default(),
        None,
        llm_tx,
        Some(token),
    ));

    let Err(err) = outcome else {
        panic!("a turn-cancelled pass must not commit its summary");
    };
    assert!(err.is_cancelled());
    assert_eq!(server.count(), 1);
    assert!(
        session.compacted_summary.is_none(),
        "a summary produced during cancellation is discarded whole"
    );
    assert!(session.compacted_range.is_none());
    // Message token state untouched by an aborted pass.
    let token_state = session
        .messages
        .iter()
        .map(|m| m.prompt_tokens)
        .collect::<Vec<_>>();
    assert!(
        token_state.iter().all(|t| t.is_none()),
        "no prompt token was rewritten"
    );
}

// ── Matrix: discard stays coherent / DB ≡ live session ───────────────

#[tokio::test]
async fn completed_summary_with_turn_token_defers_the_commit() {
    // With a turn token the summary lands in the in-memory clone only; the
    // SQLite commit is the turn's terminal commit, so an untouched database
    // proves the deferral (the cancel path then discards the clone whole).
    let server = MockServer::spawn(|_| {});
    let llm_tx = llm_channel();
    let (mut session, db) = session_with_db();
    for i in 0..4 {
        session.add_user_message(format!("question {i}"));
        session.add_assistant_message(format!("answer {i}"), None, None);
    }
    fill_past_buffer(&mut session);
    let cfg = test_model(4096);
    let token = CancellationToken::new();

    let mut view = channel_view();
    let outcome = CompactionContext {
        provider: &provider_mock(&server),
        model_config: &cfg,
        session: &mut session,
        settings: &Settings::default(),
        agents_md: None,
        context_window: cfg.num_ctx as usize,
        view: &mut view,
        llm_tx,
        cancel_token: Some(&token),
    }
    .compact_if_needed()
    .await;

    assert_eq!(outcome, CompactionOutcome::Completed);
    assert!(session.compacted_summary.is_some());
    assert!(
        persisted_summary(&db, &session.id).map(|s| s.is_none()).unwrap_or(true),
        "deferred commit: the db must not hold a summary the turn has not terminalized"
    );
}

#[tokio::test]
async fn cancel_after_response_preserves_it_and_makes_no_calls() {
    // Post-response auto-compaction with the token already cancelled: the
    // final response stays in the session, the summary never appears,
    // zero model calls, and the turn still commits the response.
    let server = MockServer::spawn(|_| panic!("post-response cancellation makes no model calls"));
    let llm_tx = llm_channel();
    let (mut session, _db) = session_with_db();
    session.add_user_message("long message to fill the buffer".to_string());
    fill_past_buffer(&mut session);
    let cfg = test_model(4096);
    let provider = provider_mock(&server);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider)
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid test ReplState");
    fill_past_buffer(&mut state.session);

    let token = CancellationToken::new();
    token.cancel();

    let mut view = channel_view();
    let result = SendMessageResult {
        response: "the final answer".to_string(),
        thinking: None,
        pre_tool_content: None,
        pre_tool_thinking: None,
        metrics: sprachspiel::chat::view::TokenMetrics {
            prompt_tokens: 100,
            response_tokens: 10,
            total_tokens: 110,
        },
        context_window: cfg.num_ctx as usize,
        system_prompt: String::new(),
        continuation_needed: None,
    };

    let processed = process_send_result(
        &mut state, result, None, &mut view, llm_tx, Some(&token),
    )
    .await;

    assert_eq!(
        processed,
        ProcessResult::Success,
        "a response preserved across a trailing cancellation still completes"
    );
    assert_eq!(server.count(), 0);
    assert!(
        state
            .session
            .messages
            .iter()
            .any(|m| m.content == "the final answer"),
        "the response survives the cancelled compaction"
    );
    assert!(state.session.compacted_summary.is_none());
}

#[tokio::test]
async fn tokenless_path_commits_and_saves_like_before() {
    // No token: a successful compaction applies the summary AND commits it
    // to SQLite in place — the pre-156 behavior for non-TUI callers.
    let server = MockServer::spawn(|_| {});
    let llm_tx = llm_channel();
    let (mut session, db) = session_with_db();
    for i in 0..4 {
        session.add_user_message(format!("question {i}"));
        session.add_assistant_message(format!("answer {i}"), None, None);
    }
    fill_past_buffer(&mut session);
    let cfg = test_model(4096);

    let mut view = channel_view();
    let outcome = CompactionContext {
        provider: &provider_mock(&server),
        model_config: &cfg,
        session: &mut session,
        settings: &Settings::default(),
        agents_md: None,
        context_window: cfg.num_ctx as usize,
        view: &mut view,
        llm_tx,
        cancel_token: None,
    }
    .compact_if_needed()
    .await;

    assert_eq!(outcome, CompactionOutcome::Completed);
    assert!(session.compacted_summary.is_some());
    assert_eq!(
        persisted_summary(&db, &session.id),
        Some(Some(session.compacted_summary.clone().expect("summary"))),
        "token-less callers commit the compaction right where they always did"
    );
}

// ── Turn level: handler decides the terminal; chain cancels short ────

#[tokio::test]
async fn cancelled_before_the_turn_start_terminates_cancelled() {
    let server = MockServer::spawn(|_| panic!("no provider call may run after cancellation"));
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.add_user_message("hello".to_string());
    fill_past_buffer(&mut session);

    let cfg = test_model(4096);
    let provider = provider_mock(&server);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg)
        .provider(provider)
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let (llm_tx, mut llm_rx) = tokio::sync::mpsc::channel(64);
    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let token = CancellationToken::new();
    token.cancel();

    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "a new message",
        &mut state,
        &mut view,
        llm_tx,
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Cancelled);
    assert_eq!(server.count(), 0);
    while let Ok(ev) = llm_rx.try_recv() {
        assert!(
            !matches!(ev, LlmEvent::StreamToken(_) | LlmEvent::StreamDone { .. }),
            "a cancelled turn streams nothing new"
        );
    }
}

#[tokio::test]
async fn cancellation_flagged_result_never_sends_continuations() {
    // The continuation path: the initial result asks for a continuation and
    // the turn token is already cancelled — the handler must stop before
    // sending the continuation request (zero model calls) and report
    // ProcessResult::Cancelled so the clone is discarded.
    let server = MockServer::spawn(|_| panic!("cancellation blocks the continuation request"));
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.add_user_message("filler".to_string());
    fill_past_buffer(&mut session);

    let cfg = test_model(4096);
    let provider = provider_mock(&server);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider)
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let result = SendMessageResult {
        response: "partial".to_string(),
        thinking: None,
        pre_tool_content: None,
        pre_tool_thinking: None,
        metrics: sprachspiel::chat::view::TokenMetrics {
            prompt_tokens: 100,
            response_tokens: 10,
            total_tokens: 110,
        },
        context_window: cfg.num_ctx as usize,
        system_prompt: String::new(),
        continuation_needed: Some(sprachspiel::chat::coordinator::ContinuationTag {
            paused_at: "mid".to_string(),
            next_step: "continue".to_string(),
        }),
    };

    let llm_tx = llm_channel();
    let token = CancellationToken::new();
    token.cancel();
    let mut view = channel_view();

    let outcome = process_send_result(
        &mut state, result, None, &mut view, llm_tx, Some(&token),
    )
    .await;

    assert_eq!(
        outcome,
        ProcessResult::Cancelled,
        "the continuation chain must report the cancellation"
    );
    assert_eq!(server.count(), 0, "no continuation request reaches the wire");
    assert!(
        !state
            .session
            .messages
            .iter()
            .any(|m| m.content == "partial"),
        "nothing from the aborted chain is committed"
    );
}
