//! Cost breakdown: per-turn token costs, cache savings, model-specific pricing.

use crate::output::OutputFormatter;
use crate::sessions::{
    FormatRegistry, LogFormat, ModelPricing, SessionFile, TokenUsage, parse_session,
    parse_session_with_format,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::stats::{list_all_project_sessions_by_mode, parse_date};
use super::{SessionMode, list_sessions_by_mode, session_matches_grep};

/// Cost for a single turn.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TurnCost {
    pub turn: usize,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: Option<f64>,
}

/// Report for `normalize sessions cost`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CostReport {
    pub session_path: PathBuf,
    pub turns: Vec<TurnCost>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_write_tokens: u64,
    /// Estimated total cost in USD (None if no model pricing available).
    pub total_cost_usd: Option<f64>,
    /// What the cost would have been without cache (None if no model pricing).
    pub cost_without_cache_usd: Option<f64>,
    /// Actual savings from cache reads.
    pub cache_savings_usd: Option<f64>,
    /// Cache efficiency: cache_read / (cache_read + input) as a percentage.
    pub cache_efficiency_pct: Option<f64>,
    /// Models seen in this session.
    pub models: Vec<String>,
}

impl CostReport {
    fn compute_totals(&mut self) {
        self.total_input_tokens = self.turns.iter().map(|t| t.input_tokens).sum();
        self.total_output_tokens = self.turns.iter().map(|t| t.output_tokens).sum();
        self.total_cache_read_tokens = self.turns.iter().map(|t| t.cache_read_tokens).sum();
        self.total_cache_write_tokens = self.turns.iter().map(|t| t.cache_write_tokens).sum();

        let total_cost: f64 = self.turns.iter().filter_map(|t| t.cost_usd).sum();
        if self.turns.iter().any(|t| t.cost_usd.is_some()) {
            self.total_cost_usd = Some(total_cost);

            // Compute what cost would have been without cache reads.
            // Cache reads would have been regular input tokens.
            // Find the dominant pricing for the session.
            let dominant_model = self
                .turns
                .iter()
                .filter_map(|t| t.model.as_deref())
                .filter_map(ModelPricing::from_model_str)
                .next();

            if let Some(pricing) = dominant_model {
                let cache_read_as_input =
                    (self.total_cache_read_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok;
                let actual_cache_read_cost = (self.total_cache_read_tokens as f64 / 1_000_000.0)
                    * pricing.cache_read_per_mtok;
                let savings = cache_read_as_input - actual_cache_read_cost;
                self.cache_savings_usd = Some(savings);
                self.cost_without_cache_usd = Some(total_cost + savings);
            }
        }

        let total_context = self.total_input_tokens + self.total_cache_read_tokens;
        if total_context > 0 {
            self.cache_efficiency_pct =
                Some(self.total_cache_read_tokens as f64 / total_context as f64 * 100.0);
        }
    }
}

impl OutputFormatter for CostReport {
    fn format_text(&self) -> String {
        let mut out = String::new();

        if !self.models.is_empty() {
            writeln!(out, "Models: {}", self.models.join(", ")).unwrap();
        }
        writeln!(out).unwrap();

        // Per-turn table
        if !self.turns.is_empty() {
            writeln!(
                out,
                "  {:<5} {:<8} {:<8} {:<8} {:<8}  {:<10}  model",
                "turn", "input", "output", "cache_r", "cache_w", "cost_usd"
            )
            .unwrap();
            writeln!(out, "  {}", "-".repeat(70)).unwrap();
            for t in &self.turns {
                let cost_str = t
                    .cost_usd
                    .map(|c| format!("${:.4}", c))
                    .unwrap_or_else(|| "-".into());
                let model_str = t.model.as_deref().unwrap_or("-");
                writeln!(
                    out,
                    "  {:<5} {:<8} {:<8} {:<8} {:<8}  {:<10}  {}",
                    t.turn,
                    fmt_tokens(t.input_tokens),
                    fmt_tokens(t.output_tokens),
                    fmt_tokens(t.cache_read_tokens),
                    fmt_tokens(t.cache_write_tokens),
                    cost_str,
                    model_str,
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        // Summary
        writeln!(out, "Summary:").unwrap();
        writeln!(
            out,
            "  Input tokens:        {}",
            fmt_tokens(self.total_input_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Output tokens:       {}",
            fmt_tokens(self.total_output_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Cache read tokens:   {}",
            fmt_tokens(self.total_cache_read_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Cache write tokens:  {}",
            fmt_tokens(self.total_cache_write_tokens)
        )
        .unwrap();
        if let Some(c) = self.total_cost_usd {
            writeln!(out, "  Total cost:          ${:.4}", c).unwrap();
        } else {
            writeln!(out, "  Total cost:          (unknown model)").unwrap();
        }
        if let Some(wc) = self.cost_without_cache_usd {
            writeln!(out, "  Without cache:       ${:.4}", wc).unwrap();
        }
        if let Some(s) = self.cache_savings_usd {
            writeln!(out, "  Cache savings:       ${:.4}", s).unwrap();
        }
        if let Some(e) = self.cache_efficiency_pct {
            writeln!(out, "  Cache efficiency:    {:.1}%", e).unwrap();
        }
        out
    }

    fn format_pretty(&self) -> String {
        let mut out = String::new();

        if !self.models.is_empty() {
            writeln!(out, "\x1b[1mModels:\x1b[0m {}", self.models.join(", ")).unwrap();
        }
        writeln!(out).unwrap();

        if !self.turns.is_empty() {
            writeln!(
                out,
                "  \x1b[2m{:<5} {:<8} {:<8} {:<8} {:<8}  {:<10}  model\x1b[0m",
                "turn", "input", "output", "cache_r", "cache_w", "cost_usd"
            )
            .unwrap();
            writeln!(out, "  \x1b[2m{}\x1b[0m", "-".repeat(70)).unwrap();
            for t in &self.turns {
                let cost_str = t
                    .cost_usd
                    .map(|c| format!("\x1b[32m${:.4}\x1b[0m", c))
                    .unwrap_or_else(|| "\x1b[2m-\x1b[0m".into());
                let model_str = t.model.as_deref().unwrap_or("-");
                writeln!(
                    out,
                    "  {:<5} \x1b[36m{:<8}\x1b[0m {:<8} \x1b[33m{:<8}\x1b[0m {:<8}  {}  {}",
                    t.turn,
                    fmt_tokens(t.input_tokens),
                    fmt_tokens(t.output_tokens),
                    fmt_tokens(t.cache_read_tokens),
                    fmt_tokens(t.cache_write_tokens),
                    cost_str,
                    model_str,
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        writeln!(out, "\x1b[1mSummary:\x1b[0m").unwrap();
        writeln!(
            out,
            "  Input tokens:        \x1b[36m{}\x1b[0m",
            fmt_tokens(self.total_input_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Output tokens:       {}",
            fmt_tokens(self.total_output_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Cache read tokens:   \x1b[33m{}\x1b[0m",
            fmt_tokens(self.total_cache_read_tokens)
        )
        .unwrap();
        writeln!(
            out,
            "  Cache write tokens:  {}",
            fmt_tokens(self.total_cache_write_tokens)
        )
        .unwrap();
        if let Some(c) = self.total_cost_usd {
            writeln!(out, "  Total cost:          \x1b[32m${:.4}\x1b[0m", c).unwrap();
        } else {
            writeln!(out, "  Total cost:          \x1b[2m(unknown model)\x1b[0m").unwrap();
        }
        if let Some(wc) = self.cost_without_cache_usd {
            writeln!(out, "  Without cache:       \x1b[31m${:.4}\x1b[0m", wc).unwrap();
        }
        if let Some(s) = self.cache_savings_usd {
            writeln!(out, "  Cache savings:       \x1b[32m${:.4}\x1b[0m", s).unwrap();
        }
        if let Some(e) = self.cache_efficiency_pct {
            writeln!(out, "  Cache efficiency:    \x1b[33m{:.1}%\x1b[0m", e).unwrap();
        }
        out
    }
}

fn fmt_tokens(n: u64) -> String {
    if n == 0 {
        "0".into()
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Build a CostReport from a list of (turn_index, TokenUsage) pairs.
fn build_report_from_turns(
    session_path: PathBuf,
    turn_usages: Vec<(usize, TokenUsage)>,
) -> CostReport {
    let mut report = CostReport {
        session_path,
        ..Default::default()
    };

    let mut seen_models: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (turn_idx, usage) in turn_usages {
        let pricing = usage
            .model
            .as_deref()
            .and_then(ModelPricing::from_model_str);
        let cost_usd = pricing.map(|p| p.calculate_turn_cost(&usage));

        if let Some(m) = &usage.model {
            seen_models.insert(m.clone());
        }

        report.turns.push(TurnCost {
            turn: turn_idx + 1,
            model: usage.model.clone(),
            input_tokens: usage.input,
            output_tokens: usage.output,
            cache_read_tokens: usage.cache_read.unwrap_or(0),
            cache_write_tokens: usage.cache_create.unwrap_or(0),
            cost_usd,
        });
    }

    report.models = {
        let mut v: Vec<String> = seen_models.into_iter().collect();
        v.sort();
        v
    };

    report.compute_totals();
    report
}

/// Collect turn usages from a session file.
fn collect_turn_usages(path: &Path, format_name: Option<&str>) -> Option<Vec<(usize, TokenUsage)>> {
    let session = if let Some(fmt) = format_name {
        parse_session_with_format(path, fmt).ok()?
    } else {
        parse_session(path).ok()?
    };

    let usages: Vec<(usize, TokenUsage)> = session
        .turns
        .into_iter()
        .enumerate()
        .filter_map(|(i, t)| t.token_usage.map(|u| (i, u)))
        .collect();

    Some(usages)
}

/// Build a cost report for a single session (by ID).
pub fn build_cost_report_for_session(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
    exact: bool,
) -> Result<CostReport, String> {
    use super::{resolve_session_paths, resolve_session_paths_literal};

    let paths = if exact {
        resolve_session_paths_literal(session_id, project, format_name)
    } else {
        resolve_session_paths(session_id, project, format_name)
    };

    if paths.is_empty() {
        return Err(format!("No sessions found matching: {}", session_id));
    }

    // For a single session, show per-turn breakdown.
    if paths.len() == 1 {
        let usages = collect_turn_usages(&paths[0], format_name).unwrap_or_default();
        return Ok(build_report_from_turns(paths[0].clone(), usages));
    }

    // Multiple sessions: aggregate (no per-turn detail, just totals).
    let mut all_usages: Vec<(usize, TokenUsage)> = Vec::new();
    let mut offset = 0;
    for path in &paths {
        if let Some(usages) = collect_turn_usages(path, format_name) {
            let n = usages.len();
            all_usages.extend(usages.into_iter().map(|(i, u)| (i + offset, u)));
            offset += n;
        }
    }
    Ok(build_report_from_turns(paths[0].clone(), all_usages))
}

/// Build a cost report across multiple filtered sessions (aggregate totals only).
#[allow(clippy::too_many_arguments)]
pub fn build_cost_report(
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
) -> Result<CostReport, String> {
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

    sessions.sort_by_key(|b| std::cmp::Reverse(b.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let mut all_usages: Vec<(usize, TokenUsage)> = Vec::new();
    let mut offset = 0;
    for sf in &sessions {
        if let Some(usages) = collect_turn_usages(&sf.path, format_name) {
            let n = usages.len();
            all_usages.extend(usages.into_iter().map(|(i, u)| (i + offset, u)));
            offset += n;
        }
    }

    Ok(build_report_from_turns(PathBuf::from("."), all_usages))
}
