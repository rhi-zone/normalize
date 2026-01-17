//! Session parsing and analysis.
//!
//! Parsing is provided by the moss-sessions crate.
//! Analysis (computing metrics from parsed sessions) lives here in the CLI.

mod analysis;

// Re-export parsing types from moss-sessions
pub use rhizome_moss_sessions::{
    ContentBlock, FormatRegistry, LogFormat, Message, Role, Session, SessionFile, SessionMetadata,
    TokenUsage, Turn, detect_format, get_format, list_formats, parse_session,
    parse_session_with_format,
};

// Export analysis types from this crate
pub use analysis::{
    ErrorPattern, SessionAnalysis, TokenStats, ToolStats, analyze_session, categorize_error,
    normalize_path,
};
