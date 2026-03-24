//! Tool call sequence analysis using Markov chain transition matrices.

use crate::output::OutputFormatter;
use crate::sessions::{
    ContentBlock, FormatRegistry, LogFormat, Role, SessionFile, parse_session,
    parse_session_with_format,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use super::stats::{list_all_project_sessions_by_mode, parse_date};
use super::{SessionMode, list_sessions_by_mode, session_matches_grep};

/// A single session's metadata for the patterns report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SessionPatternMeta {
    /// Session file path.
    pub path: String,
    /// Number of turns.
    pub turn_count: usize,
    /// Total tool calls.
    pub tool_count: usize,
    /// First user message (truncated).
    pub first_user_message: String,
    /// Divergence score from population matrix (Frobenius norm).
    pub divergence: f64,
}

/// A transition matrix stored as (from, to) -> probability.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TransitionMatrix {
    /// Map of (from_state, to_state) -> probability.
    pub transitions: BTreeMap<String, BTreeMap<String, f64>>,
    /// All states observed.
    pub states: Vec<String>,
}

/// Per-session transition data.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SessionTransitions {
    /// Session file path.
    pub path: String,
    /// The session's transition matrix.
    pub matrix: TransitionMatrix,
    /// Divergence from population matrix.
    pub divergence: f64,
}

/// Report for the `sessions patterns` subcommand.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PatternsReport {
    /// Number of sessions analyzed.
    pub session_count: usize,
    /// Population-level transition matrix (all sessions aggregated).
    pub population_matrix: TransitionMatrix,
    /// Top outlier sessions ranked by divergence.
    pub outliers: Vec<SessionPatternMeta>,
    /// Most common starting tool (start -> X).
    pub common_start_tools: Vec<(String, f64)>,
    /// Most common ending tool (X -> end).
    pub common_end_tools: Vec<(String, f64)>,
    /// Per-session transition data.
    pub per_session: Vec<SessionTransitions>,
    /// Present when `--limit` truncated the session list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<super::TruncationInfo>,
}

impl OutputFormatter for PatternsReport {
    fn format_text(&self) -> String {
        let mut out = String::new();

        // Population transition matrix
        let _ = writeln!(
            out,
            "# Population Transition Matrix ({} sessions)",
            self.session_count
        );
        let _ = writeln!(out);
        format_matrix_text(&mut out, &self.population_matrix);
        let _ = writeln!(out);

        // Common start tools
        let _ = writeln!(out, "# Most Common Starting Tools (start -> X)");
        for (tool, prob) in &self.common_start_tools {
            let _ = writeln!(out, "  {} {:.1}%", tool, prob * 100.0);
        }
        let _ = writeln!(out);

        // Common end tools
        let _ = writeln!(out, "# Most Common Ending Tools (X -> end)");
        for (tool, prob) in &self.common_end_tools {
            let _ = writeln!(out, "  {} {:.1}%", tool, prob * 100.0);
        }
        let _ = writeln!(out);

        // Outliers
        let _ = writeln!(
            out,
            "# Top Outlier Sessions (by divergence from population)"
        );
        let _ = writeln!(out, "# divergence  turns  tools  session");
        for s in &self.outliers {
            let msg = truncate_str(&s.first_user_message, 60);
            let _ = writeln!(
                out,
                "  {:.4}  {}t  {}tc  {}  {}",
                s.divergence, s.turn_count, s.tool_count, s.path, msg
            );
        }

        if let Some(ref t) = self.truncated {
            let _ = writeln!(out);
            let _ = writeln!(out, "{}", t.notice());
        }

        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color::{Cyan, Green, Red, Yellow};

        let mut out = String::new();

        let _ = writeln!(
            out,
            "{}",
            Green.bold().paint(format!(
                "Population Transition Matrix ({} sessions)",
                self.session_count
            ))
        );
        let _ = writeln!(out);
        format_matrix_pretty(&mut out, &self.population_matrix);
        let _ = writeln!(out);

        let _ = writeln!(
            out,
            "{}",
            Green
                .bold()
                .paint("Most Common Starting Tools (start -> X)")
        );
        for (tool, prob) in &self.common_start_tools {
            let _ = writeln!(
                out,
                "  {} {}",
                Cyan.paint(tool.as_str()),
                Yellow.paint(format!("{:.1}%", prob * 100.0))
            );
        }
        let _ = writeln!(out);

        let _ = writeln!(
            out,
            "{}",
            Green.bold().paint("Most Common Ending Tools (X -> end)")
        );
        for (tool, prob) in &self.common_end_tools {
            let _ = writeln!(
                out,
                "  {} {}",
                Cyan.paint(tool.as_str()),
                Yellow.paint(format!("{:.1}%", prob * 100.0))
            );
        }
        let _ = writeln!(out);

        let _ = writeln!(
            out,
            "{}",
            Green
                .bold()
                .paint("Top Outlier Sessions (by divergence from population)")
        );
        for s in &self.outliers {
            let msg = truncate_str(&s.first_user_message, 60);
            let _ = writeln!(
                out,
                "  {} {}  {}  {}",
                Red.paint(format!("{:.4}", s.divergence)),
                Cyan.paint(format!("{}t {}tc", s.turn_count, s.tool_count)),
                Yellow.paint(&s.path),
                msg,
            );
        }

        if let Some(ref t) = self.truncated {
            use nu_ansi_term::Color::DarkGray;
            let _ = writeln!(out);
            let _ = writeln!(out, "{}", DarkGray.paint(t.notice()));
        }

        out
    }
}

/// Truncate a string at a char boundary, appending "..." if truncated.
fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

/// Format a transition matrix as a text table.
fn format_matrix_text(out: &mut String, matrix: &TransitionMatrix) {
    if matrix.states.is_empty() {
        let _ = writeln!(out, "(no data)");
        return;
    }

    // Find max label width; col_width must fit the full label plus a padding space
    let max_label = matrix.states.iter().map(|s| s.len()).max().unwrap_or(5);
    let col_width = matrix
        .states
        .iter()
        .map(|s| s.len() + 1)
        .max()
        .unwrap_or(7)
        .max(7);

    // Header row
    let _ = write!(out, "{:width$}", "", width = max_label + 2);
    for state in &matrix.states {
        let _ = write!(out, "{:>width$}", state, width = col_width);
    }
    let _ = writeln!(out);

    // Data rows
    for from in &matrix.states {
        let _ = write!(out, "{:width$}  ", from, width = max_label);
        for to in &matrix.states {
            let prob = matrix
                .transitions
                .get(from)
                .and_then(|m| m.get(to))
                .copied()
                .unwrap_or(0.0);
            if prob > 0.001 {
                let _ = write!(out, "{:>width$.1}%", prob * 100.0, width = col_width - 1);
            } else {
                let _ = write!(out, "{:>width$}", ".", width = col_width);
            }
        }
        let _ = writeln!(out);
    }
}

/// Format a transition matrix with ANSI colors.
fn format_matrix_pretty(out: &mut String, matrix: &TransitionMatrix) {
    use nu_ansi_term::Color::{Cyan, Yellow};

    if matrix.states.is_empty() {
        let _ = writeln!(out, "(no data)");
        return;
    }

    let max_label = matrix.states.iter().map(|s| s.len()).max().unwrap_or(5);
    let col_width = matrix
        .states
        .iter()
        .map(|s| s.len() + 1)
        .max()
        .unwrap_or(7)
        .max(7);

    // Header row
    let _ = write!(out, "{:width$}", "", width = max_label + 2);
    for state in &matrix.states {
        let _ = write!(
            out,
            "{:>width$}",
            Cyan.paint(state.as_str()),
            width = col_width
        );
    }
    let _ = writeln!(out);

    // Data rows
    for from in &matrix.states {
        let _ = write!(
            out,
            "{:width$}  ",
            Yellow.paint(from.as_str()),
            width = max_label
        );
        for to in &matrix.states {
            let prob = matrix
                .transitions
                .get(from)
                .and_then(|m| m.get(to))
                .copied()
                .unwrap_or(0.0);
            if prob > 0.001 {
                let _ = write!(out, "{:>width$.1}%", prob * 100.0, width = col_width - 1);
            } else {
                let _ = write!(out, "{:>width$}", ".", width = col_width);
            }
        }
        let _ = writeln!(out);
    }
}

/// Extract the tool call sequence from a session file.
/// Returns (tool_sequence, turn_count, first_user_message).
fn extract_tool_sequence(
    path: &Path,
    format_name: Option<&str>,
) -> Option<(Vec<String>, usize, String)> {
    let session = if let Some(fmt) = format_name {
        parse_session_with_format(path, fmt).ok()?
    } else {
        parse_session(path).ok()?
    };

    let mut tools = Vec::new();
    let mut first_user_msg = String::new();

    for turn in &session.turns {
        for msg in &turn.messages {
            // Capture first user message
            if msg.role == Role::User && first_user_msg.is_empty() {
                for block in &msg.content {
                    if let ContentBlock::Text { text } = block {
                        first_user_msg = text.chars().take(200).collect();
                        break;
                    }
                }
            }

            // Extract tool use names from assistant messages
            if msg.role == Role::Assistant {
                for block in &msg.content {
                    if let ContentBlock::ToolUse { name, .. } = block {
                        tools.push(name.clone());
                    }
                }
            }
        }
    }

    Some((tools, session.turns.len(), first_user_msg))
}

/// Build a transition count map from a tool sequence.
/// Includes "start" -> first tool and last tool -> "end".
fn build_transition_counts(tools: &[String]) -> HashMap<(String, String), usize> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();

    if tools.is_empty() {
        *counts
            .entry(("start".to_string(), "end".to_string()))
            .or_insert(0) += 1;
        return counts;
    }

    // start -> first tool
    *counts
        .entry(("start".to_string(), tools[0].clone()))
        .or_insert(0) += 1;

    // tool -> tool transitions
    for window in tools.windows(2) {
        *counts
            .entry((window[0].clone(), window[1].clone()))
            .or_insert(0) += 1;
    }

    // last tool -> end
    *counts
        .entry((tools[tools.len() - 1].clone(), "end".to_string()))
        .or_insert(0) += 1;

    counts
}

/// Normalize transition counts into probabilities.
fn normalize_counts(counts: &HashMap<(String, String), usize>) -> TransitionMatrix {
    // Collect all states
    let mut states_set = BTreeSet::new();
    for (from, to) in counts.keys() {
        states_set.insert(from.clone());
        states_set.insert(to.clone());
    }
    let states: Vec<String> = states_set.into_iter().collect();

    // Sum outgoing counts per state
    let mut row_totals: HashMap<String, usize> = HashMap::new();
    for ((from, _), count) in counts {
        *row_totals.entry(from.clone()).or_insert(0) += count;
    }

    // Build probability matrix
    let mut transitions: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for ((from, to), count) in counts {
        let total = row_totals[from] as f64;
        if total > 0.0 {
            transitions
                .entry(from.clone())
                .or_default()
                .insert(to.clone(), *count as f64 / total);
        }
    }

    TransitionMatrix {
        transitions,
        states,
    }
}

/// Compute the Frobenius norm of the difference between two transition matrices.
fn frobenius_divergence(a: &TransitionMatrix, b: &TransitionMatrix) -> f64 {
    // Union of all states from both matrices
    let mut all_states = BTreeSet::new();
    for s in &a.states {
        all_states.insert(s.clone());
    }
    for s in &b.states {
        all_states.insert(s.clone());
    }

    let mut sum_sq = 0.0;
    for from in &all_states {
        for to in &all_states {
            let va = a
                .transitions
                .get(from.as_str())
                .and_then(|m| m.get(to.as_str()))
                .copied()
                .unwrap_or(0.0);
            let vb = b
                .transitions
                .get(from.as_str())
                .and_then(|m| m.get(to.as_str()))
                .copied()
                .unwrap_or(0.0);
            let diff = va - vb;
            sum_sq += diff * diff;
        }
    }

    sum_sq.sqrt()
}

/// Build the patterns report from filtered sessions.
#[allow(clippy::too_many_arguments)]
pub fn build_patterns_report(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    mode: &SessionMode,
    agent_type: Option<&str>,
) -> Result<PatternsReport, String> {
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
        let project = project_filter.or(root);
        list_sessions_by_mode(format, project, mode)
    };

    // Date filters
    let now = std::time::SystemTime::now();
    let since_time = if let Some(d) = days {
        Some(now - std::time::Duration::from_secs(d as u64 * 86400))
    } else if let Some(s) = since {
        Some(parse_date(s).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", s))?)
    } else {
        None
    };
    let until_time = if let Some(u) = until {
        Some(
            parse_date(u).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", u))?
                + std::time::Duration::from_secs(86400),
        )
    } else {
        None
    };

    if let Some(since) = since_time {
        sessions.retain(|s| s.mtime >= since);
    }
    if let Some(until) = until_time {
        sessions.retain(|s| s.mtime <= until);
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

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    let total_before_limit = sessions.len();
    if limit > 0 {
        sessions.truncate(limit);
    }
    let truncated = super::TruncationInfo::if_truncated(total_before_limit, limit);

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    // Extract tool sequences from all sessions
    struct SessionData {
        path: String,
        tools: Vec<String>,
        turn_count: usize,
        first_user_message: String,
    }

    let mut session_data: Vec<SessionData> = Vec::new();
    let mut population_counts: HashMap<(String, String), usize> = HashMap::new();

    for sf in &sessions {
        if let Some((tools, turn_count, first_msg)) = extract_tool_sequence(&sf.path, format_name) {
            let counts = build_transition_counts(&tools);

            // Accumulate into population counts
            for ((from, to), count) in &counts {
                *population_counts
                    .entry((from.clone(), to.clone()))
                    .or_insert(0) += count;
            }

            session_data.push(SessionData {
                path: sf.path.to_string_lossy().to_string(),
                tools,
                turn_count,
                first_user_message: first_msg,
            });
        }
    }

    if session_data.is_empty() {
        return Err("No sessions could be parsed".to_string());
    }

    // Build population matrix
    let population_matrix = normalize_counts(&population_counts);

    // Build per-session matrices and compute divergence
    let mut per_session = Vec::new();
    let mut outlier_data = Vec::new();

    for sd in &session_data {
        let counts = build_transition_counts(&sd.tools);
        let session_matrix = normalize_counts(&counts);
        let divergence = frobenius_divergence(&population_matrix, &session_matrix);

        per_session.push(SessionTransitions {
            path: sd.path.clone(),
            matrix: session_matrix,
            divergence,
        });

        outlier_data.push(SessionPatternMeta {
            path: sd.path.clone(),
            turn_count: sd.turn_count,
            tool_count: sd.tools.len(),
            first_user_message: sd.first_user_message.clone(),
            divergence,
        });
    }

    // Sort outliers by divergence (highest first)
    outlier_data.sort_by(|a, b| {
        b.divergence
            .partial_cmp(&a.divergence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    outlier_data.truncate(10);

    // Extract common start/end tools from population matrix
    let common_start_tools: Vec<(String, f64)> = {
        let mut starts: Vec<(String, f64)> = population_matrix
            .transitions
            .get("start")
            .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default();
        starts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        starts.truncate(10);
        starts
    };

    let common_end_tools: Vec<(String, f64)> = {
        let mut ends: Vec<(String, f64)> = Vec::new();
        for (from, tos) in &population_matrix.transitions {
            if from == "start" || from == "end" {
                continue;
            }
            if let Some(prob) = tos.get("end") {
                ends.push((from.clone(), *prob));
            }
        }
        // These are per-state probabilities. We want the states most likely to transition to end.
        ends.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ends.truncate(10);
        ends
    };

    Ok(PatternsReport {
        session_count: session_data.len(),
        population_matrix,
        outliers: outlier_data,
        common_start_tools,
        common_end_tools,
        per_session,
        truncated,
    })
}
