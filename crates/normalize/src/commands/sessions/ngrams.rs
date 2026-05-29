//! N-gram frequency analysis of session message text.

use crate::output::OutputFormatter;
use crate::sessions::{ContentBlock, FormatRegistry, LogFormat, SessionFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime};

use super::stats::{list_all_project_sessions_by_mode, parse_date};
use super::{SessionMode, list_sessions_by_mode, session_matches_grep};

/// Role filter for ngram extraction.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NgramRole {
    /// Extract from all messages (default).
    #[default]
    All,
    /// Only assistant messages.
    Assistant,
    /// Only user messages.
    User,
}

impl FromStr for NgramRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(NgramRole::All),
            "assistant" | "asst" => Ok(NgramRole::Assistant),
            "user" => Ok(NgramRole::User),
            _ => Err(format!(
                "invalid role '{}': expected 'all', 'assistant', or 'user'",
                s
            )),
        }
    }
}

/// A single n-gram with its frequency count.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NgramEntry {
    /// The n-gram text (space-separated words).
    pub ngram: String,
    /// Number of times this n-gram appeared across all processed messages.
    pub count: usize,
}

/// Report from `normalize sessions ngrams`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct NgramsReport {
    /// Top-K most frequent n-grams, sorted by count descending.
    pub ngrams: Vec<NgramEntry>,
    /// N value used (bigram=2, trigram=3, etc.).
    pub n: usize,
    /// Total messages processed.
    pub messages_processed: usize,
    /// Total unique n-grams found (before truncation to top-K).
    pub total_unique: usize,
}

impl OutputFormatter for NgramsReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.ngrams.is_empty() {
            writeln!(out, "No {}-grams found.", self.n).unwrap();
            return out;
        }
        writeln!(
            out,
            "Top {} {}-grams ({} messages, {} unique {}-grams total)\n",
            self.ngrams.len(),
            self.n,
            self.messages_processed,
            self.total_unique,
            self.n,
        )
        .unwrap();
        // Find max count for bar chart scaling
        let max_count = self.ngrams.first().map(|e| e.count).unwrap_or(1);
        let bar_width = 30usize;
        for entry in &self.ngrams {
            let bar_len = (entry.count * bar_width) / max_count.max(1);
            let bar: String = "#".repeat(bar_len);
            writeln!(out, "  {:>6}  {:<30}  {}", entry.count, entry.ngram, bar).unwrap();
        }
        out
    }
}

/// Normalize text for n-gram extraction: lowercase, strip punctuation, collapse whitespace.
fn normalize_text(text: &str) -> String {
    // Replace punctuation with spaces, lowercase everything
    text.chars()
        .map(|c| {
            if c.is_alphabetic() || c.is_ascii_digit() || c == '\'' {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract n-grams from normalized text and update the frequency map.
fn extract_ngrams(text: &str, n: usize, counts: &mut HashMap<String, usize>) {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < n {
        return;
    }
    for i in 0..=(words.len() - n) {
        let ngram = words[i..i + n].join(" ");
        *counts.entry(ngram).or_insert(0) += 1;
    }
}

/// Build n-gram frequency report.
#[allow(clippy::too_many_arguments)]
pub fn build_ngrams_report(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    session_filter: Option<&str>,
    n: usize,
    top_k: usize,
    role: NgramRole,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    mode: &SessionMode,
    agent_type: Option<&str>,
) -> Result<NgramsReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").ok_or_else(|| {
            "Claude format not available (compile with feature = format-claude)".to_string()
        })?,
    };

    let grep_re = grep
        .map(|p| regex::Regex::new(p).map_err(|_| format!("Invalid grep pattern: {}", p)))
        .transpose()?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions_by_mode(format, mode)
    } else {
        let proj = project_filter.or(root);
        list_sessions_by_mode(format, proj, mode)
    };

    // Session ID filter
    if let Some(sid) = session_filter {
        sessions.retain(|s| {
            s.path
                .file_stem()
                .and_then(|n| n.to_str())
                .map(|n| n == sid || n.starts_with(sid))
                .unwrap_or(false)
        });
    }

    // Date filters
    let now = SystemTime::now();
    if let Some(d) = days {
        let st = now - Duration::from_secs(d as u64 * 86400);
        sessions.retain(|s| s.mtime >= st);
    }
    if let Some(s) = since {
        if let Some(st) = parse_date(s) {
            sessions.retain(|sf| sf.mtime >= st);
        } else {
            return Err(format!("Invalid date format: {} (use YYYY-MM-DD)", s));
        }
    }
    if let Some(u) = until {
        if let Some(ut) = parse_date(u) {
            let ut = ut + Duration::from_secs(86400);
            sessions.retain(|sf| sf.mtime <= ut);
        } else {
            return Err(format!("Invalid date format: {} (use YYYY-MM-DD)", u));
        }
    }

    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    sessions.sort_by_key(|b| std::cmp::Reverse(b.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut messages_processed = 0usize;

    for sf in &sessions {
        let Ok(session) = format.parse(&sf.path) else {
            continue;
        };

        for turn in &session.turns {
            for msg in &turn.messages {
                let role_str = msg.role.to_string();
                match role {
                    NgramRole::All => {}
                    NgramRole::Assistant => {
                        if role_str != "assistant" {
                            continue;
                        }
                    }
                    NgramRole::User => {
                        if role_str != "user" {
                            continue;
                        }
                    }
                }

                // Extract text from content blocks (skip tool results/uses for cleaner text)
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        ContentBlock::Thinking { text } => Some(text.as_str()),
                        // Skip tool calls and results — they contain code/paths, not natural language
                        ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                if text.is_empty() {
                    continue;
                }

                let normalized = normalize_text(&text);
                extract_ngrams(&normalized, n, &mut counts);
                messages_processed += 1;
            }
        }
    }

    let total_unique = counts.len();

    // Sort by count descending, then alphabetically for stable output
    let mut sorted: Vec<NgramEntry> = counts
        .into_iter()
        .map(|(ngram, count)| NgramEntry { ngram, count })
        .collect();
    sorted.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.ngram.cmp(&b.ngram)));
    sorted.truncate(top_k);

    Ok(NgramsReport {
        ngrams: sorted,
        n,
        messages_processed,
        total_unique,
    })
}
