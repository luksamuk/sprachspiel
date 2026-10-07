//! End-to-end wiring check for compaction fact staples (plan Tasks 13+14).
//!
//! The unit tests in `src/chat/fact_tracker.rs` prove the tracker and the
//! renderer; `mod finalize_summary_tests` in `src/chat/core.rs` proves the
//! append-once / prose-untouched / empty-unchanged contract of
//! `finalize_summary`. What they cannot prove is that the pieces are wired
//! together through API reachable from outside the crate — and that wiring is
//! where every previous defect in this area lived.
//!
//! Honest scope (why there is no live-LLM e2e here):
//!
//! * `finalize_summary` is private to `chat::core`, and `compact_conversation`
//!   drives a live model (`compact_with_llm`), so an integration test in
//!   `tests/` can neither call the single exit nor run the real compactor
//!   without standing up a model server; promoting either to `pub` would leak
//!   crate internals. The plan's e2e-with-LLM assertions are pinned instead:
//!   * here, over the full public pipeline: `record_tool` (the coordinator
//!     hook's input) -> `ChatSession::merge_fact_tracker` (the turn-exit flush
//!     it drives) -> `render_staple_block` fed from
//!     `ChatSession::get_last_user_message` (what `finalize_summary` reads) ->
//!     the caller's append -> `set_compacted_summary_with_range` -> the staple
//!     riding `compacted_summary` into `get_messages_for_llm`, i.e. the context
//!     the LLM actually receives after a compaction;
//!   * at unit level in `core.rs:mod finalize_summary_tests`, which asserts
//!     directly on `finalize_summary` — the single exit of
//!     `compact_conversation` — for append-exactly-once, prose-untouched and
//!     empty-unchanged (the equivalent of running the layers with a live LLM
//!     would end at that same exit).
//! * The coordinator hook feeding `record_tool` at tool-execution time only
//!   runs inside a live tool-call turn; its merge side (facts surviving the
//!   merge and the SQLite round-trip) is pinned by the unit tests in
//!   `src/chat/session.rs`.

use sprachspiel::chat::fact_tracker::{SessionFactTracker, render_staple_block};
use sprachspiel::chat::session::ChatSession;

/// One turn's worth of tool executions, as the coordinator hook would record
/// them: a modification, a read, and a command that passed.
fn a_turn_tracker_with_facts() -> SessionFactTracker {
    let mut turn = SessionFactTracker::default();
    turn.record_tool(
        "write_file",
        r#"{"path":"src/real.rs"}"#,
        "Successfully created",
        false,
    );
    turn.record_tool(
        "read_file",
        r#"{"path":"src/read_only.rs"}"#,
        "1|hello",
        false,
    );
    turn.record_tool(
        "run_command",
        r#"{"command_line":"make lint"}"#,
        "ok",
        false,
    );
    turn
}

#[test]
fn a_session_that_recorded_facts_renders_a_staple_naming_them() {
    let mut session = ChatSession::new("test-model".into(), None, true);
    session.add_user_message("fix the parser".into());
    session.merge_fact_tracker(&a_turn_tracker_with_facts());

    assert!(
        !session.fact_tracker.is_empty(),
        "the merge must carry the turn's facts into the session"
    );

    // What finalize_summary does: render from the session tracker with the
    // latest user request taken from the session, not from the compacted
    // slice.
    let last_request = session.get_last_user_message().map(|m| m.content.as_str());
    let block = render_staple_block(&session.fact_tracker, last_request);

    assert!(
        block.contains("src/real.rs"),
        "the modified path must appear: {block}"
    );
    assert!(
        block.contains("src/read_only.rs"),
        "a merely-read path must appear, kept distinct from modified: {block}"
    );
    assert!(
        block.contains("make lint -> PASS"),
        "the command and its outcome must appear: {block}"
    );
    assert!(
        block.contains("fix the parser"),
        "the user request rides the staple: {block}"
    );
    assert!(
        !block.contains("<unresolved-error>"),
        "the harness staple must omit the block it cannot know: {block}"
    );
}

#[test]
fn an_empty_session_produces_no_staple() {
    let untouched = ChatSession::new("test-model".into(), None, true);
    assert!(
        untouched.fact_tracker.is_empty(),
        "a session that touched no tool has nothing to staple"
    );

    // A turn-exit flush that recorded nothing (no tools ran) must not invent
    // facts when merged.
    let mut merged_nothing = ChatSession::new("test-model".into(), None, true);
    merged_nothing.merge_fact_tracker(&SessionFactTracker::default());
    assert!(
        merged_nothing.fact_tracker.is_empty(),
        "an empty merge must leave the session tracker empty"
    );
}

#[test]
fn the_staple_states_failures_without_claiming_resolution() {
    let mut session = ChatSession::new("test-model".into(), None, true);
    session.add_user_message("fix the parser".into());

    let mut turn = SessionFactTracker::default();
    // A write that failed changed nothing — it must not enter modified-files.
    turn.record_tool(
        "write_file",
        r#"{"path":"src/blocked.rs"}"#,
        "BLOCKED: path denied",
        true,
    );
    // The command ran and failed: the tracker records the fact, but the
    // harness cannot know whether the error was later fixed.
    turn.record_tool(
        "run_command",
        r#"{"command_line":"make lint"}"#,
        "Error: no",
        true,
    );
    session.merge_fact_tracker(&turn);

    assert!(
        !session
            .fact_tracker
            .modified_files()
            .iter()
            .any(|p| p.contains("src/blocked.rs")),
        "a failed write is not a modified file"
    );

    let last_request = session.get_last_user_message().map(|m| m.content.as_str());
    let block = render_staple_block(&session.fact_tracker, last_request);

    assert!(
        block.contains("make lint -> FAIL"),
        "the failure itself is stated, with its outcome: {block}"
    );
    assert!(
        !block.contains("unresolved-error"),
        "the forbidden unresolved-error block must not appear in any form: {block}"
    );
}

#[test]
fn the_staple_is_appended_exactly_once_after_untouched_prose() {
    let mut session = ChatSession::new("test-model".into(), None, true);
    session.add_user_message("fix the parser".into());
    session.merge_fact_tracker(&a_turn_tracker_with_facts());

    let last_request = session.get_last_user_message().map(|m| m.content.as_str());
    let block = render_staple_block(&session.fact_tracker, last_request);

    // Exactly what finalize_summary (chat::core) composes at its single exit:
    // `format!("{summary}\n\n{block}")`. Asserted here on the composed
    // contract because the fn itself is crate-private.
    let prose = "## Goal\nFix the parser.";
    let summary = format!("{prose}\n\n{block}");

    assert!(
        summary.ends_with("</compaction_facts>"),
        "the block is appended last, never merged into the prose"
    );
    assert!(summary.starts_with(prose), "the model's prose is untouched");
    assert_eq!(
        summary.matches("<compaction_facts>").count(),
        1,
        "exactly one staple per summary — the single-exit refactor is what guarantees it"
    );
}

#[test]
fn the_staple_rides_compacted_summary_into_the_next_context() {
    let mut session = ChatSession::new("test-model".into(), None, true);
    session.add_user_message("fix the parser".into());
    session.merge_fact_tracker(&a_turn_tracker_with_facts());

    let last_request = session.get_last_user_message().map(|m| m.content.as_str());
    let block = render_staple_block(&session.fact_tracker, last_request);
    let summary = format!("## Goal\nFix the parser.\n\n{block}");

    // What the compaction driver does with the returned summary
    // (compaction.rs): store it on the session; the tracker itself is NOT
    // reset — facts are cumulative across compactions.
    session.set_compacted_summary_with_range(summary, None);
    assert!(
        !session.fact_tracker.is_empty(),
        "compaction must not drop the accumulated facts"
    );

    // The next turn adds exactly one message past the compacted range.
    session.add_user_message("and now, continue".into());

    let context = session.get_messages_for_llm("generic system prompt", false);

    assert_eq!(
        context.len(),
        3,
        "system + compacted summary + the new user message: {:?}",
        context.iter().map(|m| &m.content).collect::<Vec<_>>()
    );

    let summary_message = &context[1];
    assert!(
        summary_message
            .content
            .starts_with("Previous conversation summary:"),
        "the compacted summary rides as a system message: {}",
        summary_message.content
    );
    assert_eq!(
        summary_message
            .content
            .matches("<compaction_facts>")
            .count(),
        1,
        "the staple survives the ride, still exactly once"
    );
    assert!(
        summary_message.content.contains("src/real.rs")
            && summary_message.content.contains("src/read_only.rs")
            && summary_message.content.contains("make lint -> PASS")
            && summary_message.content.contains("fix the parser"),
        "the recorded facts reach the context the LLM receives: {}",
        summary_message.content
    );
    assert!(
        !summary_message.content.contains("<unresolved-error>"),
        "the forbidden block does not reappear downstream"
    );

    assert_eq!(context[2].content, "and now, continue");
    assert!(
        !context[2].content.contains("<compaction_facts>"),
        "the staple lives only in the summary, not duplicated on the turn's tail"
    );
}
