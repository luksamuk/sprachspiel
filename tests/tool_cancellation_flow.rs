//! Cooperative tool cancellation (Ctrl+C against a running tool), end to
//! end: a real coordinator, a real provider driven by the counting SSE
//! mock, a real `sleep` child. The stop request lands mid-tool; the tool
//! reaches its own safe point, reports the typed `ToolCancelled`, and the
//! turn ends cancelled with no further request — and the child is gone
//! from the process table.

mod common;

use std::time::Duration;

use common::{channel_view, fresh_mirror, provider_mock, session_with_db, test_model, MockServer};
use tokio_util::sync::CancellationToken;

use sprachspiel::chat::llm_event::LlmEvent;
use sprachspiel::chat::repl::{TurnTerminal, handle_user_message_stream};
use sprachspiel::chat::repl_state::ReplStateBuilder;
use sprachspiel::settings::Settings;

/// Nothing outlives the cancelled turn: a child matching the unique argv
/// pair must be terminally gone — killed AND reaped, not a zombie.
fn assert_no_such_process(argv_match: &str) {
    let lookup = std::process::Command::new("ps")
        .args(["-eo", "args"])
        .output()
        .expect("ps available");
    let listing = String::from_utf8_lossy(&lookup.stdout);
    let alive = listing
        .lines()
        .filter(|line| line.contains(argv_match) && !line.contains("ps -eo"))
        .count();
    assert_eq!(alive, 0, "the killed child must be reaped, found: {listing}");
}

#[tokio::test]
async fn cancellation_mid_tool_kills_and_reaps_and_starts_no_new_request() {

    // Runtime-unique argv: no wrapper process' command line matches it.
    let uid: u64 = std::process::id() as u64 * 7919 + 999;
    let sleep_argv = format!("sleep {uid}");
    let token = CancellationToken::new();
    let handle = tokio::runtime::Handle::current();

    let body = MockServer::tool_call_body(
        "run_command",
        &format!(
            r#"{{"command_line":"{sleep_argv}","head":null,"tail":null,"timeout_seconds":null}}"#
        ),
    );
    let server = MockServer::spawn_sequence(vec![body]);
    let cancel_for_hook = token.clone();
    let hook_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    std::thread::spawn(move || {
        // Wait long enough for the first response to be consumed and the
        // tool loop to reach the cancelled child's wait, then land the
        // stop request: after the pre-call guard, before the impossible
        // completion.
        std::thread::sleep(Duration::from_millis(400));
        cancel_for_hook.cancel();
        hook_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    // `sleep` enters the whitelist through the test's own tools.toml, read
    // once by the cached config loader — the env must land before any
    // run_command in this process.
    let config_dir =
        std::env::temp_dir().join(format!("sprach-tools-test-{}", std::process::id()));
    std::fs::create_dir_all(config_dir.join("sprachspiel")).expect("create test config dir");
    std::fs::write(
        config_dir.join("sprachspiel/tools.toml"),
        "[external.tools.sleep]\nbinary = \"sleep\"\nenabled = true\n",
    )
    .expect("write test tools.toml");
    // SAFETY: single test binary, before any cached config load.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &config_dir) };

    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    let cfg = test_model(4096);
    let provider = provider_mock(&server);
    let caps = sprachspiel::capabilities::ModelCapabilities {
        tools: true,
        ..sprachspiel::capabilities::ModelCapabilities::default()
    };
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg)
        .provider(provider)
        .settings(Settings::default())
        .tools_active(true)
        .capabilities(caps)
        .build()
        .expect("valid ReplState");

    let mirror = fresh_mirror();
    let _mirror_read = std::sync::Arc::clone(&mirror);
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();

    let terminal = handle_user_message_stream(
        "run the long command",
        &mut state,
        &mut view,
        llm_tx,
        token,
        mirror,
    )
    .await;

    assert_eq!(
        terminal,
        TurnTerminal::Cancelled,
        "the cancelled tool owns the end of the turn"
    );
    assert_eq!(
        server.count(),
        1,
        "zero further requests after the ToolCancelled turn end"
    );
    assert_no_such_process(&sleep_argv);
    handle_park().await;
}

/// The mocked cancel thread is not part of the runtime; park once so the
/// runtime has no in-flight work left before the test thread returns.
async fn handle_park() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

// ── Structural guarantees (source sensors over the guarantee's files) ─

/// The cancellation is cooperative by construction: no tool runs on a
/// detached task that could outlive the turn's terminal, and the guarantee
/// cannot be silently broken by an abort mechanism.
#[test]
fn no_tool_runs_on_a_detached_task() {
    let coordinator = include_str!("../src/chat/coordinator.rs");
    // The tool loop awaits tool.call inline — the one await in the loop.
    assert!(
        coordinator.contains("tool.call(args.clone())"),
        "the tool loop awaits tools inline"
    );
    // No spawn+abort shortcut exists around tool execution.
    assert!(
        !coordinator.contains("JoinHandle::abort") && !coordinator.contains(".abort()"),
        "aborting futures is not a cancellation mechanism"
    );

    // Production side only (the file's own test module may spawn helper
    // tasks for its canceller fixtures).
    let run_cmd = include_str!("../src/tools/run_cmd.rs");
    let run_cmd_prod = run_cmd.split("#[cfg(test)]").next().expect("split at tests");
    assert!(
        !run_cmd_prod.contains("tokio::spawn"),
        "run_command owns its child in its own future, never on a detached task"
    );
}

/// File writes are deliberately outside every stop request: temp+rename
/// finishes the atomic operation in flight, and the non-atomic append never
/// consults the token at all.
#[test]
fn file_writes_are_deliberately_non_interruptible() {
    let files = include_str!("../src/tools/files_write.rs");
    assert!(
        !files.contains("get_cancel_token") && !files.contains("cancellation_requested"),
        "no file write consults the turn token"
    );
    assert!(
        files.contains("atomic_write") && files.contains("append_file"),
        "the module documents which operations are atomic and which are not"
    );
}

/// `/new` cannot open a new session while the old tool still holds the
/// turn — the guard lives in the event loop, next to the command's
/// session-forget hygiene.
#[test]
fn new_session_is_refused_while_a_tool_holds_the_turn() {
    let event_loop = include_str!("../src/chat/event_loop.rs");
    assert!(
        event_loop.contains("A tool is still executing; wait for it to reach a safe point"),
        "the /new guard keeps the wait real"
    );
}

/// A completion that raced the request does not become a new LLM request —
/// the Coordinator owns that check right after the result is shown.
#[test]
fn completed_tools_keep_their_result_and_never_summon_another_request() {
    let coordinator = include_str!("../src/chat/coordinator.rs");
    assert!(
        coordinator.contains("completed while cancellation was pending"),
        "the post-result cancellation check exists in the tool loop"
    );
    let _ = Duration::from_secs(0);
}
