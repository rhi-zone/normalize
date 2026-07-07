//! Session source plugins.
//!
//! Each format implements the `SessionSource` trait for discovering and loading
//! session logs. This replaces the former `LogFormat` trait (Phase 1 of the
//! SessionSource redesign).
//!
//! # Extensibility
//!
//! Users can register custom sources via [`register()`]:
//!
//! ```ignore
//! use normalize_chat_sessions::{SessionSource, SessionRef, register};
//! use std::path::{Path, PathBuf};
//!
//! struct MyAgentSource;
//! impl SessionSource for MyAgentSource {
//!     fn name(&self) -> &'static str { "myagent" }
//!     fn sessions_root(&self, _project: Option<&Path>) -> PathBuf { PathBuf::from("/tmp") }
//!     fn detect(&self, _path: &Path) -> f64 { 0.0 }
//!     fn discover(&self, _root: &Path) -> Result<Vec<SessionRef>, normalize_chat_sessions::DiscoverError> { Ok(vec![]) }
//!     fn load(&self, _r: &SessionRef) -> Result<normalize_chat_sessions::Session, normalize_chat_sessions::formats::ParseError> {
//!         Err(normalize_chat_sessions::formats::ParseError::Other("not implemented".into()))
//!     }
//! }
//! register(&MyAgentSource);
//! ```

#[cfg(any(feature = "format-cline", feature = "format-roo"))]
mod anthropic_history;
#[cfg(feature = "format-claude")]
mod claude_code;
#[cfg(feature = "format-cline")]
mod cline;
#[cfg(feature = "format-codex")]
mod codex;
#[cfg(feature = "format-gemini")]
mod gemini_cli;
#[cfg(feature = "format-normalize")]
mod normalize_agent;
#[cfg(feature = "format-roo")]
mod roo_code;

#[cfg(feature = "format-claude")]
pub use claude_code::ClaudeCodeFormat;
#[cfg(feature = "format-cline")]
pub use cline::ClineFormat;
#[cfg(feature = "format-codex")]
pub use codex::CodexFormat;
#[cfg(feature = "format-gemini")]
pub use gemini_cli::GeminiCliFormat;
#[cfg(feature = "format-normalize")]
pub use normalize_agent::NormalizeAgentFormat;
#[cfg(feature = "format-roo")]
pub use roo_code::RooCodeFormat;

use crate::Session;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

/// Error type for session log parsing operations.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// I/O error reading a session log file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Structural parse error in a session log file.
    #[error("parse error in {path}: {message}")]
    Format { path: PathBuf, message: String },
    /// Database query or decode error (for DB-backed sources, e.g. Phase 2 opencode).
    ///
    /// Note: `load()` is intentionally sync. DB implementations bridge async
    /// internally via `block_on` — this keeps the trait simple.
    #[error("database error: {0}")]
    Database(String),
    /// Other error (e.g. unknown format, registry failure).
    #[error("{0}")]
    Other(String),
}

/// Error returned by [`SessionSource::discover`].
#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// Where a session's data lives on disk (or in a database).
///
/// Phase 1 only produces `File` variants. `Directory` and `Database` are
/// reserved for Phase 2 sources (opencode SQLite, directory-per-session formats).
#[derive(Debug, Clone)]
pub enum SessionLocation {
    /// A single JSONL or JSON file.
    File(PathBuf),
    /// A directory containing session data.
    Directory(PathBuf),
    /// A row in a SQLite/libsql database (Phase 2).
    Database {
        db_path: PathBuf,
        session_id: String,
    },
}

impl SessionLocation {
    /// The filesystem path most relevant for display and age calculation.
    pub fn display_path(&self) -> &Path {
        match self {
            SessionLocation::File(p) | SessionLocation::Directory(p) => p.as_path(),
            SessionLocation::Database { db_path, .. } => db_path.as_path(),
        }
    }

    /// A stable string key for caching parsed sessions.
    pub fn cache_key(&self) -> String {
        match self {
            SessionLocation::File(p) | SessionLocation::Directory(p) => p.display().to_string(),
            SessionLocation::Database {
                db_path,
                session_id,
            } => {
                format!("{}::{}", db_path.display(), session_id)
            }
        }
    }
}

/// A discovered session reference — lightweight metadata before full load.
///
/// The `path` field is a convenience copy of `location.display_path()` retained
/// for backward compatibility with call sites that access `.path` directly.
pub struct SessionRef {
    /// Format that owns this session (e.g. `"claude"`, `"normalize"`).
    pub format: &'static str,
    /// Where the session data lives.
    pub location: SessionLocation,
    /// Convenience copy: `location.display_path().to_path_buf()`.
    pub path: PathBuf,
    /// File modification time (or equivalent).
    pub mtime: SystemTime,
    /// Parent session ID when this is a subagent session; `None` for top-level.
    pub parent_session_id: Option<String>,
    /// Agent ID for subagent sessions (e.g. `"agent-a5c5ccc9c2b61e757"`).
    pub agent_id: Option<String>,
    /// Subagent type (e.g. `"general-purpose"`, `"Explore"`, `"interactive"`).
    pub subagent_type: Option<String>,
}

/// Backward-compatibility alias: `SessionFile = SessionRef`.
///
/// Existing call sites typed as `Vec<SessionFile>` or `SessionFile { ... }` continue
/// to compile. New code should prefer `SessionRef`.
pub type SessionFile = SessionRef;

/// Trait for session source plugins.
///
/// Replaces the former `LogFormat` trait. Implementors know how to:
/// - resolve the directory where sessions live (`sessions_root`)
/// - enumerate session references without fully parsing (`discover`)
/// - fully load a [`Session`] from a reference (`load`)
pub trait SessionSource: Send + Sync {
    /// Format identifier (e.g., `"claude"`, `"codex"`, `"gemini"`, `"normalize"`).
    fn name(&self) -> &'static str;

    /// Primary sessions directory for the given project scope.
    ///
    /// - `project = None` → resolve from cwd / environment.
    /// - `project = Some(p)` → project-specific sessions directory.
    ///
    /// Replaces `LogFormat::sessions_dir`.
    fn sessions_root(&self, project: Option<&Path>) -> PathBuf;

    /// Roots to search for sessions during default (no explicit project) discovery.
    ///
    /// Default returns `vec![self.sessions_root(None)]`.
    fn default_roots(&self) -> Vec<PathBuf> {
        vec![self.sessions_root(None)]
    }

    /// Root directory containing all project-scoped session directories.
    ///
    /// Used by `--all-projects` listings. Returns `None` for formats that do not
    /// organise sessions one-directory-per-project.
    fn projects_root(&self) -> Option<PathBuf> {
        None
    }

    /// Confidence score [0.0, 1.0] that this source can parse `path`.
    fn detect(&self, path: &Path) -> f64;

    /// Enumerate all sessions under `root` (interactive and subagent).
    ///
    /// Subagent sessions have `parent_session_id.is_some()`.
    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError>;

    /// Fully load a session from a reference previously returned by `discover`.
    ///
    /// Intentionally synchronous. DB-backed implementations (Phase 2) bridge async
    /// internally via `block_on`.
    fn load(&self, r: &SessionRef) -> Result<Session, ParseError>;

    /// External directories that hold metadata for the given root/project.
    ///
    /// Used by `normalize sync`. Default returns `vec![root.to_owned()]`.
    fn metadata_roots(&self, root: &Path) -> Vec<PathBuf> {
        vec![root.to_owned()]
    }
}

// ── Global registry ──────────────────────────────────────────────────────────

static SOURCES: RwLock<Vec<&'static dyn SessionSource>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom session source plugin in the global registry.
///
/// Call before any parsing operations. Built-in sources are registered automatically.
pub fn register(source: &'static dyn SessionSource) {
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    SOURCES.write().unwrap().push(source);
}

fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
        let mut sources = SOURCES.write().unwrap();
        #[cfg(feature = "format-claude")]
        sources.push(&ClaudeCodeFormat);
        #[cfg(feature = "format-cline")]
        sources.push(&ClineFormat);
        #[cfg(feature = "format-codex")]
        sources.push(&CodexFormat);
        #[cfg(feature = "format-gemini")]
        sources.push(&GeminiCliFormat);
        #[cfg(feature = "format-normalize")]
        sources.push(&NormalizeAgentFormat);
        #[cfg(feature = "format-roo")]
        sources.push(&RooCodeFormat);
    });
}

/// Get a source by name from the global registry.
pub fn get_format(name: &str) -> Option<&'static dyn SessionSource> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    SOURCES
        .read()
        .unwrap()
        .iter()
        .find(|s| s.name() == name)
        .copied()
}

/// Auto-detect source for a file using the global registry.
pub fn detect_format(path: &Path) -> Option<&'static dyn SessionSource> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    let sources = SOURCES.read().unwrap();
    let mut best: Option<(&'static dyn SessionSource, f64)> = None;
    for src in sources.iter() {
        let score = src.detect(path);
        if score > 0.0 && best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((*src, score));
        }
    }
    best.map(|(src, _)| src)
}

/// List all available format names from the global registry.
pub fn list_formats() -> Vec<&'static str> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    SOURCES.read().unwrap().iter().map(|s| s.name()).collect()
}

/// Returns all external metadata directories for the given project across all known sources.
///
/// Only directories that actually exist on disk are returned. Used by `normalize sync`.
pub fn project_metadata_roots(project: &Path) -> Vec<PathBuf> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    SOURCES
        .read()
        .unwrap()
        .iter()
        .flat_map(|s| s.metadata_roots(&s.sessions_root(Some(project))))
        .filter(|p| p.exists())
        .collect()
}

// ── Helper free functions ────────────────────────────────────────────────────

/// List `.jsonl` files in a directory as interactive-session `SessionRef`s.
pub fn list_jsonl_sessions(dir: &Path) -> Vec<SessionRef> {
    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
                && let Ok(meta) = path.metadata()
                && let Ok(mtime) = meta.modified()
            {
                sessions.push(SessionRef {
                    format: "",
                    location: SessionLocation::File(path.clone()),
                    path,
                    mtime,
                    parent_session_id: None,
                    agent_id: None,
                    subagent_type: Some("interactive".into()),
                });
            }
        }
    }
    sessions
}

/// List subagent sessions from `<session-uuid>/subagents/` directories under `dir`.
///
/// Walks each subdirectory looking for a `subagents/` folder containing
/// `agent-<id>.jsonl` files. Reads `.meta.json` companions for agent type.
pub fn list_subagent_sessions(dir: &Path) -> Vec<SessionRef> {
    let mut sessions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sessions;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let parent_session_id = path.file_name().and_then(|n| n.to_str()).map(String::from);
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
            sessions.push(SessionRef {
                format: "",
                location: SessionLocation::File(sub_path.clone()),
                path: sub_path,
                mtime,
                parent_session_id: parent_session_id.clone(),
                agent_id: Some(stem.clone()),
                subagent_type,
            });
        }
    }
    sessions
}

// ── FormatRegistry ───────────────────────────────────────────────────────────

/// Isolated registry of session sources.
///
/// For most use cases, prefer the global-registry free functions ([`register`],
/// [`get_format`], [`detect_format`], [`list_formats`]).
///
/// Use `FormatRegistry` when you need an isolated instance (e.g., testing).
pub struct FormatRegistry {
    sources: Vec<Box<dyn SessionSource>>,
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatRegistry {
    /// Create a new registry with all built-in sources.
    #[allow(clippy::vec_init_then_push)] // cfg-gated pushes can't use vec![]
    pub fn new() -> Self {
        let mut sources: Vec<Box<dyn SessionSource>> = Vec::new();
        #[cfg(feature = "format-claude")]
        sources.push(Box::new(ClaudeCodeFormat));
        #[cfg(feature = "format-cline")]
        sources.push(Box::new(ClineFormat));
        #[cfg(feature = "format-codex")]
        sources.push(Box::new(CodexFormat));
        #[cfg(feature = "format-gemini")]
        sources.push(Box::new(GeminiCliFormat));
        #[cfg(feature = "format-normalize")]
        sources.push(Box::new(NormalizeAgentFormat));
        #[cfg(feature = "format-roo")]
        sources.push(Box::new(RooCodeFormat));
        Self { sources }
    }

    /// Create an empty registry.
    pub fn empty() -> Self {
        Self { sources: vec![] }
    }

    /// Register a custom source.
    pub fn register(&mut self, source: Box<dyn SessionSource>) {
        self.sources.push(source);
    }

    /// Detect the best source for a file.
    pub fn detect(&self, path: &Path) -> Option<&dyn SessionSource> {
        let mut best: Option<(&dyn SessionSource, f64)> = None;
        for src in &self.sources {
            let score = src.detect(path);
            if score > 0.0 && best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((src.as_ref(), score));
            }
        }
        best.map(|(src, _)| src)
    }

    /// Get a source by name.
    pub fn get(&self, name: &str) -> Option<&dyn SessionSource> {
        self.sources
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
    }

    /// List all available format names.
    pub fn list(&self) -> Vec<&'static str> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    /// Discover sessions from `root` across all registered sources.
    pub fn discover(&self, root: &Path) -> Vec<SessionRef> {
        let mut all = Vec::new();
        for src in &self.sources {
            if let Ok(refs) = src.discover(root) {
                all.extend(refs);
            }
        }
        all
    }

    /// Load a session, routing to the source named by `r.format`.
    ///
    /// Falls back to format auto-detection if `r.format` is empty or unrecognised.
    pub fn load(&self, r: &SessionRef) -> Result<Session, ParseError> {
        if let Some(src) = self.get(r.format) {
            return src.load(r);
        }
        let src = self.detect(&r.path).ok_or_else(|| {
            ParseError::Other(format!("Unknown format for: {}", r.path.display()))
        })?;
        src.load(r)
    }

    /// All metadata roots for `project_path` across all sources.
    pub fn project_roots(&self, project_path: &Path) -> Vec<PathBuf> {
        self.sources
            .iter()
            .flat_map(|s| s.metadata_roots(&s.sessions_root(Some(project_path))))
            .filter(|p| p.exists())
            .collect()
    }

    /// List all subagent sessions across all registered sources for a project.
    pub fn list_subagent_sessions(&self, project: Option<&Path>) -> Vec<SessionRef> {
        let mut all = Vec::new();
        for src in &self.sources {
            let root = src.sessions_root(project);
            if let Ok(refs) = src.discover(&root) {
                all.extend(refs.into_iter().filter(|r| r.parent_session_id.is_some()));
            }
        }
        all
    }
}

// ── Free-function shims ──────────────────────────────────────────────────────

/// Parse a session log with auto-format detection.
pub fn parse_session(path: &Path) -> Result<Session, ParseError> {
    let registry = FormatRegistry::new();
    let source = registry
        .detect(path)
        .ok_or_else(|| ParseError::Other(format!("Unknown log format: {}", path.display())))?;
    let mtime = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let r = SessionRef {
        format: source.name(),
        location: SessionLocation::File(path.to_path_buf()),
        path: path.to_path_buf(),
        mtime,
        parent_session_id: None,
        agent_id: None,
        subagent_type: None,
    };
    source.load(&r)
}

/// Parse a session log with an explicit format name.
pub fn parse_session_with_format(path: &Path, format_name: &str) -> Result<Session, ParseError> {
    let registry = FormatRegistry::new();
    let source = registry
        .get(format_name)
        .ok_or_else(|| ParseError::Other(format!("Unknown format: {}", format_name)))?;
    let mtime = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let r = SessionRef {
        format: source.name(),
        location: SessionLocation::File(path.to_path_buf()),
        path: path.to_path_buf(),
        mtime,
        parent_session_id: None,
        agent_id: None,
        subagent_type: None,
    };
    source.load(&r)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Read first N lines of a file.
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

/// Read entire file as string.
pub(crate) fn read_file(path: &Path) -> Result<String, ParseError> {
    let mut file = File::open(path).map_err(|e| ParseError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(content)
}
