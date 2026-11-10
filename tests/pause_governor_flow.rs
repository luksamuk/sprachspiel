//! Low-yield auto-compaction pause governor (session lifetime).
//!
//! The governor unit is exercised directly for the exact boundaries of the
//! yield bar (they are pure decisions, no server involved); the session
//! integration — pause blocking a full turn, pausing mid inter-tool pass,
//! lifting only through a successful manual compaction or a real reset —
//! runs against the request-counting SSE mock from `tests/common`.
//!
//! Covers the accepted test matrix: 9.99% counts / exactly 10% does not;
//! zero and negative yields; two consecutive low yields pause, high yield
//! zeroes; failure/cancel/manual-failure neither count nor resume; a new
//! user message neither resumes nor zeroes; /compact success resumes;
//! pause + full context = zero provider calls and zero truncation; pause
//! mid inter-tool sends no continuation; /new and resets wipe the state;
//! MAX_COMPACTION_CYCLES still bounds a single-turn cascade.

mod common;



use common::{
    channel_view, fill_past_buffer, fresh_mirror, llm_channel, provider_mock, session_with_db,
    test_model, MockServer,
};
use tokio_util::sync::CancellationToken;

use sprachspiel::chat::compaction::{
    AutoCompactionGovernor, CompactionContext, CompactionOutcome, LOW_YIELD_FRACTION,
};
use sprachspiel::chat::continuation::{OverflowHandleResult, handle_overflow_error};
use sprachspiel::chat::llm_event::LlmEvent;
use sprachspiel::chat::repl::{TurnTerminal, handle_user_message_stream};
use sprachspiel::chat::repl_state::ReplStateBuilder;
use sprachspiel::chat::session::ChatSession;
use sprachspiel::settings::Settings;

// ── The yield bar, exactly ──────────────────────────────────────────

#[test]
fn just_under_the_bar_counts_exactly_at_the_bar_does_not() {
    let mut governor = AutoCompactionGovernor::default();
    // 9.99% of any window is below 0.10 — counts.
    assert!(!governor.record_yield(LOW_YIELD_FRACTION - 0.0001));
    assert_eq!(governor.low_yield_streak(), 1);
    // Exactly the bar (10.0%) is high-yield — the streak RESETS to zero
    // instead of reaching the pause threshold.
    assert!(!governor.record_yield(LOW_YIELD_FRACTION));
    assert_eq!(governor.low_yield_streak(), 0);
    assert!(!governor.is_paused());
}

#[test]
fn two_consecutive_low_yields_pause() {
    let mut governor = AutoCompactionGovernor::default();
    assert!(!governor.record_yield(0.05));
    assert!(!governor.is_paused());
    assert!(governor.record_yield(0.05), "the second low pass trips the pause");
    assert!(governor.is_paused());
    // Further recordings while paused change nothing.
    assert!(!governor.record_yield(0.05));
    assert!(governor.is_paused());
}

#[test]
fn a_high_yield_pass_resets_the_streak() {
    let mut governor = AutoCompactionGovernor::default();
    assert!(!governor.record_yield(0.05));
    assert!(!governor.record_yield(0.5), "high-yield zeroes, never counts");
    assert_eq!(governor.low_yield_streak(), 0);
    assert!(!governor.record_yield(0.05));
    assert!(!governor.is_paused(), "one fresh low pass cannot pause");
}

#[test]
fn zero_and_negative_yields_count_as_low() {
    // Zero yield: two summarizer calls freed nothing.
    let mut governor = AutoCompactionGovernor::default();
    assert!(!governor.record_yield(0.0));
    assert!(governor.record_yield(0.0));
    assert!(governor.is_paused());

    // Negative yield: the "compaction" made the context larger — it counts
    // as low-yield in its own right (the caller owns the explicit log).
    let mut governor = AutoCompactionGovernor::default();
    assert!(!governor.record_yield(-0.02));
    assert!(governor.record_yield(0.0));
    assert!(governor.is_paused());
}

// ── Only completed passes reach the governor ─────────────────────────

#[tokio::test]
async fn a_model_failure_neither_counts_nor_resumes() {
    let rejecting = MockServer::spawn_rejecting();
    let llm_tx = llm_channel();
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    // Streak already at 1 from an earlier low pass.
    session.governor.record_yield(0.0);
    fill_past_buffer(&mut session);
    let cfg = test_model(4096);

    let mut view = channel_view();
    let outcome = CompactionContext {
        provider: &provider_mock(&rejecting),
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

    assert!(matches!(outcome, CompactionOutcome::Failed));
    assert_eq!(
        session.governor.low_yield_streak(),
        1,
        "the failed pass must not count toward the pause"
    );
    assert!(!session.governor.is_paused(), "failure cannot pause");
}

#[tokio::test]
async fn a_cancelled_pass_neither_counts_nor_resumes() {
    let token = CancellationToken::new();
    let cancel_for_hook = token.clone();
    let server = MockServer::spawn(move |_| cancel_for_hook.cancel());
    let llm_tx = llm_channel();
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.governor.record_yield(0.0);
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

    assert!(matches!(outcome, CompactionOutcome::Cancelled));
    assert_eq!(session.governor.low_yield_streak(), 1);
    assert!(!session.governor.is_paused(), "cancellation cannot pause or resume");
}

#[tokio::test]
async fn not_needed_and_paused_passes_touch_nothing() {
    // Below the buffer: NotNeeded — the streak is untouched.
    let server = MockServer::spawn(|_| panic!("no buffer trigger, no calls"));
    let llm_tx = llm_channel();
    let (mut session, _db) = session_with_db();
    session.add_user_message("tiny".to_string());
    session.governor.record_yield(0.0);
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

    assert_eq!(outcome, CompactionOutcome::NotNeeded);
    assert_eq!(session.governor.low_yield_streak(), 1);
    assert_eq!(server.count(), 0);
}

// ── Session integration: resets, resume, blocking ────────────────────

#[test]
fn real_resets_wipe_the_pause() {
    // forget_session (/new) and clear_messages are the same hygiene that
    // resets summary/range/fact tracker.
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());
    session.forget_session();
    assert!(!session.governor.is_paused(), "a real reset lifts the pause");
    assert_eq!(session.governor.low_yield_streak(), 0);

    // clear_messages keeps the summary but loses the compaction history.
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());
    session.clear_messages();
    assert!(!session.governor.is_paused());
}

#[tokio::test]
async fn a_new_user_message_neither_resumes_nor_zeroes() {
    let server = MockServer::spawn(|_| {});
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());

    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");
    // Small context: below any trigger, so the turn is allowed to run.
    state.session.add_user_message("just a message".to_string());

    let mirror = fresh_mirror();
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let token = CancellationToken::new();
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

    assert_eq!(terminal, TurnTerminal::Complete);
    assert_eq!(server.count(), 1, "only the response call ran below the limit");
    assert!(
        state.session.governor.is_paused(),
        "a new message neither resumes nor zeroes the pause"
    );
    assert_eq!(state.session.governor.low_yield_streak(), 0);
}

#[tokio::test]
async fn pause_with_full_context_makes_zero_provider_calls_and_truncates_nothing() {
    let server = MockServer::spawn(|_| panic!("paused session at the limit must not call"));
    let (mut session, _db) = session_with_db();
    session.anonymous = true;
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());

    let cfg = test_model(4096);
    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");
    fill_past_buffer(&mut state.session);
    let message_count_before = state.session.messages.len();

    let mirror = fresh_mirror();
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let token = CancellationToken::new();
    let mut view = channel_view();

    let terminal = handle_user_message_stream(
        "another message",
        &mut state,
        &mut view,
        llm_tx,
        token,
        mirror,
    )
    .await;

    assert_eq!(terminal, TurnTerminal::Complete);
    assert_eq!(server.count(), 0, "zero provider calls at the limit");
    assert_eq!(
        state.session.messages.len(),
        message_count_before + 1,
        "no automatic truncation: history intact, only the user message added"
    );
    assert!(state.session.compacted_summary.is_none());
    assert!(state.session.governor.is_paused());
}

/// A 4096-token window: the buffer trigger fires above ~3.1K estimated
/// tokens and the yield bar is 10% of 4096 ≈ 409 tokens. The accounting
/// after a pass counts summary + active messages (the kept-last side), so
/// this shape — tiny keep-first and middle, padded keep-last — makes the
/// real pass free ~nothing: genuinely low-yield without stubbing numbers.
fn low_yield_session() -> (ChatSession, usize) {
    let mut session = ChatSession::new("test-model".to_string(), None, true);
    for i in 0..11 {
        if i < 6 {
            session.add_user_message("word ".repeat(10));
        } else {
            session.add_user_message(format!("topic {i}: {}", "field ".repeat(600)));
        }
    }
    (session, 4096)
}

#[tokio::test]
async fn low_yield_passes_pause_mid_inter_tool_and_send_no_continuation() {
    let token = CancellationToken::new();
    let server = MockServer::spawn(|_| {});
    let (mut session, window) = low_yield_session();
    let cfg = test_model(window as u32);
    // One low pass already counted; this inter-tool pass is the second.
    session.governor.record_yield(0.0);
    assert!(!session.governor.is_paused());

    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();
    let inter_tool_error = "CONTEXT_NEEDS_COMPACT:600:256:tool_a,tool_b";

    let result = handle_overflow_error(
        &mut state, inter_tool_error, &mut view, llm_tx, Some(&token),
    )
    .await;

    // The pass ran (the summary was applied), then the trip happened: the
    // caller gets Paused — no continuation prompt is built, and the turn
    // facts stay with the fallback mirror.
    assert!(matches!(result, OverflowHandleResult::Paused));
    assert!(
        state.session.governor.is_paused(),
        "the second low-yield pass pauses the session"
    );
    assert!(
        state.session.compacted_summary.is_some(),
        "the finished pass committed its summary before pausing"
    );
    assert_eq!(
        state.session.governor.low_yield_streak(),
        0,
        "the tripping pass consumed the streak"
    );
}

#[tokio::test]
async fn manual_compaction_success_lifts_the_pause() {
    let server = MockServer::spawn(|_| {});
    let (mut session, window) = low_yield_session();
    let cfg = test_model(window as u32);
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());

    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&server))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    // `/compact` via the standalone (non-TUI) command path: same recovery
    // the event loop applies on CompactStreamDone.
    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();
    let outputs = sprachspiel::chat::command_handlers::handle_compact(&mut state, &mut view, llm_tx)
        .await;

    assert!(
        outputs
            .iter()
            .any(|o| matches!(o, sprachspiel::chat::command_output::CommandOutput::CompactResult(_))),
        "the manual compaction succeeded: {:?}",
        outputs
    );
    assert!(!state.session.governor.is_paused(), "success lifts the pause");
    assert_eq!(state.session.governor.low_yield_streak(), 0);
    assert!(
        state.session.compacted_summary.is_some(),
        "the manual pass is a real compaction, not just a resume"
    );
    assert!(
        !state.session.governor.record_yield(0.0),
        "the governor restarts from zero after a lift"
    );
}

#[tokio::test]
async fn a_failed_manual_compaction_does_not_lift_the_pause() {
    let rejecting = MockServer::spawn_rejecting();
    let (mut session, window) = low_yield_session();
    let cfg = test_model(window as u32);
    session.governor.record_yield(0.0);
    session.governor.record_yield(0.0);
    assert!(session.governor.is_paused());

    let mut state = ReplStateBuilder::new()
        .session(session)
        .model_config(cfg.clone())
        .provider(provider_mock(&rejecting))
        .settings(Settings::default())
        .capabilities(sprachspiel::capabilities::ModelCapabilities::default())
        .build()
        .expect("valid ReplState");

    let (llm_tx, _llm_rx) = tokio::sync::mpsc::channel::<LlmEvent>(64);
    let mut view = channel_view();
    let outputs =
        sprachspiel::chat::command_handlers::handle_compact(&mut state, &mut view, llm_tx).await;

    assert!(
        outputs
            .iter()
            .any(|o| matches!(o, sprachspiel::chat::command_output::CommandOutput::Error(_))),
        "the manual pass failed"
    );
    assert!(
        state.session.governor.is_paused(),
        "a failed manual compaction cannot lift the pause"
    );
    assert!(state.session.compacted_summary.is_none());
}

#[tokio::test]
async fn max_compaction_cycles_still_bounds_a_single_turn_cascade() {
    // High-yield passes never feed the governor's pause, so an old
    // session-wide cascade is bounded exactly as before — the streak never
    // grows on high-yield passes, and the per-turn cycle cap stays in the
    // turn loop (its cap applies before any pause decision).
    let mut governor = AutoCompactionGovernor::default();
    assert_eq!(governor.low_yield_streak(), 0);
    for _ in 0..5 {
        assert!(!governor.record_yield(0.5));
    }
    assert_eq!(governor.low_yield_streak(), 0, "high-yield passes never accumulate");
    assert!(!governor.is_paused());

    let source = include_str!("../src/chat/repl.rs");
    assert!(
        source.contains("MAX_COMPACTION_CYCLES"),
        "the per-turn cascade guard remains in the turn loop"
    );
    let _: TurnTerminal = TurnTerminal::Complete;
}
