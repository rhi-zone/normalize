//! Session log parsing for AI coding agents.
//!
//! Parses session logs from various AI coding agents:
//! - Claude Code (JSONL)
//! - Gemini CLI (JSON)
//! - OpenAI Codex CLI (JSONL)
//! - Moss Agent (JSONL)
//!
//! # Architecture
//!
//! This crate separates parsing from analysis:
//!
//! - **Parsing**: `LogFormat::parse()` converts format-specific logs into a unified `Session` type
//! - **Analysis**: Consumers compute their own metrics from `Session` data
//!
//! Each log format implements the `LogFormat` trait for format detection and parsing.
//!
//! # Example
//!
//! ```ignore
//! use rhi_normalize_sessions::{parse_session, Session};
//!
//! let session = parse_session("~/.claude/projects/foo/session.jsonl")?;
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
