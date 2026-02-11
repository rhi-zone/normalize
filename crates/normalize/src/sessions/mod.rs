//! Session parsing and analysis.
//!
//! Parsing is provided by the normalize-sessions crate.
//! Analysis (computing metrics from parsed sessions) lives here in the CLI.

mod analysis;

// Re-export parsing types from normalize-sessions
pub use normalize_chat_sessions::{
    ContentBlock, FormatRegistry, LogFormat, Message, Role, Session, SessionFile, SessionMetadata,
    TokenUsage, Turn, detect_format, get_format, list_formats, parse_session,
    parse_session_with_format,
};

// Export analysis types from this crate
pub use analysis::{
    CommandDetail, CommandStats, DedupTokenStats, ErrorPattern, RetryHotspot, SessionAnalysis,
    TokenStats, ToolStats, analyze_session, categorize_command, categorize_error,
    extract_tool_patterns, normalize_path,
};
