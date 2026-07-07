//! OpenCode session format parser (libsql backend).
//!
//! OpenCode stores sessions in a SQLite database at
//! `$XDG_DATA_HOME/opencode/opencode.db`.
//!
//! Default storage root: `$XDG_DATA_HOME/opencode/opencode.db`
//! Override via: `OPENCODE_DB` environment variable.
//!
//! # Schema (relevant tables)
//!
//! `session(id, project_id, parent_id, slug, directory, title, model, time_created, ...)`
//! `session_message(id, session_id, type, seq, time_created, data)` — `data` is JSON
//! with all message fields except `type` and `id` (those are separate columns).
//!
//! # Async bridging
//!
//! libsql is async; `SessionSource::load` is sync.  The bridge pattern mirrors
//! `normalize-facts/src/ca_cache.rs`: a `block_on` dispatcher that picks the right
//! strategy based on the calling thread's tokio context.

use super::{DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource};
use crate::session::{ContentBlock, Message, Role, Session, SessionMetadata, TokenUsage, Turn};
use libsql::Builder;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Async bridge ──────────────────────────────────────────────────────────────

/// Drive `fut` to completion, choosing a strategy based on the calling thread's
/// tokio context.  Mirrors `block_on_helper` in `normalize-facts/src/ca_cache.rs`.
fn block_on<F>(fut: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    use tokio::runtime::Handle;
    if let Ok(handle) = Handle::try_current() {
        return match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(fut))
            }
            // Current-thread runtime: block_in_place would panic, so use a scoped thread.
            _ => spawn_scoped(fut),
        };
    }
    // No active runtime — build a disposable current-thread one.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime for opencode")
        .block_on(fut)
}

/// Drive `fut` on a freshly-built current-thread runtime on a scoped OS thread.
fn spawn_scoped<F>(fut: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    std::thread::scope(|s| {
        s.spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime worker for opencode")
                .block_on(fut)
        })
        .join()
        .expect("opencode libsql worker thread panicked")
    })
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn xdg_data_home() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local").join("share")
}

fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("OPENCODE_DB") {
        return PathBuf::from(p);
    }
    xdg_data_home().join("opencode").join("opencode.db")
}

// ── DB detection helpers ──────────────────────────────────────────────────────

/// Returns `true` if `path` starts with the SQLite file magic (`SQLite format 3\0`).
fn has_sqlite_magic(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 16];
    if f.read_exact(&mut buf).is_err() {
        return false;
    }
    &buf[..15] == b"SQLite format 3"
}

/// Returns `true` if `path` is a SQLite file with both `session` and
/// `session_message` tables — the fingerprint for an OpenCode database.
fn has_opencode_tables(path: &Path) -> bool {
    block_on(async {
        let db = Builder::new_local(path).build().await.ok()?;
        let conn = db.connect().ok()?;
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name IN ('session', 'session_message')",
                libsql::params![],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        let count: i64 = row.get(0).ok()?;
        Some(count == 2)
    })
    .unwrap_or(false)
}

// ── Timestamp helper ──────────────────────────────────────────────────────────

fn millis_to_system_time(ms: i64) -> SystemTime {
    if ms <= 0 {
        return UNIX_EPOCH;
    }
    UNIX_EPOCH + Duration::from_millis(ms as u64)
}

// ── OpenCodeFormat ────────────────────────────────────────────────────────────

/// OpenCode session source — reads from `opencode.db` via libsql.
pub struct OpenCodeFormat;

impl SessionSource for OpenCodeFormat {
    fn name(&self) -> &'static str {
        "opencode"
    }

    /// Returns the directory containing the database (`$XDG_DATA_HOME/opencode`).
    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        default_db_path()
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }

    /// `[db_path]` — the single database file is the root.
    fn default_roots(&self) -> Vec<PathBuf> {
        vec![default_db_path()]
    }

    /// `1.0` when `path` has the SQLite magic bytes and both opencode tables;
    /// `0.0` otherwise.
    fn detect(&self, path: &Path) -> f64 {
        if !path.is_file() {
            return 0.0;
        }
        if !has_sqlite_magic(path) {
            return 0.0;
        }
        if has_opencode_tables(path) { 1.0 } else { 0.0 }
    }

    /// Enumerate all sessions in the database at `root` (or `root/opencode.db`).
    ///
    /// Returns one `SessionRef` per row in the `session` table.  Messages are
    /// NOT loaded here; call `load` to fully parse a session.
    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError> {
        let db_path = if root.is_file() {
            root.to_path_buf()
        } else {
            root.join("opencode.db")
        };
        if !db_path.exists() {
            return Ok(vec![]);
        }
        block_on(async move {
            let db = Builder::new_local(&db_path)
                .build()
                .await
                .map_err(|e| DiscoverError::Other(e.to_string()))?;
            let conn = db
                .connect()
                .map_err(|e| DiscoverError::Other(e.to_string()))?;
            let mut rows = conn
                .query(
                    "SELECT id, parent_id, time_created \
                     FROM session ORDER BY time_created DESC",
                    libsql::params![],
                )
                .await
                .map_err(|e| DiscoverError::Other(e.to_string()))?;

            let mut refs: Vec<SessionRef> = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                let session_id: String = row.get(0).unwrap_or_default();
                let parent_id: Option<String> = row.get::<String>(1).ok().filter(|s| !s.is_empty());
                let time_ms: Option<i64> = row.get(2).ok();
                let mtime = time_ms.map(millis_to_system_time).unwrap_or(UNIX_EPOCH);
                refs.push(SessionRef {
                    format: "opencode",
                    location: SessionLocation::Database {
                        db_path: db_path.clone(),
                        session_id: session_id.clone(),
                    },
                    path: db_path.clone(),
                    mtime,
                    parent_session_id: parent_id,
                    agent_id: None,
                    subagent_type: None,
                });
            }
            Ok(refs)
        })
    }

    /// Fully load a session from a `SessionLocation::Database` reference.
    ///
    /// Queries `session_message WHERE session_id = ?` ordered by `seq`, then
    /// reconstructs `Session → Turn → Message → ContentBlock`.
    fn load(&self, r: &SessionRef) -> Result<Session, ParseError> {
        let (db_path, session_id) = match &r.location {
            SessionLocation::Database {
                db_path,
                session_id,
            } => (db_path.clone(), session_id.clone()),
            _ => {
                return Err(ParseError::Other(format!(
                    "opencode: expected Database location, got file: {}",
                    r.path.display()
                )));
            }
        };

        block_on(async move {
            let db = Builder::new_local(&db_path)
                .build()
                .await
                .map_err(|e| ParseError::Database(e.to_string()))?;
            let conn = db
                .connect()
                .map_err(|e| ParseError::Database(e.to_string()))?;

            // ── Session metadata ──────────────────────────────────────────────
            let mut meta_rows = conn
                .query(
                    "SELECT parent_id, model, time_created, directory \
                     FROM session WHERE id = ?",
                    libsql::params![session_id.clone()],
                )
                .await
                .map_err(|e| ParseError::Database(e.to_string()))?;

            let (parent_id, provider, model_id, timestamp, project) =
                if let Ok(Some(row)) = meta_rows.next().await {
                    let pid: Option<String> = row.get::<String>(0).ok().filter(|s| !s.is_empty());
                    let model_json: Option<String> = row.get(1).ok();
                    let time_ms: Option<i64> = row.get(2).ok();
                    let dir: Option<String> = row.get(3).ok();
                    let (prov, mid) = if let Some(json) = model_json {
                        let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                        (
                            v.get("providerID")
                                .and_then(|x| x.as_str())
                                .map(String::from),
                            v.get("id").and_then(|x| x.as_str()).map(String::from),
                        )
                    } else {
                        (None, None)
                    };
                    (pid, prov, mid, time_ms.map(|ms| ms.to_string()), dir)
                } else {
                    (None, None, None, None, None)
                };

            let mut session = Session::new(db_path.clone(), "opencode");
            session.metadata = SessionMetadata {
                session_id: Some(session_id.clone()),
                timestamp,
                provider,
                model: model_id,
                project,
            };
            session.parent_id = parent_id;

            // ── Messages ──────────────────────────────────────────────────────
            let mut msg_rows = conn
                .query(
                    "SELECT id, type, seq, time_created, data \
                     FROM session_message WHERE session_id = ? ORDER BY seq ASC",
                    libsql::params![session_id.clone()],
                )
                .await
                .map_err(|e| ParseError::Database(e.to_string()))?;

            // One turn per user message.  Each user message starts a new turn;
            // assistant messages following it belong to that same turn.
            let mut current_turn: Option<Turn> = None;
            let mut current_turn_tokens: Option<TokenUsage> = None;

            while let Ok(Some(row)) = msg_rows.next().await {
                let msg_id: String = row.get(0).unwrap_or_default();
                let msg_type: String = row.get(1).unwrap_or_default();
                let time_ms: Option<i64> = row.get(3).ok();
                let data_str: String = row.get(4).unwrap_or_default();
                let ts = time_ms.map(|ms| ms.to_string());

                let data: serde_json::Value =
                    serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);

                match msg_type.as_str() {
                    "user" => {
                        // Flush previous turn before starting a new one.
                        if let Some(mut t) = current_turn.take() {
                            t.token_usage = current_turn_tokens.take();
                            session.turns.push(t);
                        }
                        let text = data
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mut turn = Turn::default();
                        turn.messages.push(Message {
                            role: Role::User,
                            content: vec![ContentBlock::Text { text }],
                            timestamp: ts,
                        });
                        current_turn = Some(turn);
                    }
                    "assistant" => {
                        let turn = current_turn.get_or_insert_with(Turn::default);
                        let content_arr = data
                            .get("content")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();

                        let mut blocks: Vec<ContentBlock> = Vec::new();
                        for item in content_arr {
                            let block_type =
                                item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            match block_type {
                                "text" => {
                                    let text = item
                                        .get("text")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    if !text.is_empty() {
                                        blocks.push(ContentBlock::Text { text });
                                    }
                                }
                                "reasoning" => {
                                    let text = item
                                        .get("text")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    if !text.is_empty() {
                                        blocks.push(ContentBlock::Thinking { text });
                                    }
                                }
                                "tool" => {
                                    blocks.extend(tool_blocks_from_item(&item, &msg_id));
                                }
                                _ => {}
                            }
                        }

                        accumulate_tokens(&data, &mut current_turn_tokens);

                        turn.messages.push(Message {
                            role: Role::Assistant,
                            content: blocks,
                            timestamp: ts,
                        });
                    }
                    "system" => {
                        let text = data
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let turn = current_turn.get_or_insert_with(Turn::default);
                        turn.messages.push(Message {
                            role: Role::System,
                            content: vec![ContentBlock::Text { text }],
                            timestamp: ts,
                        });
                    }
                    // agent-switched, model-switched, synthetic, shell, compaction → skip.
                    _ => {}
                }
            }

            // Flush last turn.
            if let Some(mut t) = current_turn.take() {
                t.token_usage = current_turn_tokens.take();
                session.turns.push(t);
            }

            Ok(session)
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `ToolUse` + optional `ToolResult` content blocks from an assistant
/// tool item (`{type: "tool", id, name, state: {status, input, content, ...}}`).
fn tool_blocks_from_item(item: &serde_json::Value, fallback_id: &str) -> Vec<ContentBlock> {
    let tool_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_id)
        .to_string();
    let tool_name = item
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let state = item
        .get("state")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let status = state
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("pending");

    // `input` is a JSON-encoded string in the `pending` state; a plain object otherwise.
    let input: serde_json::Value = if status == "pending" {
        state
            .get("input")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Null)
    } else {
        state
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };

    let mut blocks = vec![ContentBlock::ToolUse {
        id: tool_id.clone(),
        name: tool_name,
        input,
    }];

    // Add result only for terminal states.
    if matches!(status, "completed" | "error") {
        let content_text: String = state
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        blocks.push(ContentBlock::ToolResult {
            tool_use_id: tool_id,
            content: content_text,
            is_error: status == "error",
        });
    }

    blocks
}

/// Accumulate token usage from an assistant message's `tokens` field into `acc`.
fn accumulate_tokens(data: &serde_json::Value, acc: &mut Option<TokenUsage>) {
    let Some(tokens) = data.get("tokens") else {
        return;
    };
    let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
    let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
    let cache_read = tokens
        .get("cache")
        .and_then(|c| c.get("read"))
        .and_then(|v| v.as_u64());
    let cache_write = tokens
        .get("cache")
        .and_then(|c| c.get("write"))
        .and_then(|v| v.as_u64());

    let usage = acc.get_or_insert_with(TokenUsage::default);
    usage.input += input;
    usage.output += output;
    if let Some(cr) = cache_read {
        *usage.cache_read.get_or_insert(0) += cr;
    }
    if let Some(cw) = cache_write {
        *usage.cache_create.get_or_insert(0) += cw;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    /// Build a synthetic opencode.db, verify detect/discover/load round-trip.
    #[tokio::test]
    async fn test_opencode_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("opencode.db");

        // ── Seed the database ──────────────────────────────────────────────
        {
            let db = Builder::new_local(&db_path)
                .build()
                .await
                .expect("build db");
            let conn = db.connect().expect("connect");

            conn.execute_batch(
                "CREATE TABLE session (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL DEFAULT '',
                    workspace_id TEXT,
                    parent_id TEXT,
                    slug TEXT NOT NULL DEFAULT '',
                    directory TEXT NOT NULL DEFAULT '',
                    path TEXT,
                    title TEXT NOT NULL DEFAULT '',
                    version TEXT NOT NULL DEFAULT '',
                    share_url TEXT,
                    summary_additions INTEGER,
                    summary_deletions INTEGER,
                    summary_files INTEGER,
                    summary_diffs TEXT,
                    metadata TEXT,
                    cost REAL NOT NULL DEFAULT 0,
                    tokens_input INTEGER NOT NULL DEFAULT 0,
                    tokens_output INTEGER NOT NULL DEFAULT 0,
                    tokens_reasoning INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_read INTEGER NOT NULL DEFAULT 0,
                    tokens_cache_write INTEGER NOT NULL DEFAULT 0,
                    revert TEXT,
                    permission TEXT,
                    agent TEXT,
                    model TEXT,
                    time_created INTEGER NOT NULL DEFAULT 0,
                    time_updated INTEGER NOT NULL DEFAULT 0,
                    time_compacting INTEGER,
                    time_archived INTEGER
                );
                CREATE TABLE session_message (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES session(id),
                    type TEXT NOT NULL,
                    seq INTEGER NOT NULL,
                    time_created INTEGER NOT NULL DEFAULT 0,
                    time_updated INTEGER NOT NULL DEFAULT 0,
                    data TEXT NOT NULL
                );",
            )
            .await
            .expect("create tables");

            let model_json = r#"{"id":"claude-sonnet-4-5","providerID":"anthropic"}"#;

            // Session 1: alpha
            conn.execute(
                "INSERT INTO session \
                 (id, project_id, slug, directory, title, version, model, time_created, time_updated) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                libsql::params![
                    "ses_alpha",
                    "proj_1",
                    "alpha",
                    "/home/user/myproject",
                    "Alpha session",
                    "1.0",
                    model_json,
                    1_700_000_000_000i64,
                    1_700_000_001_000i64,
                ],
            )
            .await
            .expect("insert session alpha");

            // Session 2: beta (more recent, should come first in discover)
            conn.execute(
                "INSERT INTO session \
                 (id, project_id, slug, directory, title, version, model, time_created, time_updated) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                libsql::params![
                    "ses_beta",
                    "proj_1",
                    "beta",
                    "/home/user/myproject",
                    "Beta session",
                    "1.0",
                    model_json,
                    1_700_000_002_000i64,
                    1_700_000_003_000i64,
                ],
            )
            .await
            .expect("insert session beta");

            // Messages for ses_alpha: user → assistant (with tool call) → user → assistant
            let user_data = r#"{"text":"Hello, world!","files":[],"agents":[],"time":{"created":1700000000000}}"#;
            let tool_state = serde_json::json!({
                "status": "completed",
                "input": {"path": "/tmp/foo.txt"},
                "structured": {},
                "content": [{"type": "text", "text": "file contents here"}]
            });
            let asst_data = serde_json::json!({
                "agent": "claude",
                "model": {"id": "claude-sonnet-4-5", "providerID": "anthropic"},
                "content": [
                    {"type": "text", "id": "txt_1", "text": "I will read the file."},
                    {"type": "tool", "id": "tool_1", "name": "read_file", "state": tool_state}
                ],
                "tokens": {
                    "input": 100, "output": 50, "reasoning": 0,
                    "cache": {"read": 10, "write": 5}
                },
                "time": {"created": 1_700_000_000_500_i64}
            });
            let user2_data =
                r#"{"text":"Follow up.","files":[],"agents":[],"time":{"created":1700000001000}}"#;
            let asst2_data = serde_json::json!({
                "agent": "claude",
                "model": {"id": "claude-sonnet-4-5", "providerID": "anthropic"},
                "content": [
                    {"type": "text", "id": "txt_2", "text": "Done."}
                ],
                "tokens": {
                    "input": 120, "output": 20, "reasoning": 0,
                    "cache": {"read": 0, "write": 0}
                },
                "time": {"created": 1_700_000_001_500_i64}
            });

            let msgs: &[(&str, &str, i64, String)] = &[
                ("msg_1", "user", 1, user_data.to_string()),
                ("msg_2", "assistant", 2, asst_data.to_string()),
                ("msg_3", "user", 3, user2_data.to_string()),
                ("msg_4", "assistant", 4, asst2_data.to_string()),
            ];
            for (id, typ, seq, data) in msgs {
                conn.execute(
                    "INSERT INTO session_message \
                     (id, session_id, type, seq, time_created, time_updated, data) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    libsql::params![
                        *id,
                        "ses_alpha",
                        *typ,
                        *seq,
                        1_700_000_000_000i64,
                        1_700_000_000_000i64,
                        data.clone(),
                    ],
                )
                .await
                .expect("insert message");
            }
        } // drop conn + db, flush WAL

        // ── detect ────────────────────────────────────────────────────────
        let fmt = OpenCodeFormat;
        let score = fmt.detect(&db_path);
        assert_eq!(score, 1.0, "detect should return 1.0 for opencode.db");

        // ── discover ──────────────────────────────────────────────────────
        let refs = fmt.discover(dir.path()).expect("discover");
        assert_eq!(refs.len(), 2, "should discover both sessions");

        let alpha_ref = refs
            .iter()
            .find(|r| {
                matches!(&r.location,
                    SessionLocation::Database { session_id, .. } if session_id == "ses_alpha")
            })
            .expect("ses_alpha ref");
        assert_eq!(alpha_ref.format, "opencode");
        assert_ne!(alpha_ref.mtime, UNIX_EPOCH, "mtime should be set");

        // ── load ──────────────────────────────────────────────────────────
        let session = fmt.load(alpha_ref).expect("load ses_alpha");
        assert_eq!(session.format, "opencode");
        assert_eq!(session.metadata.session_id.as_deref(), Some("ses_alpha"));
        assert_eq!(session.metadata.provider.as_deref(), Some("anthropic"));
        assert_eq!(session.metadata.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(
            session.metadata.project.as_deref(),
            Some("/home/user/myproject")
        );

        // Two turns (one per user message).
        assert_eq!(session.turns.len(), 2, "expected 2 turns");

        let turn0 = &session.turns[0];
        assert_eq!(turn0.messages.len(), 2, "turn0: user + assistant");
        assert_eq!(turn0.messages[0].role, Role::User);
        assert_eq!(turn0.messages[1].role, Role::Assistant);

        // Assistant blocks: Text + ToolUse + ToolResult
        let asst_blocks = &turn0.messages[1].content;
        assert!(
            asst_blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::Text { .. })),
            "expected Text block in assistant"
        );
        assert!(
            asst_blocks
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { name, .. } if name == "read_file")),
            "expected ToolUse(read_file)"
        );
        assert!(
            asst_blocks.iter().any(|b| matches!(
                b,
                ContentBlock::ToolResult { content, .. } if content.contains("file contents")
            )),
            "expected ToolResult with file contents"
        );

        // Token usage on turn 0.
        let usage = turn0.token_usage.as_ref().expect("token_usage on turn0");
        assert_eq!(usage.input, 100);
        assert_eq!(usage.output, 50);
        assert_eq!(usage.cache_read, Some(10));
        assert_eq!(usage.cache_create, Some(5));

        let turn1 = &session.turns[1];
        assert_eq!(turn1.messages.len(), 2, "turn1: user + assistant");
        assert!(
            turn1.messages[1]
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::Text { text, .. } if text == "Done.")),
            "expected 'Done.' in turn 1 assistant"
        );
    }
}
