//! Session log parsing for AI coding agents.
//!
//! Parses session logs from various AI coding agents:
//! - Claude Code (JSONL)
//! - Gemini CLI (JSON)
//! - OpenAI Codex CLI (JSONL)
//! - Normalize Agent (JSONL)
//!
//! # Architecture
//!
//! This crate separates discovery from parsing:
//!
//! - **Discovery**: `SessionSource::discover()` enumerates session references without full parsing
//! - **Parsing**: `SessionSource::load()` / `parse_session()` converts format-specific logs into `Session`
//! - **Analysis**: Consumers compute their own metrics from `Session` data
//!
//! Each format implements the `SessionSource` trait (Phase 1 redesign; replaces the former `LogFormat` trait).
//!
//! # Example
//!
//! ```ignore
//! use normalize_chat_sessions::{parse_session, Session};
//!
//! let session = parse_session(std::path::Path::new("~/.claude/projects/foo/session.jsonl"))?;
//! for turn in &session.turns {
//!     for msg in &turn.messages {
//!         println!("{}: {} blocks", msg.role, msg.content.len());
//!     }
//! }
//! ```

mod formats;
mod session;

pub use formats::*;
pub use session::*;
