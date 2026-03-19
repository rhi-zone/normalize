//! Log format plugins.
//!
//! Each format implements the `LogFormat` trait for parsing session logs.
//!
//! # Extensibility
//!
//! Users can register custom formats via [`register()`]:
//!
//! ```ignore
//! use normalize_chat_sessions::{LogFormat, SessionAnalysis, SessionFile, register};
//! use std::path::{Path, PathBuf};
//!
//! struct MyAgentFormat;
//!
//! impl LogFormat for MyAgentFormat {
//!     fn name(&self) -> &'static str { "myagent" }
//!     fn sessions_dir(&self, project: Option<&Path>) -> PathBuf { /* ... */ }
//!     fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> { /* ... */ }
//!     fn detect(&self, path: &Path) -> f64 { /* ... */ }
//!     fn analyze(&self, path: &Path) -> Result<SessionAnalysis, String> { /* ... */ }
//! }
//!
//! // Register before first use
//! register(&MyAgentFormat);
//! ```

#[cfg(feature = "format-claude")]
mod claude_code;
#[cfg(feature = "format-codex")]
mod codex;
#[cfg(feature = "format-gemini")]
mod gemini_cli;
#[cfg(feature = "format-normalize")]
mod normalize_agent;

#[cfg(feature = "format-claude")]
pub use claude_code::ClaudeCodeFormat;
#[cfg(feature = "format-codex")]
pub use codex::CodexFormat;
#[cfg(feature = "format-gemini")]
pub use gemini_cli::GeminiCliFormat;
#[cfg(feature = "format-normalize")]
pub use normalize_agent::NormalizeAgentFormat;

use crate::Session;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// Global registry of log format plugins.
static FORMATS: RwLock<Vec<&'static dyn LogFormat>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom log format plugin.
///
/// Call this before any parsing operations to add custom formats.
/// Built-in formats are registered automatically on first use.
pub fn register(format: &'static dyn LogFormat) {
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    FORMATS.write().unwrap().push(format);
}

/// Initialize built-in formats (called automatically on first use).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
        let mut formats = FORMATS.write().unwrap();
        #[cfg(feature = "format-claude")]
        formats.push(&ClaudeCodeFormat);
        #[cfg(feature = "format-codex")]
        formats.push(&CodexFormat);
        #[cfg(feature = "format-gemini")]
        formats.push(&GeminiCliFormat);
        #[cfg(feature = "format-normalize")]
        formats.push(&NormalizeAgentFormat);
    });
}

/// Session file with metadata.
pub struct SessionFile {
    pub path: PathBuf,
    pub mtime: std::time::SystemTime,
    /// Parent session ID (set for subagent sessions).
    pub parent_id: Option<String>,
    /// Agent ID (set for subagent sessions, e.g. "agent-a5c5ccc9c2b61e757").
    pub agent_id: Option<String>,
    /// Subagent type from meta.json (e.g. "general-purpose", "Explore").
    pub subagent_type: Option<String>,
}

/// Trait for session log format plugins.
pub trait LogFormat: Send + Sync {
    /// Format identifier (e.g., "claude", "codex", "gemini", "normalize").
    fn name(&self) -> &'static str;

    /// Get the sessions directory for this format.
    /// Does NOT check if the directory exists - that's handled by list_sessions.
    fn sessions_dir(&self, project: Option<&Path>) -> PathBuf;

    /// List all session files for this format.
    fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile>;

    /// List subagent session files for this format.
    /// Default returns empty (only Claude Code supports subagents currently).
    fn list_subagent_sessions(&self, _project: Option<&Path>) -> Vec<SessionFile> {
        Vec::new()
    }

    /// Check if this format can parse the given file.
    /// Returns a confidence score 0.0-1.0.
    fn detect(&self, path: &Path) -> f64;

    /// Parse the log file into a unified Session structure.
    fn parse(&self, path: &Path) -> Result<Session, String>;
}

/// Get a format by name from the global registry.
pub fn get_format(name: &str) -> Option<&'static dyn LogFormat> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    FORMATS
        .read()
        .unwrap()
        .iter()
        .find(|f| f.name() == name)
        .copied()
}

/// Auto-detect format for a file using the global registry.
pub fn detect_format(path: &Path) -> Option<&'static dyn LogFormat> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    let formats = FORMATS.read().unwrap();
    let mut best: Option<(&'static dyn LogFormat, f64)> = None;
    for fmt in formats.iter() {
        let score = fmt.detect(path);
        if score > 0.0 && best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((*fmt, score));
        }
    }
    best.map(|(fmt, _)| fmt)
}

/// List all available format names from the global registry.
pub fn list_formats() -> Vec<&'static str> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    FORMATS.read().unwrap().iter().map(|f| f.name()).collect()
}

/// Default implementation: list .jsonl files in a directory.
pub fn list_jsonl_sessions(dir: &Path) -> Vec<SessionFile> {
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
                && let Ok(meta) = path.metadata()
                && let Ok(mtime) = meta.modified()
            {
                sessions.push(SessionFile {
                    path,
                    mtime,
                    parent_id: None,
                    agent_id: None,
                    subagent_type: Some("interactive".into()),
                });
            }
        }
    }
    sessions
}

/// List subagent sessions from `<session-uuid>/subagents/` directories.
///
/// Walks each subdirectory of `dir` looking for a `subagents/` folder containing
/// `agent-<id>.jsonl` files. Also reads the companion `.meta.json` for agent type.
pub fn list_subagent_sessions(dir: &Path) -> Vec<SessionFile> {
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sessions;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let parent_id = path.file_name().and_then(|n| n.to_str()).map(String::from);
        let subagents_dir = path.join("subagents");
        if !subagents_dir.is_dir() {
            continue;
        }
        let Ok(sub_entries) = std::fs::read_dir(&subagents_dir) else {
            continue;
        };
        for sub_entry in sub_entries.filter_map(|e| e.ok()) {
            let sub_path = sub_entry.path();
            if sub_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = match sub_path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            if !stem.starts_with("agent-") {
                continue;
            }
            let Ok(meta) = sub_path.metadata() else {
                continue;
            };
            let Ok(mtime) = meta.modified() else {
                continue;
            };
            // Read companion .meta.json for agent type, default to "subagent"
            let meta_path = sub_path.with_extension("meta.json");
            let subagent_type = Some(
                std::fs::read_to_string(&meta_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| {
                        v.get("agentType")
                            .and_then(|t| t.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| "subagent".into()),
            );
            sessions.push(SessionFile {
                path: sub_path,
                mtime,
                parent_id: parent_id.clone(),
                agent_id: Some(stem.clone()),
                subagent_type,
            });
        }
    }
    sessions
}

/// Registry of available log formats.
///
/// For most use cases, prefer the global registry via [`register()`],
/// [`get_format()`], [`detect_format()`], and [`list_formats()`].
///
/// Use `FormatRegistry` when you need an isolated registry (e.g., testing).
pub struct FormatRegistry {
    formats: Vec<Box<dyn LogFormat>>,
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatRegistry {
    /// Create a new registry with all built-in formats.
    #[allow(clippy::vec_init_then_push)] // cfg-gated pushes can't use vec![]
    pub fn new() -> Self {
        let mut formats: Vec<Box<dyn LogFormat>> = Vec::new();
        #[cfg(feature = "format-claude")]
        formats.push(Box::new(ClaudeCodeFormat));
        #[cfg(feature = "format-codex")]
        formats.push(Box::new(CodexFormat));
        #[cfg(feature = "format-gemini")]
        formats.push(Box::new(GeminiCliFormat));
        #[cfg(feature = "format-normalize")]
        formats.push(Box::new(NormalizeAgentFormat));
        Self { formats }
    }

    /// Create an empty registry (no built-in formats).
    pub fn empty() -> Self {
        Self { formats: vec![] }
    }

    /// Register a custom format.
    pub fn register(&mut self, format: Box<dyn LogFormat>) {
        self.formats.push(format);
    }

    /// Detect the best format for a file.
    pub fn detect(&self, path: &Path) -> Option<&dyn LogFormat> {
        let mut best: Option<(&dyn LogFormat, f64)> = None;
        for fmt in &self.formats {
            let score = fmt.detect(path);
            if score > 0.0 && best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((fmt.as_ref(), score));
            }
        }
        best.map(|(fmt, _)| fmt)
    }

    /// Get a format by name.
    pub fn get(&self, name: &str) -> Option<&dyn LogFormat> {
        self.formats
            .iter()
            .find(|f| f.name() == name)
            .map(|f| f.as_ref())
    }

    /// List all available format names.
    pub fn list(&self) -> Vec<&'static str> {
        self.formats.iter().map(|f| f.name()).collect()
    }

    /// List subagent sessions across all registered formats.
    pub fn list_subagent_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> {
        let mut all = Vec::new();
        for fmt in &self.formats {
            all.extend(fmt.list_subagent_sessions(project));
        }
        all
    }
}

/// Parse a session log with auto-format detection.
pub fn parse_session(path: &Path) -> Result<Session, String> {
    let registry = FormatRegistry::new();
    let format = registry
        .detect(path)
        .ok_or_else(|| format!("Unknown log format: {}", path.display()))?;
    format.parse(path)
}

/// Parse a session log with explicit format.
pub fn parse_session_with_format(path: &Path, format_name: &str) -> Result<Session, String> {
    let registry = FormatRegistry::new();
    let format = registry
        .get(format_name)
        .ok_or_else(|| format!("Unknown format: {}", format_name))?;
    format.parse(path)
}

/// Helper: read first N lines of a file.
pub(crate) fn peek_lines(path: &Path, n: usize) -> Vec<String> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .take(n)
        .filter_map(|l| l.ok())
        .collect()
}

/// Helper: read entire file as string.
pub(crate) fn read_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| e.to_string())?;
    Ok(content)
}
