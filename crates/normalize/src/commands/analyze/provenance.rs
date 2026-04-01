//! Provenance graph: git blame → session mapping + logical code relations.
//!
//! Produces a graph linking commits to files (blame), sessions to commits
//! (authored), and optionally imports/calls/co-change edges.

use super::git_utils;
use crate::output::OutputFormatter;
use normalize_chat_sessions::{ClaudeCodeFormat, ContentBlock, LogFormat, Session};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ── Data structures ──────────────────────────────────────────────

/// A node in the provenance graph.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProvenanceNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Node type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Symbol,
    Commit,
    Session,
}

/// An edge in the provenance graph.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProvenanceEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<usize>,
}

/// Edge type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Blame,
    Authored,
    Imports,
    Calls,
    CoChanged,
}

/// Summary statistics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProvenanceStats {
    pub total_files: usize,
    pub total_commits: usize,
    pub matched_sessions: usize,
    pub unmatched_commits: usize,
    pub import_edges: usize,
    pub call_edges: usize,
    pub co_change_edges: usize,
}

/// Full provenance report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ProvenanceReport {
    pub nodes: Vec<ProvenanceNode>,
    pub edges: Vec<ProvenanceEdge>,
    pub stats: ProvenanceStats,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

// ── Commit → session mapping ─────────────────────────────────────

/// Maps a commit hash to the session that authored it.
#[derive(Debug)]
struct CommitSession {
    session_id: String,
    timestamp: Option<String>,
}

struct CommitSessionMap {
    map: HashMap<String, CommitSession>,
    warnings: Vec<String>,
}

/// Build commit→session map by scanning session tool-use blocks.
fn build_commit_session_map(root: &Path, sessions_path: Option<&Path>) -> CommitSessionMap {
    let mut warnings = Vec::new();
    let format = ClaudeCodeFormat;

    let session_files = if let Some(dir) = sessions_path {
        normalize_chat_sessions::list_jsonl_sessions(dir)
    } else {
        format.list_sessions(Some(root))
    };

    if session_files.is_empty() {
        warnings.push("No session files found".to_string());
        return CommitSessionMap {
            map: HashMap::new(),
            warnings,
        };
    }

    // Regex for git commit output: [branch hash] message
    // normalize-syntax-allow: rust/unwrap-in-impl - compile-time constant regex pattern
    let commit_re = Regex::new(r"\[[\w./-]+ ([a-f0-9]{7,40})\]").unwrap();

    let mut short_to_session: HashMap<String, CommitSession> = HashMap::new();

    for sf in &session_files {
        let session: Session = match if sessions_path.is_some() {
            normalize_chat_sessions::parse_session(&sf.path)
        } else {
            format.parse(&sf.path)
        } {
            Ok(s) => s,
            Err(_) => continue,
        };

        let session_id = session.metadata.session_id.clone().unwrap_or_else(|| {
            sf.path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        });
        let timestamp = session.metadata.timestamp.clone();

        // Walk content blocks to find git commit tool uses and their results
        for turn in &session.turns {
            for msg in &turn.messages {
                // Collect tool-use IDs that are git commit commands
                let mut commit_tool_ids: HashSet<String> = HashSet::new();
                for block in &msg.content {
                    if let ContentBlock::ToolUse { id, name, input } = block
                        && (name == "Bash" || name == "bash")
                    {
                        let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        if cmd.contains("git commit") {
                            commit_tool_ids.insert(id.clone());
                        }
                    }
                }

                // Find matching tool results and extract commit hashes
                if !commit_tool_ids.is_empty() {
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = block
                        {
                            if *is_error {
                                continue;
                            }
                            if commit_tool_ids.contains(tool_use_id)
                                && let Some(caps) = commit_re.captures(content)
                            {
                                let short_hash = caps[1].to_string();
                                short_to_session.insert(
                                    short_hash,
                                    CommitSession {
                                        session_id: session_id.clone(),
                                        timestamp: timestamp.clone(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Resolve short hashes to full via git rev-parse
    let mut full_map: HashMap<String, CommitSession> = HashMap::new();
    for (short, cs) in short_to_session {
        if short.len() >= 40 {
            full_map.insert(short, cs);
        } else if let Some(full) = resolve_full_hash(root, &short) {
            full_map.insert(full, cs);
        } else {
            // Keep short hash as-is if rev-parse fails
            full_map.insert(short, cs);
        }
    }

    CommitSessionMap {
        map: full_map,
        warnings,
    }
}

fn resolve_full_hash(root: &Path, short: &str) -> Option<String> {
    git_utils::resolve_ref(root, short).ok()
}

// ── Blame extraction ─────────────────────────────────────────────

/// Per-file blame: commit hash → line count.
fn blame_file(root: &Path, path: &str) -> Option<HashMap<String, usize>> {
    let repo = git_utils::open_repo(root)?;
    let head_id = repo.head_id().ok()?;
    let path_bstr: &gix::bstr::BStr = path.as_bytes().into();
    let outcome = repo
        .blame_file(
            path_bstr,
            head_id.detach(),
            gix::repository::blame_file::Options::default(),
        )
        .ok()?;

    // Accumulate per-commit line counts from blame entries.
    let mut commit_lines: HashMap<String, usize> = HashMap::new();
    for entry in &outcome.entries {
        let hash = entry.commit_id.to_hex().to_string();
        *commit_lines.entry(hash).or_default() += entry.len.get() as usize;
    }

    if commit_lines.is_empty() {
        None
    } else {
        Some(commit_lines)
    }
}

/// Collect git-tracked source files, optionally scoped to a target.
fn git_tracked_files(root: &Path, target: Option<&str>) -> Vec<String> {
    let all = git_utils::git_ls_files(root);
    all.into_iter()
        .filter(|l| {
            let p = Path::new(l.as_str());
            super::is_source_file(p) && target.is_none_or(|t| l.starts_with(t))
        })
        .collect()
}

// ── Co-change extraction ─────────────────────────────────────────

fn build_co_change_edges(root: &Path, files: &HashSet<String>) -> Vec<ProvenanceEdge> {
    let per_commit = git_utils::git_per_commit_files(root);

    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
    for mut commit_files in per_commit {
        if commit_files.len() < 2 || commit_files.len() > 50 {
            continue;
        }
        commit_files.sort();
        commit_files.dedup();
        for i in 0..commit_files.len() {
            for j in (i + 1)..commit_files.len() {
                if files.contains(&commit_files[i]) && files.contains(&commit_files[j]) {
                    let key = (commit_files[i].clone(), commit_files[j].clone());
                    *pair_counts.entry(key).or_default() += 1;
                }
            }
        }
    }

    pair_counts
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .map(|((a, b), count)| ProvenanceEdge {
            source: format!("file:{}", a),
            target: format!("file:{}", b),
            kind: EdgeKind::CoChanged,
            weight: Some(count),
        })
        .collect()
}

// ── Import graph extraction ──────────────────────────────────────

async fn build_import_edges(root: &Path, files: &HashSet<String>) -> Vec<ProvenanceEdge> {
    {
        let idx = match crate::index::ensure_ready_or_warn(root).await {
            Some(idx) => idx,
            None => return Vec::new(),
        };

        let graph = match super::architecture::build_import_graph(&idx).await {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let mut edges = Vec::new();
        for (source, targets) in &graph.imports_by_file {
            if !files.contains(source) {
                continue;
            }
            for target in targets {
                if files.contains(target) && source != target {
                    edges.push(ProvenanceEdge {
                        source: format!("file:{}", source),
                        target: format!("file:{}", target),
                        kind: EdgeKind::Imports,
                        weight: None,
                    });
                }
            }
        }
        edges
    }
}

// ── Call graph extraction ────────────────────────────────────────

async fn build_call_edges(root: &Path) -> Vec<(ProvenanceEdge, ProvenanceNode, ProvenanceNode)> {
    {
        let idx = match crate::index::ensure_ready_or_warn(root).await {
            Some(idx) => idx,
            None => return Vec::new(),
        };

        // (caller_file, caller_symbol, callee_name, line)
        let calls = match idx.all_calls_with_lines().await {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for (caller_file, caller_sym, callee_name, _line) in &calls {
            let src_id = format!("symbol:{}:{}", caller_file, caller_sym);
            let tgt_id = format!("symbol:{}", callee_name);
            let key = (src_id.clone(), tgt_id.clone());
            if !seen.insert(key) {
                continue;
            }

            let src_node = ProvenanceNode {
                id: src_id.clone(),
                kind: NodeKind::Symbol,
                label: caller_sym.clone(),
                metadata: None,
            };
            let tgt_node = ProvenanceNode {
                id: tgt_id.clone(),
                kind: NodeKind::Symbol,
                label: callee_name.clone(),
                metadata: None,
            };
            let edge = ProvenanceEdge {
                source: src_id,
                target: tgt_id,
                kind: EdgeKind::Calls,
                weight: None,
            };

            results.push((edge, src_node, tgt_node));
        }

        results
    }
}

// ── Graph assembly ───────────────────────────────────────────────

/// Provenance analysis options.
pub struct ProvenanceOptions {
    pub target: Option<String>,
    pub include_calls: bool,
    pub include_coupling: bool,
    pub sessions_path: Option<String>,
    pub limit: usize,
}

/// Main entry point: assemble the provenance graph.
pub async fn analyze_provenance(root: &Path, opts: &ProvenanceOptions) -> ProvenanceReport {
    let mut nodes: Vec<ProvenanceNode> = Vec::new();
    let mut edges: Vec<ProvenanceEdge> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // 1. Collect files
    let tracked = git_tracked_files(root, opts.target.as_deref());
    let files: Vec<String> = tracked.into_iter().take(opts.limit).collect();
    let file_set: HashSet<String> = files.iter().cloned().collect();

    if files.is_empty() {
        warnings.push("No source files found".to_string());
        return ProvenanceReport {
            nodes,
            edges,
            stats: ProvenanceStats {
                total_files: 0,
                total_commits: 0,
                matched_sessions: 0,
                unmatched_commits: 0,
                import_edges: 0,
                call_edges: 0,
                co_change_edges: 0,
            },
            warnings,
        };
    }

    // Add file nodes
    for f in &files {
        nodes.push(ProvenanceNode {
            id: format!("file:{}", f),
            kind: NodeKind::File,
            label: f.clone(),
            metadata: None,
        });
    }

    // 2. Build commit→session map
    let sessions_dir = opts.sessions_path.as_ref().map(|p| Path::new(p.as_str()));
    let csm = build_commit_session_map(root, sessions_dir);
    warnings.extend(csm.warnings);
    let commit_session_map = csm.map;

    // 3. Blame extraction (parallel)
    let blame_results: Vec<(String, HashMap<String, usize>)> = files
        .par_iter()
        .filter_map(|path| blame_file(root, path).map(|b| (path.clone(), b)))
        .collect();

    let mut all_commits: HashSet<String> = HashSet::new();
    for (file, commit_lines) in &blame_results {
        for (hash, count) in commit_lines {
            all_commits.insert(hash.clone());
            edges.push(ProvenanceEdge {
                source: format!("commit:{}", &hash[..7.min(hash.len())]),
                target: format!("file:{}", file),
                kind: EdgeKind::Blame,
                weight: Some(*count),
            });
        }
    }

    // Add commit nodes
    let mut matched_sessions: HashSet<String> = HashSet::new();
    for hash in &all_commits {
        let short = &hash[..7.min(hash.len())];
        let label = if let Some(cs) = commit_session_map.get(hash) {
            matched_sessions.insert(cs.session_id.clone());
            format!(
                "{} (session: {})",
                short,
                &cs.session_id[..8.min(cs.session_id.len())]
            )
        } else {
            short.to_string()
        };
        nodes.push(ProvenanceNode {
            id: format!("commit:{}", short),
            kind: NodeKind::Commit,
            label,
            metadata: None,
        });
    }

    // Add session nodes + authored edges
    let mut session_commit_counts: HashMap<String, usize> = HashMap::new();
    for hash in &all_commits {
        if let Some(cs) = commit_session_map.get(hash) {
            *session_commit_counts
                .entry(cs.session_id.clone())
                .or_default() += 1;
            let short = &hash[..7.min(hash.len())];
            edges.push(ProvenanceEdge {
                source: format!("session:{}", &cs.session_id[..8.min(cs.session_id.len())]),
                target: format!("commit:{}", short),
                kind: EdgeKind::Authored,
                weight: None,
            });
        }
    }

    for session_id in session_commit_counts.keys() {
        let short_id = &session_id[..8.min(session_id.len())];
        let ts = commit_session_map
            .values()
            .find(|cs| cs.session_id == *session_id)
            .and_then(|cs| cs.timestamp.clone());
        nodes.push(ProvenanceNode {
            id: format!("session:{}", short_id),
            kind: NodeKind::Session,
            label: if let Some(ref t) = ts {
                format!("{} ({})", short_id, t)
            } else {
                short_id.to_string()
            },
            metadata: ts.map(|t| serde_json::json!({"timestamp": t})),
        });
    }

    // 4. Import edges
    let import_edges = build_import_edges(root, &file_set).await;
    let import_count = import_edges.len();
    edges.extend(import_edges);

    // 5. Call edges (optional)
    let mut call_count = 0;
    if opts.include_calls {
        let call_data = build_call_edges(root).await;
        call_count = call_data.len();
        if call_count == 0 {
            warnings.push("No call graph data found (is the facts index built?)".to_string());
        }
        let mut seen_nodes: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        for (edge, src_node, tgt_node) in call_data {
            if seen_nodes.insert(src_node.id.clone()) {
                nodes.push(src_node);
            }
            if seen_nodes.insert(tgt_node.id.clone()) {
                nodes.push(tgt_node);
            }
            edges.push(edge);
        }
    }

    // 6. Co-change edges (optional)
    let mut co_change_count = 0;
    if opts.include_coupling {
        let co_edges = build_co_change_edges(root, &file_set);
        co_change_count = co_edges.len();
        edges.extend(co_edges);
    }

    let unmatched = all_commits
        .iter()
        .filter(|h| !commit_session_map.contains_key(*h))
        .count();

    ProvenanceReport {
        nodes,
        edges,
        stats: ProvenanceStats {
            total_files: files.len(),
            total_commits: all_commits.len(),
            matched_sessions: matched_sessions.len(),
            unmatched_commits: unmatched,
            import_edges: import_count,
            call_edges: call_count,
            co_change_edges: co_change_count,
        },
        warnings,
    }
}

// ── OutputFormatter ──────────────────────────────────────────────

impl OutputFormatter for ProvenanceReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Provenance — {} files, {} commits, {} sessions ({} unmatched)",
            self.stats.total_files,
            self.stats.total_commits,
            self.stats.matched_sessions,
            self.stats.unmatched_commits,
        ));
        lines.push(String::new());

        // Session coverage
        let mut session_commits: HashMap<String, usize> = HashMap::new();
        for edge in &self.edges {
            if edge.kind == EdgeKind::Authored {
                *session_commits.entry(edge.source.clone()).or_default() += 1;
            }
        }

        if !session_commits.is_empty() {
            lines.push("Session Coverage".to_string());
            let mut sorted: Vec<_> = session_commits.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            for (session_id, count) in &sorted {
                let pct = if self.stats.total_commits > 0 {
                    **count as f64 / self.stats.total_commits as f64 * 100.0
                } else {
                    0.0
                };
                let label = session_id.strip_prefix("session:").unwrap_or(session_id);
                lines.push(format!(
                    "  {:<16} {:>4} commits ({:>4.0}%)",
                    label, count, pct
                ));
            }
            lines.push(String::new());
        }

        // Top files by session (blame attribution)
        let mut file_session_lines: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut file_total_lines: HashMap<String, usize> = HashMap::new();

        for edge in &self.edges {
            if edge.kind == EdgeKind::Blame {
                let file_id = &edge.target;
                let commit_id = &edge.source;
                let weight = edge.weight.unwrap_or(1);
                *file_total_lines.entry(file_id.clone()).or_default() += weight;

                // Find which session authored this commit
                let commit_short = commit_id.strip_prefix("commit:").unwrap_or(commit_id);
                let session = self
                    .edges
                    .iter()
                    .find(|e| {
                        e.kind == EdgeKind::Authored
                            && e.target.strip_prefix("commit:").unwrap_or(&e.target) == commit_short
                    })
                    .map(|e| e.source.clone())
                    .unwrap_or_else(|| "unattributed".to_string());

                *file_session_lines
                    .entry(file_id.clone())
                    .or_default()
                    .entry(session)
                    .or_default() += weight;
            }
        }

        if !file_session_lines.is_empty() {
            lines.push("Top Files by Session".to_string());
            let mut file_entries: Vec<_> = file_session_lines.iter().collect();
            file_entries.sort_by(|a, b| {
                let total_a = file_total_lines.get(a.0).unwrap_or(&0);
                let total_b = file_total_lines.get(b.0).unwrap_or(&0);
                total_b.cmp(total_a)
            });

            for (file_id, sessions) in file_entries.iter().take(15) {
                let file_label = file_id.strip_prefix("file:").unwrap_or(file_id);
                let total = file_total_lines.get(*file_id).unwrap_or(&1);
                let mut sorted_sessions: Vec<_> = sessions.iter().collect();
                sorted_sessions.sort_by(|a, b| b.1.cmp(a.1));

                let parts: Vec<String> = sorted_sessions
                    .iter()
                    .take(3)
                    .map(|(s, count)| {
                        let pct = **count as f64 / *total as f64 * 100.0;
                        let label = s.strip_prefix("session:").unwrap_or(s);
                        format!("{} ({:.0}%)", label, pct)
                    })
                    .collect();

                let display_path = if file_label.len() > 40 {
                    format!("...{}", &file_label[file_label.len() - 37..])
                } else {
                    file_label.to_string()
                };
                lines.push(format!("  {:<42} {}", display_path, parts.join(", ")));
            }
            lines.push(String::new());
        }

        // Edge summary
        let mut edge_parts = Vec::new();
        if self.stats.import_edges > 0 {
            edge_parts.push(format!("{} imports", self.stats.import_edges));
        }
        if self.stats.call_edges > 0 {
            edge_parts.push(format!("{} calls", self.stats.call_edges));
        }
        if self.stats.co_change_edges > 0 {
            edge_parts.push(format!("{} co-changes", self.stats.co_change_edges));
        }
        if !edge_parts.is_empty() {
            lines.push(format!("Edges: {}", edge_parts.join(", ")));
        }

        for w in &self.warnings {
            lines.push(format!("Warning: {}", w));
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "\x1b[1mProvenance\x1b[0m — {} files, {} commits, \x1b[36m{} sessions\x1b[0m ({} unmatched)",
            self.stats.total_files,
            self.stats.total_commits,
            self.stats.matched_sessions,
            self.stats.unmatched_commits,
        ));
        lines.push(String::new());

        // Session coverage with color
        let mut session_commits: HashMap<String, usize> = HashMap::new();
        for edge in &self.edges {
            if edge.kind == EdgeKind::Authored {
                *session_commits.entry(edge.source.clone()).or_default() += 1;
            }
        }

        if !session_commits.is_empty() {
            lines.push("\x1b[1mSession Coverage\x1b[0m".to_string());
            let mut sorted: Vec<_> = session_commits.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            for (session_id, count) in &sorted {
                let pct = if self.stats.total_commits > 0 {
                    **count as f64 / self.stats.total_commits as f64 * 100.0
                } else {
                    0.0
                };
                let label = session_id.strip_prefix("session:").unwrap_or(session_id);
                let color = if pct > 50.0 {
                    "\x1b[32m"
                } else if pct > 20.0 {
                    "\x1b[33m"
                } else {
                    "\x1b[90m"
                };
                lines.push(format!(
                    "  \x1b[36m{:<16}\x1b[0m {:>4} commits ({}{}%\x1b[0m)",
                    label, count, color, pct as u32
                ));
            }
            lines.push(String::new());
        }

        // Edge summary
        let mut edge_parts = Vec::new();
        if self.stats.import_edges > 0 {
            edge_parts.push(format!("{} imports", self.stats.import_edges));
        }
        if self.stats.call_edges > 0 {
            edge_parts.push(format!("{} calls", self.stats.call_edges));
        }
        if self.stats.co_change_edges > 0 {
            edge_parts.push(format!("{} co-changes", self.stats.co_change_edges));
        }
        if !edge_parts.is_empty() {
            lines.push(format!("\x1b[1mEdges:\x1b[0m {}", edge_parts.join(", ")));
        }

        for w in &self.warnings {
            lines.push(format!("\x1b[33mWarning:\x1b[0m {}", w));
        }

        lines.join("\n")
    }
}
