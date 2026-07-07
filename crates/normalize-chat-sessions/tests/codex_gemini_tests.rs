//! Integration tests for the rewritten Codex and Gemini CLI session parsers.
//!
//! All tests run against synthetic-but-faithful fixtures built from the upstream
//! type definitions — not against real captured files from a live installation.

use normalize_chat_sessions::{
    CodexFormat, ContentBlock, GeminiCliFormat, Role, SessionLocation, SessionRef, SessionSource,
};
use std::path::Path;
use std::time::SystemTime;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn make_file_ref(format: &'static str, path: std::path::PathBuf) -> SessionRef {
    SessionRef {
        format,
        location: SessionLocation::File(path.clone()),
        path,
        mtime: SystemTime::UNIX_EPOCH,
        parent_session_id: None,
        agent_id: None,
        subagent_type: None,
    }
}

// ── CodexFormat ───────────────────────────────────────────────────────────────

#[test]
fn codex_detect_high_on_rollout_jsonl() {
    let fmt = CodexFormat;
    let path = fixtures_dir()
        .join("codex/sessions/2025/05/07")
        .join("rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl");
    let score = fmt.detect(&path);
    assert!(
        score >= 1.0,
        "expected score=1.0, got {score} for {}",
        path.display()
    );
}

#[test]
fn codex_detect_zero_for_nonexistent() {
    let fmt = CodexFormat;
    let path = fixtures_dir().join("codex/sessions/2025/05/07/rollout-NOEXIST.jsonl");
    assert_eq!(fmt.detect(&path), 0.0);
}

#[test]
fn codex_detect_zero_for_non_jsonl() {
    let fmt = CodexFormat;
    let path = fixtures_dir().join("cline/tasks/abc123"); // dir, no .jsonl
    assert_eq!(fmt.detect(&path), 0.0);
}

#[test]
fn codex_discover_finds_rollout_files() {
    let fmt = CodexFormat;
    let root = fixtures_dir().join("codex/sessions");
    let refs = fmt.discover(&root).expect("discover should succeed");
    assert!(
        !refs.is_empty(),
        "expected at least one session ref in codex fixtures"
    );
}

#[test]
fn codex_discover_extracts_parent_thread_id() {
    let fmt = CodexFormat;
    let root = fixtures_dir().join("codex/sessions");
    let refs = fmt.discover(&root).expect("discover should succeed");

    // The subagent fixture has parent_thread_id set.
    let subagent = refs
        .iter()
        .find(|r| r.path.to_string_lossy().contains("subagent"))
        .expect("expected a subagent fixture");
    assert_eq!(
        subagent.parent_session_id.as_deref(),
        Some("5973b6c0-94b8-487b-a530-2aeb6098ae0e"),
        "parent_session_id should be set from parent_thread_id in session_meta"
    );
}

#[test]
fn codex_load_parses_session_meta() {
    let fmt = CodexFormat;
    let path = fixtures_dir()
        .join("codex/sessions/2025/05/07")
        .join("rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl");
    let session = fmt
        .load(&make_file_ref("codex", path))
        .expect("load should succeed");

    assert_eq!(
        session.metadata.session_id.as_deref(),
        Some("5973b6c0-94b8-487b-a530-2aeb6098ae0e"),
        "session_id from session_meta"
    );
    assert_eq!(
        session.metadata.provider.as_deref(),
        Some("openai"),
        "provider set from model_provider"
    );
    assert!(session.parent_id.is_none(), "main session has no parent_id");
}

#[test]
fn codex_load_parses_turns_correctly() {
    let fmt = CodexFormat;
    let path = fixtures_dir()
        .join("codex/sessions/2025/05/07")
        .join("rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl");
    let session = fmt
        .load(&make_file_ref("codex", path))
        .expect("load should succeed");

    // Expected turn structure:
    // Turn 1: user("Hello…") + thinking("The user wants…") + assistant("Sure!…")
    // Turn 2: user("I need to list…") + assistant(function_call shell) + user(tool_result) + assistant("Here's a…")
    assert_eq!(session.turns.len(), 2, "expected 2 turns");

    let turn1 = &session.turns[0];
    // First message is user
    assert_eq!(turn1.messages[0].role, Role::User);
    assert!(matches!(
        &turn1.messages[0].content[0],
        ContentBlock::Text { text } if text.contains("Hello")
    ));
    // Second message is thinking (reasoning)
    assert!(matches!(
        &turn1.messages[1].content[0],
        ContentBlock::Thinking { text } if text.contains("Python script")
    ));
    // Third message is assistant text
    assert_eq!(turn1.messages[2].role, Role::Assistant);
    assert!(matches!(
        &turn1.messages[2].content[0],
        ContentBlock::Text { text } if text.contains("Sure!")
    ));

    let turn2 = &session.turns[1];
    // user → function_call (tool use) → tool result → assistant
    assert_eq!(turn2.messages[0].role, Role::User);
    assert!(
        matches!(&turn2.messages[1].content[0], ContentBlock::ToolUse { name, .. } if name == "shell")
    );
    assert!(
        matches!(&turn2.messages[2].content[0], ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "call_abc123")
    );
    assert!(
        matches!(&turn2.messages[3].content[0], ContentBlock::Text { text } if text.contains("pathlib"))
    );
}

#[test]
fn codex_load_parses_function_call_arguments_as_json() {
    let fmt = CodexFormat;
    let path = fixtures_dir()
        .join("codex/sessions/2025/05/07")
        .join("rollout-2025-05-07T17-24-21-5973b6c0-94b8-487b-a530-2aeb6098ae0e.jsonl");
    let session = fmt
        .load(&make_file_ref("codex", path))
        .expect("load should succeed");

    // function_call: arguments is a JSON string on wire → parsed to Value
    match &session.turns[1].messages[1].content[0] {
        ContentBlock::ToolUse { name, input, .. } => {
            assert_eq!(name, "shell");
            assert!(
                input.get("cmd").is_some(),
                "arguments should be parsed JSON object"
            );
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[test]
fn codex_load_subagent_has_parent_id() {
    let fmt = CodexFormat;
    let path = fixtures_dir()
        .join("codex/sessions/2025/05/07")
        .join("rollout-2025-05-07T17-30-00-subagent-0000-0000-0000-000000000001.jsonl");
    let session = fmt
        .load(&make_file_ref("codex", path))
        .expect("load should succeed");
    assert_eq!(
        session.parent_id.as_deref(),
        Some("5973b6c0-94b8-487b-a530-2aeb6098ae0e"),
        "subagent session should carry parent_id from parent_thread_id"
    );
}

// ── GeminiCliFormat ───────────────────────────────────────────────────────────

#[test]
fn gemini_detect_high_on_session_jsonl() {
    let fmt = GeminiCliFormat;
    let path = fixtures_dir()
        .join("gemini/tmp/abc123hash/chats")
        .join("session-2025-05-07T17-24-abcdef12.jsonl");
    let score = fmt.detect(&path);
    assert!(
        score >= 1.0,
        "expected score=1.0, got {score} for {}",
        path.display()
    );
}

#[test]
fn gemini_detect_zero_for_nonexistent() {
    let fmt = GeminiCliFormat;
    let path = fixtures_dir().join("gemini/tmp/abc123hash/chats/session-NOEXIST.jsonl");
    assert_eq!(fmt.detect(&path), 0.0);
}

#[test]
fn gemini_discover_finds_main_and_subagent_sessions() {
    let fmt = GeminiCliFormat;
    let root = fixtures_dir().join("gemini/tmp");
    let refs = fmt.discover(&root).expect("discover should succeed");

    let main_session = refs
        .iter()
        .find(|r| r.parent_session_id.is_none())
        .expect("expected at least one main session");
    assert_eq!(main_session.subagent_type.as_deref(), Some("interactive"));

    let sub_session = refs
        .iter()
        .find(|r| r.parent_session_id.is_some())
        .expect("expected at least one subagent session");
    assert_eq!(
        sub_session.parent_session_id.as_deref(),
        Some("session-parent-id-main123x"),
        "subagent parent_session_id should be the parent directory name"
    );
    assert_eq!(sub_session.subagent_type.as_deref(), Some("subagent"));
}

#[test]
fn gemini_load_parses_session_metadata() {
    let fmt = GeminiCliFormat;
    let path = fixtures_dir()
        .join("gemini/tmp/abc123hash/chats")
        .join("session-2025-05-07T17-24-abcdef12.jsonl");
    let session = fmt
        .load(&make_file_ref("gemini", path))
        .expect("load should succeed");

    assert_eq!(
        session.metadata.session_id.as_deref(),
        Some("session-abcdef12-0000-0000-0000-000000000001")
    );
    assert_eq!(session.metadata.provider.as_deref(), Some("google"));
    assert_eq!(
        session.metadata.timestamp.as_deref(),
        Some("2025-05-07T17:24:00.000Z")
    );
    assert_eq!(session.metadata.model.as_deref(), Some("gemini-2.0-flash"));
}

#[test]
fn gemini_load_parses_turns_with_thoughts_and_tool_calls() {
    let fmt = GeminiCliFormat;
    let path = fixtures_dir()
        .join("gemini/tmp/abc123hash/chats")
        .join("session-2025-05-07T17-24-abcdef12.jsonl");
    let session = fmt
        .load(&make_file_ref("gemini", path))
        .expect("load should succeed");

    // Turn 1: user("What is 2+2?") + gemini("2+2 equals 4.")
    // Turn 2: user("Can you read a file for me?") + gemini(thinking + text + tool_use + tool_result)
    assert_eq!(session.turns.len(), 2, "expected 2 turns");

    let t1 = &session.turns[0];
    assert_eq!(t1.messages[0].role, Role::User);
    assert!(
        matches!(&t1.messages[0].content[0], ContentBlock::Text { text } if text.contains("2+2"))
    );
    assert_eq!(t1.messages[1].role, Role::Assistant);
    assert!(
        matches!(&t1.messages[1].content[0], ContentBlock::Text { text } if text.contains("equals 4"))
    );
    // Token usage on turn 1
    let tu = t1
        .token_usage
        .as_ref()
        .expect("turn 1 should have token usage");
    assert_eq!(tu.input, 10);
    assert_eq!(tu.output, 8);

    let t2 = &session.turns[1];
    assert_eq!(t2.messages[0].role, Role::User);
    assert_eq!(t2.messages[1].role, Role::Assistant);
    let asst_content = &t2.messages[1].content;
    // Thought → Thinking, then text, then ToolUse, then ToolResult
    assert!(
        asst_content
            .iter()
            .any(|b| matches!(b, ContentBlock::Thinking { text } if text.contains("read a file"))),
        "expected a Thinking block for the thought"
    );
    assert!(
        asst_content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { name, .. } if name == "read_file")),
        "expected a ToolUse block for read_file"
    );
    assert!(
        asst_content.iter().any(
            |b| matches!(b, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "tc-001")
        ),
        "expected a ToolResult block"
    );
}

#[test]
fn gemini_load_subagent_file_parses_correctly() {
    let fmt = GeminiCliFormat;
    let path = fixtures_dir()
        .join("gemini/tmp/abc123hash/chats/session-parent-id-main123x")
        .join("sub-session-00000001.jsonl");
    let session = fmt
        .load(&make_file_ref("gemini", path))
        .expect("load should succeed");
    assert_eq!(
        session.metadata.session_id.as_deref(),
        Some("sub-session-00000001")
    );
    assert_eq!(
        session.subagent_type.as_deref(),
        Some("subagent"),
        "kind from metadata line should set subagent_type"
    );
    assert_eq!(session.turns.len(), 1);
}
