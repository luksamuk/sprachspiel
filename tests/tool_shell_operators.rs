//! Shell operators in `run_command`, end to end: a real
//! coordinator, a real provider driven by the counting SSE mock, real
//! child processes. `allow_shell_operators` defaults true — compound
//! commands (pipes, chaining, redirects, substitution) execute as-is under
//! the Landlock sandbox, the per-binary whitelist still applies to every
//! top-level segment, and cancellation keeps the direct-child kill/reap
//! contract of the cooperative tool cancellation.

mod common;

use std::time::Duration;

use common::{channel_view, fresh_mirror, provider_mock, session_with_db, test_model, MockServer};
use tokio_util::sync::CancellationToken;

use sprachspiel::chat::llm_event::LlmEvent;
use sprachspiel::chat::repl::{TurnTerminal, handle_user_message_stream};
use sprachspiel::chat::repl_state::ReplStateBuilder;
use sprachspiel::settings::Settings;

/// Nothing outlives the cancelled turn: a process matching the unique argv
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

/// Test config directory with `sleep`, `echo` and `tr` whitelisted —
/// XDG_CONFIG_HOME must land before any run_command in this process (the
/// cached loader).
fn install_test_config() {
    let config_dir =
        std::env::temp_dir().join(format!("sprach-shell-op-test-{}", std::process::id()));
    std::fs::create_dir_all(config_dir.join("sprachspiel")).expect("create test config dir");
    std::fs::write(
        config_dir.join("sprachspiel/tools.toml"),
        "[external]\ndefault_timeout = 5\n\n\
         [external.tools.sleep]\nbinary = \"sleep\"\nenabled = true\n\n\
         [external.tools.echo]\nbinary = \"echo\"\nenabled = true\n\n\
         [external.tools.tr]\nbinary = \"tr\"\nenabled = true\n",
    )
    .expect("write test tools.toml");
    // SAFETY: single test binary, before any cached config load.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &config_dir) };
}

/// Build a ReplState wired to the mock provider.
async fn build_state(server: &MockServer) -> sprachspiel::chat::repl_state::ReplState {
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    let cfg = test_model(4096);
    let provider = provider_mock(server);
    let caps = sprachspiel::capabilities::ModelCapabilities {
        tools: true,
        ..sprachspiel::capabilities::ModelCapabilities::default()
    };
    ReplStateBuilder::new()
        .session(session)
        .model_config(cfg)
        .provider(provider)
        .settings(Settings::default())
        .tools_active(true)
        .capabilities(caps)
        .build()
        .expect("valid ReplState")
}

#[tokio::test]
async fn compound_pipeline_executes_end_to_end() {
    install_test_config();
    let tool_body = MockServer::tool_call_body(
        "run_command",
        r#"{"command_line":"echo test-pipeline-payload | tr a b","head":null,"tail":null,"timeout_seconds":null}"#,
    );
    let server = MockServer::spawn_sequence(vec![tool_body, MockServer::plain_body("done")]);

    let mut state = build_state(&server).await;
    let mirror = fresh_mirror();
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();

    let token = CancellationToken::new();
    let terminal = handle_user_message_stream(
        "run the pipeline",
        &mut state,
        &mut view,
        llm_tx,
        token,
        mirror,
    )
    .await;

    assert_eq!(
        terminal,
        TurnTerminal::Complete,
        "the compound command runs under the shell"
    );
    assert_eq!(
        server.count(),
        2,
        "the pipeline result goes back to the model"
    );
}

#[tokio::test]
async fn cancelling_a_compound_command_kills_and_reaps_the_shell() {
    install_test_config();
    let uid: u64 = std::process::id() as u64 * 7919 + 1271;
    // A short sleep keeps the orphaned pipeline segment short-lived: only
    // the direct child (the shell) is killed and reaped, descendants are an
    // explicit boundary.
    let tool_body = MockServer::tool_call_body(
        "run_command",
        &format!(
            r#"{{"command_line":"sleep 1.2 | tr a {uid}","head":null,"tail":null,"timeout_seconds":null}}"#
        ),
    );
    let server = MockServer::spawn_sequence(vec![tool_body]);
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    std::thread::spawn(move || {
        // Land the stop request mid-tool: after the pre-call guard, before
        // the compound command can finish.
        std::thread::sleep(Duration::from_millis(400));
        cancel_for_hook.cancel();
    });

    let mut state = build_state(&server).await;
    let mirror = fresh_mirror();
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();

    let terminal = handle_user_message_stream(
        "run the pipeline",
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
    // The direct child of run_command is the shell process; the stop request
    // kills and reaps it. Descendants of the pipeline (here `sleep` and
    // `tr`) are an explicit boundary — they die on their own or with the
    // shell's pipe teardown, and no process-group kill is claimed.
    assert_no_such_process(&format!("sleep 1.2 | tr a {uid}"));
    handle_park().await;
}

fn handle_park() -> impl std::future::Future<Output = ()> {
    tokio::time::sleep(Duration::from_millis(1))
}
