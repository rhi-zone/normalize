//! Integration tests for Cline and Roo-Code session format parsers.

use normalize_chat_sessions::{
    ClineFormat, ContentBlock, FormatRegistry, ParseError, Role, RooCodeFormat, SessionLocation,
    SessionRef, SessionSource,
};
use std::path::Path;
use std::time::SystemTime;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn make_ref(format: &'static str, dir: std::path::PathBuf) -> SessionRef {
    SessionRef {
        format,
        location: SessionLocation::Directory(dir.clone()),
        path: dir,
        mtime: SystemTime::UNIX_EPOCH,
        parent_session_id: None,
        agent_id: None,
        subagent_type: Some("interactive".into()),
    }
}

// ── ClineFormat ───────────────────────────────────────────────────────────────

#[test]
fn cline_detect_high_confidence_with_path_hint() {
    let fmt = ClineFormat;
    let task_dir = fixtures_dir().join("cline").join("tasks").join("abc123");
    // Without the saoudrizwan.claude-dev hint, should still match (dir has history file)
    let score = fmt.detect(&task_dir);
    assert!(
        score > 0.0,
        "expected score > 0, got {score} for {}",
        task_dir.display()
    );
}

#[test]
fn cline_detect_zero_for_nonexistent_dir() {
    let fmt = ClineFormat;
    let nonexistent = fixtures_dir().join("cline").join("tasks").join("NOEXIST");
    assert_eq!(fmt.detect(&nonexistent), 0.0);
}

#[test]
fn cline_discover_finds_task_dirs() {
    let fmt = ClineFormat;
    let root = fixtures_dir().join("cline");
    let refs = fmt.discover(&root).expect("discover should succeed");
    assert!(!refs.is_empty(), "expected at least one session ref");
    let found = refs.iter().any(|r| {
        r.path
            .file_name()
            .is_some_and(|n| n.to_str() == Some("abc123"))
    });
    assert!(
        found,
        "expected to find abc123 task in {refs:?}",
        refs = refs
            .iter()
            .map(|r| r.path.display().to_string())
            .collect::<Vec<_>>()
    );
    // All refs should be Directory variants
    for r in &refs {
        assert!(
            matches!(r.location, SessionLocation::Directory(_)),
            "expected Directory location for {}",
            r.path.display()
        );
    }
}

#[test]
fn cline_load_basic_structure() {
    let fmt = ClineFormat;
    let task_dir = fixtures_dir().join("cline").join("tasks").join("abc123");
    let r = make_ref("cline", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    assert_eq!(session.format, "cline");
    assert_eq!(session.metadata.session_id.as_deref(), Some("abc123"));
    assert_eq!(session.metadata.provider.as_deref(), Some("anthropic"));
    assert!(!session.turns.is_empty(), "expected at least one turn");
}

#[test]
fn cline_load_correct_turn_count() {
    let fmt = ClineFormat;
    let task_dir = fixtures_dir().join("cline").join("tasks").join("abc123");
    let r = make_ref("cline", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    // Fixture has 2 user prompts → 2 turns
    assert_eq!(
        session.turns.len(),
        2,
        "expected 2 turns (one per user prompt), got {}",
        session.turns.len()
    );
}

#[test]
fn cline_load_tool_result_as_tool_role() {
    let fmt = ClineFormat;
    let task_dir = fixtures_dir().join("cline").join("tasks").join("abc123");
    let r = make_ref("cline", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    let all_messages: Vec<_> = session.turns.iter().flat_map(|t| &t.messages).collect();

    // There should be Tool-role messages (from tool_result blocks)
    let tool_msgs: Vec<_> = all_messages
        .iter()
        .filter(|m| m.role == Role::Tool)
        .collect();
    assert!(
        !tool_msgs.is_empty(),
        "expected at least one Tool-role message"
    );

    // There should be ToolUse blocks in assistant messages
    let has_tool_use = all_messages.iter().any(|m| {
        m.role == Role::Assistant
            && m.content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    });
    assert!(has_tool_use, "expected at least one ToolUse block");
}

// ── RooCodeFormat ─────────────────────────────────────────────────────────────

#[test]
fn roo_detect_high_confidence_for_task_dir() {
    let fmt = RooCodeFormat;
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let score = fmt.detect(&task_dir);
    assert!(score > 0.0, "expected score > 0, got {score}");
}

#[test]
fn roo_discover_finds_task_dirs() {
    let fmt = RooCodeFormat;
    let root = fixtures_dir().join("roo");
    let refs = fmt.discover(&root).expect("discover should succeed");
    assert!(!refs.is_empty(), "expected at least one session ref");
    let found = refs.iter().any(|r| {
        r.path
            .file_name()
            .is_some_and(|n| n.to_str() == Some("def456"))
    });
    assert!(found, "expected to find def456 task");
    for r in &refs {
        assert!(
            matches!(r.location, SessionLocation::Directory(_)),
            "expected Directory location"
        );
    }
}

#[test]
fn roo_load_basic_structure() {
    let fmt = RooCodeFormat;
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let r = make_ref("roo-code", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    assert_eq!(session.format, "roo-code");
    assert_eq!(session.metadata.session_id.as_deref(), Some("def456"));
    assert_eq!(session.metadata.provider.as_deref(), Some("anthropic"));
    assert!(!session.turns.is_empty(), "expected at least one turn");
}

#[test]
fn roo_load_skips_summary_messages() {
    let fmt = RooCodeFormat;
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let r = make_ref("roo-code", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    // Fixture has 2 non-summary user prompts → 2 turns
    // (the isSummary message should be skipped)
    assert_eq!(
        session.turns.len(),
        2,
        "expected 2 turns (isSummary messages skipped), got {}",
        session.turns.len()
    );
}

#[test]
fn roo_load_reasoning_content_becomes_thinking_block() {
    let fmt = RooCodeFormat;
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let r = make_ref("roo-code", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    let has_thinking = session.turns.iter().flat_map(|t| &t.messages).any(|m| {
        m.role == Role::Assistant
            && m.content
                .iter()
                .any(|b| matches!(b, ContentBlock::Thinking { .. }))
    });
    assert!(
        has_thinking,
        "expected reasoning_content to produce a Thinking block"
    );
}

#[test]
fn roo_load_timestamps_from_ts_field() {
    let fmt = RooCodeFormat;
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let r = make_ref("roo-code", task_dir);
    let session = fmt.load(&r).expect("load should succeed");

    // First user message should have a timestamp derived from ts=1705319400000
    let first_msg = session.turns.first().and_then(|t| t.messages.first());
    let ts = first_msg.and_then(|m| m.timestamp.as_deref());
    // ts=1705319400000ms → 1705319400s → 2024-01-15T11:50:00Z
    assert_eq!(
        ts,
        Some("2024-01-15T11:50:00Z"),
        "unexpected timestamp: {ts:?}"
    );
}

// ── FormatRegistry integration ────────────────────────────────────────────────

#[test]
fn registry_contains_cline_and_roo() {
    let registry = FormatRegistry::new();
    let names = registry.list();
    assert!(
        names.contains(&"cline"),
        "expected 'cline' in registry; got: {names:?}"
    );
    assert!(
        names.contains(&"roo-code"),
        "expected 'roo-code' in registry; got: {names:?}"
    );
}

#[test]
fn registry_load_cline_by_format_name() {
    let registry = FormatRegistry::new();
    let task_dir = fixtures_dir().join("cline").join("tasks").join("abc123");
    let r = make_ref("cline", task_dir);
    let session = registry.load(&r).expect("registry load should succeed");
    assert_eq!(session.format, "cline");
}

#[test]
fn registry_load_roo_by_format_name() {
    let registry = FormatRegistry::new();
    let task_dir = fixtures_dir().join("roo").join("tasks").join("def456");
    let r = make_ref("roo-code", task_dir);
    let session = registry.load(&r).expect("registry load should succeed");
    assert_eq!(session.format, "roo-code");
}

// ── Edge cases ────────────────────────────────────────────────────────────────

fn make_task_dir_with_history(subdir: &str, content: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("normalize-chat-sessions-test-{}", subdir));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("api_conversation_history.json"), content).unwrap();
    base
}

#[test]
fn load_empty_history_returns_session_with_no_turns() {
    let dir = make_task_dir_with_history("empty-history", "[]");
    let fmt = ClineFormat;
    let r = make_ref("cline", dir.clone());
    let session = fmt.load(&r).expect("load of empty history should succeed");
    assert!(session.turns.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_invalid_json_returns_format_error() {
    let dir = make_task_dir_with_history("invalid-json", "not json");
    let fmt = ClineFormat;
    let r = make_ref("cline", dir.clone());
    let result = fmt.load(&r);
    assert!(matches!(result, Err(ParseError::Format { .. })));
    std::fs::remove_dir_all(&dir).ok();
}
