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
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use tokio_util::sync::CancellationToken;

use sprachspiel::chat::compaction::{CompactionContext, CompactionOutcome};
use sprachspiel::chat::continuation::{ProcessResult, process_send_result};
use sprachspiel::chat::core::{SendMessageResult, compact_conversation};
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

/// A scripted response for one request index: a full SSE body, or an HTTP
/// rejection (non-empty body: an empty-body 400 is the llama-swap
/// transient-retry signature, which would drive nondeterministic retries).
#[derive(Clone, PartialEq)]
enum ScriptedResponse {
    Body(String),
    /// A non-streaming JSON completion, served with plain framing — the
    /// chunked-SSE framing the 2xx path uses is what the non-streaming
    /// decoder rejects with "error decoding response body".
    Json(String),
    Reject,
}

impl MockServer {
    /// One-thread OpenAI-compatible SSE server. `on_request` receives the
    /// 0-based request index before the response is written, so cancelling
    /// a token inside it reproduces Ctrl+C racing an in-flight call.
    fn spawn(on_request: impl Fn(usize) + Send + Sync + 'static) -> Self {
        Self::spawn_scripted(Vec::new(), on_request)
    }

    /// Scripted variant: request `idx` gets `responses[idx]`; past the end
    /// the LAST body repeats, so a repeating model behavior (a compaction
    /// cycle that keeps overflowing) costs one line.
    fn spawn_scripted(
        responses: Vec<ScriptedResponse>,
        on_request: impl Fn(usize) + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let port = listener.local_addr().expect("local addr").port();
        let requests = Arc::new(AtomicUsize::new(0));
        let reqs = Arc::clone(&requests);
        let handler = Arc::new(on_request);
        let responses = Arc::new(responses);
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
                let first_line = String::from_utf8_lossy(&raw)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let body_preview: String = {
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body_bytes = &raw[pos + 4..];
                        let body_start = String::from_utf8_lossy(body_bytes).to_string();
                        body_start.chars().take(120).collect()
                    } else {
                        String::new()
                    }
                };
                handler(idx);
                const FIXED_BODY: &str = concat!(
                    "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"S\"}}]}\n\n",
                    "data: [DONE]\n\n"
                );
                // A PERMANENT-pattern 4xx: "invalid" maps to no-retry, so a
                // rejection is one request, never a retry chain.
                const REJECT_BODY: &str =
                    "{\"error\":{\"message\":\"invalid request: rejected deliberately\"}}";
                let (status, body, plain_framing) =
                    match responses.get(idx).or_else(|| responses.last()) {
                        Some(ScriptedResponse::Body(b)) => ("200 OK", b.clone(), false),
                        Some(ScriptedResponse::Json(p)) => ("200 OK", p.clone(), true),
                        Some(ScriptedResponse::Reject) => {
                            ("400 Bad Request", REJECT_BODY.to_string(), true)
                        }
                        None => ("200 OK", FIXED_BODY.to_string(), false),
                    };
                if plain_framing {
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(body.as_bytes());
                    let _ = stream.flush();
                } else {
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\n\
                         Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                    );
                    let chunk = format!("{:x}\r\n{}\r\n", body.len(), body);
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(chunk.as_bytes());
                    let _ = stream.write_all(b"0\r\n\r\n");
                    let _ = stream.flush();
                }
            }
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            requests,
            _handle: handle,
        }
    }

    /// A plain streaming answer whose single content token is `content`.
    /// Newlines are JSON-escaped: a raw newline inside a JSON string makes
    /// the chunk unparseable and silently drops the content.
    fn plain_body(content: &str) -> ScriptedResponse {
        let escaped = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        ScriptedResponse::Body(format!(
            "data: {{\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"test-model\",\
             \"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{escaped}\"}}}}]}}\n\n\
             data: [DONE]\n\n"
        ))
    }

    /// A non-streaming JSON completion — the shape the coordinator's
    /// non-streaming chat path parses, served with plain framing by the
    /// mock's `Json` variant. Streaming requests never consume this, and
    /// non-streaming requests cannot consume the SSE bodies above, so a
    /// script drives both paths by index.
    fn json_body(content: &str) -> ScriptedResponse {
        let escaped = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        ScriptedResponse::Json(format!(
            "{{\"id\":\"c\",\"object\":\"chat.completion\",\"created\":1,\
             \"model\":\"test-model\",\"choices\":[{{\"index\":0,\
             \"message\":{{\"role\":\"assistant\",\"content\":\"{escaped}\"}},\
             \"finish_reason\":\"stop\"}}],\
             \"usage\":{{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}}}"
        ))
    }

    /// A streaming answer that streams a content prefix (the turn's
    /// pre-tool content) and then asks for one `run_command` call.
    fn content_then_tool_call_body(prefix: &str, arguments_json: &str) -> ScriptedResponse {
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        let mut body = MockServer::tool_call_body(arguments_json);
        if let ScriptedResponse::Body(text) = &mut body {
            *text = format!(
                "data: {{\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"test-model\",\
                 \"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{escaped}\"}}}}]}}\n\n{text}"
            );
        }
        body
    }

    /// A streaming answer that asks for one `run_command` call and ends the
    /// turn with `finish_reason: tool_calls` — the shape that drives the
    /// coordinator into its tool loop.
    fn tool_call_body(arguments_json: &str) -> ScriptedResponse {
        let escaped = arguments_json.replace('\\', "\\\\").replace('"', "\\\"");
        ScriptedResponse::Body(format!(
            concat!(
                "data: {{\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"test-model\",\"choices\":[{{\"index\":0,\"delta\":{{\"tool_calls\":[{{",
                "\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{{",
                "\"name\":\"run_command\",\"arguments\":\"{escaped}\"}}}}]}},",
                "\"finish_reason\":null}}]}}\n\n",
                "data: {{\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"test-model\",\"choices\":[{{\"index\":0,\"delta\":{{}},",
                "\"finish_reason\":\"tool_calls\"}}]}}\n\n",
                "data: [DONE]\n\n"
            ),
            escaped = escaped,
        ))
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

/// Sixteen exchanges: the middle-compaction window keeps five on each side
/// and summarizes six heavy middle messages (~4.8K estimated tokens) —
/// Layer 1 does not fit, and Layer 2 splits that middle into TWO chunks,
/// so the terminal consolidation pass is the third request.
fn sixteen_session() -> ChatSession {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..16 {
        if (5..11).contains(&i) {
            session.add_user_message(format!("topic {i}: {}", "field ".repeat(600)));
            session.add_assistant_message(
                format!("answer {i}: {}", "note ".repeat(300)),
                None,
                None,
            );
        } else {
            session.add_user_message(format!("topic {i}"));
            session.add_assistant_message(format!("answer {i}"), None, None);
        }
    }
    session
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
    let outcome = block_on(
        CompactionContext {
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
        .compact_if_needed(),
    );

    assert_eq!(outcome, CompactionOutcome::Cancelled);
    assert_eq!(
        server.count(),
        0,
        "cancelled before start: zero model calls"
    );
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
        server.count() == 1,
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
        persisted_summary(&db, &session.id)
            .map(|s| s.is_none())
            .unwrap_or(true),
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

    let processed =
        process_send_result(&mut state, result, None, &mut view, llm_tx, Some(&token)).await;

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
    // to SQLite in place — token-less callers preserve immediate-commit behavior.
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

    let outcome =
        process_send_result(&mut state, result, None, &mut view, llm_tx, Some(&token)).await;

    assert_eq!(
        outcome,
        ProcessResult::Cancelled,
        "the continuation chain must report the cancellation"
    );
    assert_eq!(
        server.count(),
        0,
        "no continuation request reaches the wire"
    );
    assert!(
        !state
            .session
            .messages
            .iter()
            .any(|m| m.content == "partial"),
        "nothing from the aborted chain is committed"
    );
}

// ── Matrix: provider error paths, terminal commit, layers, TUI /compact ─

/// Configures a per-test tools.toml whitelist (the cached loader reads it
/// on the first `get_config`) with `sleep` and `echo`.
fn allow_run_commands(config_dir: &std::path::Path) {
    std::fs::create_dir_all(config_dir.join("sprachspiel")).expect("config dir");
    std::fs::write(
        config_dir.join("sprachspiel/tools.toml"),
        "[external.tools.sleep]\nbinary = \"sleep\"\nenabled = true\n\n\
         [external.tools.echo]\nbinary = \"echo\"\nenabled = true\n",
    )
    .expect("write tools.toml");
    // SAFETY: single test binary; the cached config loader reads this once.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", config_dir) };
}

#[tokio::test]
async fn provider_error_racing_cancellation_ends_cancelled() {
    // A real provider failure (no cancellation marker in the error text)
    // that races the stop request ends the turn cancelled — never as a
    // Complete whose adopted clone has no final response to preserve.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(vec![ScriptedResponse::Reject], move |_| {
        cancel_for_hook.cancel()
    });
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.add_user_message("hello".to_string());

    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg)
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "a message that will fail",
        &mut state,
        &mut view,
        llm_channel(),
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Cancelled);
    assert_eq!(server.count(), 1);
    assert!(
        state
            .session
            .messages
            .iter()
            .all(|m| m.role != sprachspiel::chat::session::MessageRole::Assistant),
        "a cancelled failed turn preserves no final response"
    );
}

#[tokio::test]
async fn provider_error_without_cancellation_still_completes() {
    // Without a stop request the same provider failure keeps its old
    // behavior: the error surfaces and the terminal is Complete.
    let server = MockServer::spawn_scripted(vec![ScriptedResponse::Reject], |_| {});
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.add_user_message("hello".to_string());

    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg)
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let token = CancellationToken::new();
    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "a message that will fail",
        &mut state,
        &mut view,
        llm_channel(),
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Complete);
    assert_eq!(server.count(), 1);
}

#[tokio::test]
async fn terminal_commit_persists_pre_turn_compaction_when_the_provider_fails() {
    // The pre-turn compaction completes (its summary is in memory only),
    // then the provider fails with a non-overflow error: the terminal
    // commit persists the summary and invalidates the prompt tokens —
    // neither happened before this fix.
    let server = MockServer::spawn_scripted(
        vec![
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            MockServer::plain_body("compact summary"),
            ScriptedResponse::Reject,
        ],
        |_| {},
    );
    let (session, db) = session_with_db();
    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");
    state
        .session
        .add_user_message("a filler question".to_string());
    // A cumulative prompt-token value above the buffer: it both makes the
    // pre-turn compaction fire and leaves rows with prompt tokens in the
    // DB for the terminal commit to clear.
    {
        let prior = state.session.add_assistant_message(
            format!("{} note", "note ".repeat(300)),
            None,
            Some(3_500),
        );
        let _ = prior;
    }
    fill_past_buffer(&mut state.session);
    let _ = state.session.save_sqlite();
    assert!(
        db.conversation_has_prompt_tokens(&state.session.id)
            .expect("db")
    );

    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "this turn will fail after compaction",
        &mut state,
        &mut view,
        llm_channel(),
        CancellationToken::new(),
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Complete);
    assert!(
        state.session.compacted_summary.is_some(),
        "the completed pre-turn compaction stays in the adopted clone"
    );
    assert_eq!(
        persisted_summary(&db, &state.session.id),
        Some(state.session.compacted_summary.clone()),
        "DB and adopted clone agree after the terminal"
    );
    assert!(
        !db.conversation_has_prompt_tokens(&state.session.id)
            .expect("db"),
        "the terminal invalidates prompt tokens a completed compaction made stale"
    );
}

#[tokio::test]
async fn terminal_commit_persists_after_a_continuation_failure() {
    // A tool call streams a pre-tool content prefix, then the model pauses
    // for a continuation; the chain compacts and its second continuation
    // request fails. A failed chain is still a COMPLETE turn: the terminal
    // commit inserts the deferred capture, persists the in-memory summary
    // and invalidates the prompt tokens.
    let config_dir = std::env::temp_dir().join(format!("sprach-tools-tcf-{}", std::process::id()));
    allow_run_commands(&config_dir);
    let server = MockServer::spawn_scripted(
        vec![
            // pre-turn compaction: four chunk passes and the consolidation
            MockServer::plain_body("pre-turn chunk 1 summary"),
            MockServer::plain_body("pre-turn chunk 2 summary"),
            MockServer::plain_body("pre-turn chunk 3 summary"),
            MockServer::plain_body("pre-turn chunk 4 summary"),
            MockServer::plain_body("pre-turn consolidation"),
            MockServer::content_then_tool_call_body(
                "I will check the file before answering.",
                r#"{"command_line":"echo done","head":null,"tail":null,"timeout_seconds":null}"#,
            ),
            MockServer::plain_body(
                "partial answer\n<continuation_needed>\nReasoning paused: mid\nNext step: go on\n</continuation_needed>",
            ),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            MockServer::plain_body("in-chain summary"),
            ScriptedResponse::Reject,
        ],
        |idx| eprintln!("HOOK-REQ {idx}"),
    );
    let (session, db) = session_with_db();
    let cfg = test_model(4096);
    let caps = sprachspiel::capabilities::ModelCapabilities {
        tools: true,
        ..sprachspiel::capabilities::ModelCapabilities::default()
    };
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .tools_active(true)
        .capabilities(caps)
        .build()
        .expect("valid ReplState");
    state.session.add_user_message("filler".to_string());
    state
        .session
        .add_assistant_message("note ".repeat(300), None, Some(3_500));
    fill_past_buffer(&mut state.session);

    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let token = CancellationToken::new();
    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "<continuation_needed>\nReasoning paused: mid\nNext step: go on\n</continuation_needed>",
        &mut state,
        &mut view,
        llm_channel(),
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Complete);
    assert!(
        state.session.compacted_summary.is_some(),
        "the in-chain compaction stays in the adopted clone"
    );
    assert_eq!(
        persisted_summary(&db, &state.session.id),
        Some(state.session.compacted_summary.clone()),
        "the failed-turn commit reaches the DB"
    );
    assert!(
        db.persisted_message_count(&state.session.id, "I will check the file")
            .expect("db")
            > 0,
        "deferred pre-tool captures are inserted by the terminal commit"
    );
    assert!(
        !db.conversation_has_prompt_tokens(&state.session.id)
            .expect("db"),
        "prompt tokens invalidation follows the chain's compaction"
    );
    let _ = std::fs::remove_dir_all(&config_dir);
}

#[tokio::test]
async fn abandoned_continuation_chain_leaves_zero_residual_rows() {
    // The same chain, but the stop request lands on its in-flight
    // continuation request: the clone is abandoned whole — the deferred
    // capture queue dies with it, so ZERO pre-tool rows stay in the
    // database, and the in-memory summary never persists.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(
        vec![
            // pre-turn compaction: four chunk passes and the consolidation
            MockServer::plain_body("pre-turn chunk 1 summary"),
            MockServer::plain_body("pre-turn chunk 2 summary"),
            MockServer::plain_body("pre-turn chunk 3 summary"),
            MockServer::plain_body("pre-turn chunk 4 summary"),
            MockServer::plain_body("pre-turn consolidation"),
            MockServer::content_then_tool_call_body(
                "I will check the file before answering.",
                r#"{"command_line":"echo done","head":null,"tail":null,"timeout_seconds":null}"#,
            ),
            MockServer::plain_body(
                "partial answer\n<continuation_needed>\nReasoning paused: mid\nNext step: go on\n</continuation_needed>",
            ),
            // The continuation request itself; the stop request lands while
            // it is in flight. The session was just compacted pre-turn, so
            // the chain's own compaction check is NotNeeded — no extra
            // requests between the tag and this one.
            MockServer::plain_body("final answer"),
        ],
        move |idx| {
            if idx == 7 {
                cancel_for_hook.cancel();
            }
        },
    );
    let (session, db) = session_with_db();
    let cfg = test_model(4096);
    let caps = sprachspiel::capabilities::ModelCapabilities {
        tools: true,
        ..sprachspiel::capabilities::ModelCapabilities::default()
    };
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .tools_active(true)
        .capabilities(caps)
        .build()
        .expect("valid ReplState");
    state.session.add_user_message("filler".to_string());
    state
        .session
        .add_assistant_message("note ".repeat(300), None, Some(3_500));
    fill_past_buffer(&mut state.session);
    assert!(state.pending_pre_tool.is_empty());
    let _capture_witness = state.session.id.clone();

    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let mut view = channel_view();
    let terminal = handle_user_message_stream(
        "<continuation_needed>\nReasoning paused: mid\nNext step: go on\n</continuation_needed>",
        &mut state,
        &mut view,
        llm_channel(),
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Cancelled);
    // Eight requests hit the wire: the pre-turn compaction (four chunks and
    // the consolidation), the tool-call exchange (two), and the continuation
    // request the stop landed on. Reaching index 7 proves the tool call was
    // processed — the capture it produced is what a direct write would have
    // persisted, so the zero-rows assertion below only holds because the
    // deferral kept it in the abandoned clone.
    assert_eq!(server.count(), 8);
    assert!(
        db.persisted_message_count(&state.session.id, "I will check the file")
            .expect("db")
            == 0,
        "zero residual rows: the abandoned capture queue dies with the clone"
    );
    assert_eq!(
        persisted_summary(&db, &state.session.id),
        Some(None),
        "the in-chain summary never commits on a cancelled turn"
    );
    let _ = std::fs::remove_dir_all(&config_dir_unused());
}

fn config_dir_unused() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("sprach-tools-tcf-{}", std::process::id()))
}

#[tokio::test]
async fn earlier_compaction_state_survives_another_cancelled_attempt() {
    // A session already compacted by an earlier turn (summary + tokens in
    // place) faces another attempt whose stop request lands first: the
    // discard touches nothing of the earlier state.
    let token = CancellationToken::new();
    token.cancel();
    let server = MockServer::spawn(|_| panic!("a cancelled attempt makes no model calls"));
    let llm_tx = llm_channel();
    let (mut session, db) = session_with_db();
    session.set_compacted_summary_with_range("earlier summary".to_string(), Some((0, 3)));
    for i in 0..3 {
        session.add_user_message(format!("question {i}"));
        session.add_assistant_message(format!("answer {i}"), None, Some(3_500 + i as u64));
    }
    assert!(db.conversation_has_prompt_tokens(&session.id).expect("db"));

    let mut view = channel_view();
    let outcome = CompactionContext {
        provider: &provider_mock(&server),
        model_config: &test_model(4096),
        session: &mut session,
        settings: &Settings::default(),
        agents_md: None,
        context_window: 4096,
        view: &mut view,
        llm_tx,
        cancel_token: Some(&token),
    }
    .compact_if_needed()
    .await;

    assert_eq!(outcome, CompactionOutcome::Cancelled);
    assert_eq!(
        session.compacted_summary.as_deref(),
        Some("earlier summary")
    );
    assert_eq!(session.compacted_range, Some((0, 3)));
    assert!(
        session
            .messages
            .iter()
            .any(|m| m.prompt_tokens == Some(3_502)),
        "earlier prompt tokens survive the discarded attempt"
    );
    assert_eq!(server.count(), 0);
}

#[test]
fn cancellation_stops_the_consolidation_pass_after_chunks_succeed() {
    // Layer 2 with two chunks: both chunk summaries come back, the stop
    // request lands on the terminal consolidation pass — it neither runs
    // nor commits, and the discarded whole carries nothing to the session.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(
        vec![
            MockServer::plain_body("chunk summary A"),
            MockServer::plain_body("chunk summary B"),
        ],
        move |idx| {
            eprintln!("HOOK-REQ idx={idx}");
            if idx == 2 {
                cancel_for_hook.cancel();
            }
        },
    );
    let llm_tx = llm_channel();
    let mut session = sixteen_session();
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
        panic!(
            "a cancelled consolidation pass must not return a summary (count={} outcome=Ok)",
            server.count()
        );
    };
    assert!(err.is_cancelled());
    assert_eq!(
        server.count(),
        3,
        "two chunk passes and the interrupted consolidation attempt"
    );
    assert!(session.compacted_summary.is_none());
    assert!(session.compacted_range.is_none());
}

#[test]
fn cancellation_reaches_the_truncation_layer_and_stops_it() {
    // Layer 2 fails with model errors, the truncation fallback starts, and
    // the stop request lands on it: no history is erased and no summary is
    // returned after a stop.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(vec![ScriptedResponse::Reject], move |idx| {
        if idx == 1 {
            cancel_for_hook.cancel();
        }
    });
    let llm_tx = llm_channel();
    let mut session = sixteen_session();
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
        panic!(
            "a cancelled truncation pass must not return a summary (count={} outcome=Ok)",
            server.count()
        );
    };
    assert!(err.is_cancelled());
    assert_eq!(
        server.count(),
        2,
        "one failed chunk pass and the interrupted truncation pass"
    );
    assert!(session.compacted_summary.is_none());
    assert!(session.compacted_range.is_none());
}

#[test]
fn cancellation_stops_a_recursion_of_depth_two() {
    // Layer 2 chunk summaries are huge, so the terminal consolidation must
    // recurse; the second depth's request takes the stop request and the
    // recursion stops there.
    let giant_summary = "summary piece ".repeat(9_000);
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(
        vec![
            MockServer::plain_body(&giant_summary),
            MockServer::plain_body(&giant_summary),
        ],
        move |idx| {
            if idx == 2 {
                cancel_for_hook.cancel();
            }
        },
    );
    let llm_tx = llm_channel();
    let mut session = sixteen_session();
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
        panic!(
            "a stopped recursion must not return a summary (count={} outcome=Ok)",
            server.count()
        );
    };
    assert!(err.is_cancelled());
    assert_eq!(
        server.count(),
        3,
        "two depth-0 chunk passes and one depth-1 attempt"
    );
    assert!(session.compacted_summary.is_none());
    assert!(session.compacted_range.is_none());
}

#[tokio::test]
async fn nested_continuation_cancels_at_its_second_request() {
    // The first continuation comes back asking for one more; the stop
    // request lands on the second continuation request. The whole chain —
    // both responses' text, the captures — aborts, and no residual row
    // stays behind.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let tag_a =
        "<continuation_needed>\nReasoning paused: mid\nNext step: go on\n</continuation_needed>";
    let tag_b = "<continuation_needed>\nReasoning paused: at branch\nNext step: one more\n</continuation_needed>";
    let server = MockServer::spawn_scripted(
        vec![
            // Stage one: the initial turn asks for a continuation.
            MockServer::plain_body(&format!("part one\n{tag_a}")),
            // Stage two: the FIRST continuation comes back asking for one
            // more — this is what drives the chain into its nested branch.
            // It travels on the non-streaming chat path, so it needs the
            // JSON completion shape, not an SSE body.
            MockServer::json_body(&format!("part two\n{tag_b}")),
            // Stage three: the SECOND (nested) continuation request takes
            // the stop request; it never produces a usable response.
            ScriptedResponse::Reject,
        ],
        move |idx| {
            if idx == 2 {
                cancel_for_hook.cancel();
            }
        },
    );
    let (session, db) = session_with_db();
    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");
    state.session.add_user_message("seed".to_string());
    let mirror: sprachspiel::chat::fact_tracker::SharedFactTracker = Arc::new(
        std::sync::Mutex::new(sprachspiel::chat::fact_tracker::SessionFactTracker::default()),
    );
    let mut view = channel_view();
    let (llm_tx, mut llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let terminal = handle_user_message_stream(
        "<continuation_needed>\nReasoning paused: start\nNext step: begin\n</continuation_needed>",
        &mut state,
        &mut view,
        llm_tx,
        token,
        mirror,
    )
    .await;
    while let Ok(ev) = llm_rx.try_recv() {}
    assert_eq!(terminal, TurnTerminal::Cancelled);
    assert!(
        db.persisted_message_count(&state.session.id, "part one")
            .expect("db")
            == 0
            && db
                .persisted_message_count(&state.session.id, "part two")
                .expect("db")
                == 0,
        "an aborted nested chain leaves no residual rows"
    );
    assert_eq!(
        server.count(),
        3,
        "turn, the nested-driving continuation, then the cancelled request"
    );
}

#[tokio::test]
async fn compaction_cancels_during_its_real_await_and_commits_nothing() {
    // The summary pass is genuinely in flight when the stop request lands:
    // the connection opened, the response never completes. The typed
    // cancellation wins over whatever the pass would have produced.
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn_scripted(vec![ScriptedResponse::Reject], move |_| {
        cancel_for_hook.cancel()
    });
    let llm_tx = llm_channel();
    let (mut session, db) = session_with_db();
    session.add_user_message("filler".to_string());
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
        cancel_token: Some(&token),
    }
    .compact_if_needed()
    .await;

    assert_eq!(outcome, CompactionOutcome::Cancelled);
    assert_eq!(server.count(), 1, "only the in-flight pass hit the wire");
    assert!(session.compacted_summary.is_none());
}

#[tokio::test]
async fn post_response_commit_reaches_sqlite_too() {
    // Extends cancel_after_response: the preserved response is not only in
    // the live session — it is in the SQLite rows the terminal saved.
    let server = MockServer::spawn(|_| panic!("no model calls"));
    let llm_tx = llm_channel();
    let (mut session, db) = session_with_db();
    session.add_user_message("long message to fill the buffer".to_string());
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

    let mut view = channel_view();
    let processed =
        process_send_result(&mut state, result, None, &mut view, llm_tx, Some(&token)).await;

    assert_eq!(processed, ProcessResult::Success);
    assert_eq!(server.count(), 0);
    assert!(
        db.persisted_message_count(&state.session.id, "the final answer")
            .expect("db")
            > 0,
        "the preserved response reaches the SQLite rows with the commit"
    );
}
