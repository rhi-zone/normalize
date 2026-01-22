//! Session analysis types and functions.
//!
//! This module computes analytics from parsed Session data.
//! Analysis is intentionally in the CLI, not the parsing library,
//! because what metrics matter is subjective and consumer-specific.

use rhizome_moss_sessions::{ContentBlock, Session};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Statistics for a single tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolStats {
    pub name: String,
    pub calls: usize,
    pub errors: usize,
}

impl ToolStats {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            calls: 0,
            errors: 0,
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.calls == 0 {
            0.0
        } else {
            (self.calls - self.errors) as f64 / self.calls as f64
        }
    }
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStats {
    pub total_input: u64,
    pub total_output: u64,
    pub cache_read: u64,
    pub cache_create: u64,
    pub min_context: u64,
    pub max_context: u64,
    pub api_calls: usize,
}

impl TokenStats {
    pub fn avg_context(&self) -> u64 {
        if self.api_calls == 0 {
            0
        } else {
            (self.total_input + self.cache_read) / self.api_calls as u64
        }
    }

    pub fn update_context(&mut self, context_size: u64) {
        if self.min_context == 0 || context_size < self.min_context {
            self.min_context = context_size;
        }
        if context_size > self.max_context {
            self.max_context = context_size;
        }
    }
}

/// A recurring error pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub category: String,
    pub count: usize,
    pub examples: Vec<String>,
}

impl ErrorPattern {
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            count: 0,
            examples: Vec::new(),
        }
    }
}

/// A sequence of consecutive single-tool calls (potential parallelization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChain {
    pub tools: Vec<String>,
    pub turn_range: (usize, usize),
}

impl ToolChain {
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

/// Type of correction made by the assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionType {
    Apology,
    Mistake,
    LetMeFix,
    Actually,
}

impl CorrectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CorrectionType::Apology => "Apology",
            CorrectionType::Mistake => "Mistake",
            CorrectionType::LetMeFix => "Let me fix",
            CorrectionType::Actually => "Actually",
        }
    }
}

/// An assistant correction or apology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub turn: usize,
    pub text: String,
    pub category: CorrectionType,
}

/// Complete analysis of a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionAnalysis {
    pub session_path: PathBuf,
    pub format: String,
    pub message_counts: HashMap<String, usize>,
    pub tool_stats: HashMap<String, ToolStats>,
    pub token_stats: TokenStats,
    pub error_patterns: Vec<ErrorPattern>,
    /// Token usage per file/symbol path
    pub file_tokens: HashMap<String, u64>,
    /// Turns with single tool call (parallelization opportunity)
    pub parallel_opportunities: usize,
    pub total_turns: usize,
    /// Sequences of consecutive single-tool calls
    pub tool_chains: Vec<ToolChain>,
    /// Assistant corrections and apologies
    pub corrections: Vec<Correction>,
}

impl SessionAnalysis {
    pub fn new(session_path: PathBuf, format: impl Into<String>) -> Self {
        Self {
            session_path,
            format: format.into(),
            ..Default::default()
        }
    }

    pub fn total_tool_calls(&self) -> usize {
        self.tool_stats.values().map(|t| t.calls).sum()
    }

    pub fn total_errors(&self) -> usize {
        self.tool_stats.values().map(|t| t.errors).sum()
    }

    pub fn overall_success_rate(&self) -> f64 {
        let total = self.total_tool_calls();
        if total == 0 {
            0.0
        } else {
            (total - self.total_errors()) as f64 / total as f64
        }
    }

    /// Format as markdown report.
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            "# Session Analysis".to_string(),
            String::new(),
            "## Summary".to_string(),
            String::new(),
            format!("- **Format**: {}", self.format),
            format!("- **Tool calls**: {}", self.total_tool_calls()),
            format!(
                "- **Success rate**: {:.1}%",
                self.overall_success_rate() * 100.0
            ),
            format!("- **Total turns**: {}", self.total_turns),
            format!(
                "- **Parallel opportunities**: {}",
                self.parallel_opportunities
            ),
            String::new(),
        ];

        // Message types
        if !self.message_counts.is_empty() {
            lines.push("## Message Types".to_string());
            lines.push(String::new());
            lines.push("| Type | Count |".to_string());
            lines.push("|------|-------|".to_string());
            let mut counts: Vec<_> = self.message_counts.iter().collect();
            counts.sort_by(|a, b| b.1.cmp(a.1));
            for (msg_type, count) in counts {
                lines.push(format!("| {} | {} |", msg_type, count));
            }
            lines.push(String::new());
        }

        // Tool usage
        if !self.tool_stats.is_empty() {
            lines.push("## Tool Usage".to_string());
            lines.push(String::new());
            lines.push("| Tool | Calls | Errors | Success Rate |".to_string());
            lines.push("|------|-------|--------|--------------|".to_string());
            let mut tools: Vec<_> = self.tool_stats.values().collect();
            tools.sort_by(|a, b| b.calls.cmp(&a.calls));
            for tool in tools {
                lines.push(format!(
                    "| {} | {} | {} | {:.0}% |",
                    tool.name,
                    tool.calls,
                    tool.errors,
                    tool.success_rate() * 100.0
                ));
            }
            lines.push(String::new());
        }

        // Token usage
        if self.token_stats.api_calls > 0 {
            let ts = &self.token_stats;
            lines.push("## Token Usage".to_string());
            lines.push(String::new());
            lines.push(format!("- **API calls**: {}", ts.api_calls));
            lines.push(format!("- **Input tokens**: {}", ts.total_input));
            lines.push(format!("- **Output tokens**: {}", ts.total_output));
            lines.push(format!(
                "- **Total tokens**: {}",
                ts.total_input + ts.total_output
            ));
            if ts.cache_read > 0 {
                lines.push(format!("- **Cache read**: {} tokens", ts.cache_read));
            }
            if ts.cache_create > 0 {
                lines.push(format!("- **Cache create**: {} tokens", ts.cache_create));
            }
            lines.push(format!("- **Avg context**: {} tokens", ts.avg_context()));
            lines.push(format!(
                "- **Context range**: {} - {}",
                ts.min_context, ts.max_context
            ));
            lines.push(String::new());
        }

        // Token hotspots
        if !self.file_tokens.is_empty() {
            lines.push("## Token Hotspots".to_string());
            lines.push(String::new());
            lines.push("| Path | Tokens |".to_string());
            lines.push("|------|--------|".to_string());
            let mut paths: Vec<_> = self.file_tokens.iter().collect();
            paths.sort_by(|a, b| b.1.cmp(a.1));
            for (path, tokens) in paths.iter().take(10) {
                lines.push(format!("| {} | {} |", path, tokens));
            }
            lines.push(String::new());
        }

        // Tool chains
        if !self.tool_chains.is_empty() {
            lines.push("## Tool Chains".to_string());
            lines.push(String::new());
            lines.push(
                "Sequences of consecutive single-tool calls (potential parallelization):"
                    .to_string(),
            );
            lines.push(String::new());
            for chain in &self.tool_chains {
                let tools_str = chain.tools.join(" → ");
                lines.push(format!(
                    "- **Turns {}-{}** ({} tools): {}",
                    chain.turn_range.0,
                    chain.turn_range.1,
                    chain.len(),
                    tools_str
                ));
            }
            lines.push(String::new());
        }

        // Corrections
        if !self.corrections.is_empty() {
            lines.push("## Corrections & Apologies".to_string());
            lines.push(String::new());
            for correction in &self.corrections {
                lines.push(format!(
                    "- **Turn {}** [{}]: {}",
                    correction.turn,
                    correction.category.as_str(),
                    correction.text
                ));
            }
            lines.push(String::new());
        }

        // Error patterns
        if !self.error_patterns.is_empty() {
            lines.push("## Error Patterns".to_string());
            lines.push(String::new());
            for pattern in &self.error_patterns {
                lines.push(format!("### {} ({})", pattern.category, pattern.count));
                for ex in &pattern.examples {
                    lines.push(format!("- {}", ex));
                }
                lines.push(String::new());
            }
        }

        lines.join("\n")
    }

    /// Format as pretty output with bar charts.
    pub fn to_pretty(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        // Header
        writeln!(out, "\x1b[1;36m━━━ Session Analysis ━━━\x1b[0m").unwrap();
        writeln!(out).unwrap();

        // Summary
        writeln!(out, "\x1b[1mFormat:\x1b[0m {}", self.format).unwrap();
        writeln!(
            out,
            "\x1b[1mTool calls:\x1b[0m {} ({:.1}% success)",
            self.total_tool_calls(),
            self.overall_success_rate() * 100.0
        )
        .unwrap();
        writeln!(out, "\x1b[1mTurns:\x1b[0m {}", self.total_turns).unwrap();
        if self.parallel_opportunities > 0 {
            writeln!(
                out,
                "\x1b[1mParallel opportunities:\x1b[0m {}",
                self.parallel_opportunities
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        // Tool usage with bar charts
        if !self.tool_stats.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ Tool Usage ━━━\x1b[0m").unwrap();

            let mut tools: Vec<_> = self.tool_stats.values().collect();
            tools.sort_by(|a, b| b.calls.cmp(&a.calls));

            let max_calls = tools.first().map(|t| t.calls).unwrap_or(1);
            let max_name_len = tools.iter().map(|t| t.name.len()).max().unwrap_or(10);

            for tool in tools {
                let bar_width = 30;
                let filled = (tool.calls as f64 / max_calls as f64 * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

                let color = if tool.errors > 0 {
                    "\x1b[31m"
                } else {
                    "\x1b[32m"
                };
                writeln!(
                    out,
                    "{:>width$} {} {}{:>5}\x1b[0m{}",
                    tool.name,
                    bar,
                    color,
                    tool.calls,
                    if tool.errors > 0 {
                        format!(" ({} errors)", tool.errors)
                    } else {
                        String::new()
                    },
                    width = max_name_len
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        // Token usage
        if self.token_stats.api_calls > 0 {
            let ts = &self.token_stats;
            writeln!(out, "\x1b[1;36m━━━ Token Usage ━━━\x1b[0m").unwrap();
            writeln!(out, "API calls: {}", ts.api_calls).unwrap();
            writeln!(out, "Avg context: {} tokens", ts.avg_context()).unwrap();
            writeln!(
                out,
                "Context range: {} - {}",
                ts.min_context, ts.max_context
            )
            .unwrap();
            if ts.cache_read > 0 {
                writeln!(out, "Cache read: {} tokens", format_tokens(ts.cache_read)).unwrap();
            }
            if ts.cache_create > 0 {
                writeln!(
                    out,
                    "Cache create: {} tokens",
                    format_tokens(ts.cache_create)
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        // Token hotspots
        if !self.file_tokens.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ Token Hotspots ━━━\x1b[0m").unwrap();
            let mut paths: Vec<_> = self.file_tokens.iter().collect();
            paths.sort_by(|a, b| b.1.cmp(a.1));

            let max_tokens = paths.first().map(|(_, t)| **t).unwrap_or(1);

            for (path, tokens) in paths.iter().take(10) {
                let bar_width = 20;
                let filled = (**tokens as f64 / max_tokens as f64 * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
                writeln!(out, "{} {:>8} {}", bar, format_tokens(**tokens), path).unwrap();
            }
            writeln!(out).unwrap();
        }

        // Message types (compact)
        if !self.message_counts.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ Message Types ━━━\x1b[0m").unwrap();
            let mut counts: Vec<_> = self.message_counts.iter().collect();
            counts.sort_by(|a, b| b.1.cmp(a.1));

            let items: Vec<String> = counts
                .iter()
                .take(8)
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect();
            writeln!(out, "{}", items.join("  ")).unwrap();
        }

        // Tool chains
        if !self.tool_chains.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "\x1b[1;36m━━━ Tool Chains ━━━\x1b[0m").unwrap();
            writeln!(
                out,
                "Found {} sequences of consecutive single-tool calls:",
                self.tool_chains.len()
            )
            .unwrap();
            for chain in self.tool_chains.iter().take(10) {
                let tools_str = chain.tools.join(" → ");
                writeln!(
                    out,
                    "\x1b[33m▸\x1b[0m Turns {}-{} ({}): {}",
                    chain.turn_range.0,
                    chain.turn_range.1,
                    chain.len(),
                    tools_str
                )
                .unwrap();
            }
        }

        // Corrections
        if !self.corrections.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "\x1b[1;36m━━━ Corrections & Apologies ━━━\x1b[0m").unwrap();
            for correction in &self.corrections {
                writeln!(
                    out,
                    "\x1b[31m⚠\x1b[0m Turn {} [{}]: {}",
                    correction.turn,
                    correction.category.as_str(),
                    correction.text.chars().take(60).collect::<String>()
                )
                .unwrap();
            }
        }

        out
    }
}

/// Format token count with K/M suffix.
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Categorize an error by its content.
pub fn categorize_error(error_text: &str) -> &'static str {
    let text = error_text.to_lowercase();
    if text.contains("exit code") {
        "Command failure"
    } else if text.contains("not found") {
        "File not found"
    } else if text.contains("permission") {
        "Permission error"
    } else if text.contains("timeout") {
        "Timeout"
    } else if text.contains("syntax") {
        "Syntax error"
    } else if text.contains("import") {
        "Import error"
    } else {
        "Other"
    }
}

/// Detect correction patterns in assistant text.
/// Returns (category, excerpt) if a correction is found.
pub fn detect_correction(text: &str) -> Option<(CorrectionType, String)> {
    let lower = text.to_lowercase();

    // Look for apology patterns
    let apology_phrases = ["i apologize", "i'm sorry", "sorry about", "my apologies"];
    for phrase in &apology_phrases {
        if let Some(pos) = lower.find(phrase) {
            let excerpt = text.chars().skip(pos).take(80).collect();
            return Some((CorrectionType::Apology, excerpt));
        }
    }

    // Look for mistake acknowledgment
    let mistake_phrases = [
        "i made a mistake",
        "i was wrong",
        "that was incorrect",
        "my mistake",
    ];
    for phrase in &mistake_phrases {
        if let Some(pos) = lower.find(phrase) {
            let excerpt = text.chars().skip(pos).take(80).collect();
            return Some((CorrectionType::Mistake, excerpt));
        }
    }

    // Look for "let me fix" patterns
    let fix_phrases = ["let me fix", "i'll fix", "let me correct"];
    for phrase in &fix_phrases {
        if let Some(pos) = lower.find(phrase) {
            let excerpt = text.chars().skip(pos).take(80).collect();
            return Some((CorrectionType::LetMeFix, excerpt));
        }
    }

    // Look for "actually" corrections
    let actually_phrases = ["actually,", "actually i", "actually that"];
    for phrase in &actually_phrases {
        if let Some(pos) = lower.find(phrase) {
            let excerpt = text.chars().skip(pos).take(80).collect();
            return Some((CorrectionType::Actually, excerpt));
        }
    }

    None
}

/// Normalize a file path for aggregation.
pub fn normalize_path(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }
    // Find common project markers and make relative
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if matches!(
            *part,
            "src" | "lib" | "crates" | "tests" | "docs" | "packages"
        ) {
            return parts[i..].join("/");
        }
    }
    path.to_string()
}

/// Analyze a parsed session and compute statistics.
pub fn analyze_session(session: &Session) -> SessionAnalysis {
    let mut analysis = SessionAnalysis::new(session.path.clone(), &session.format);

    // Count message types by role
    for turn in &session.turns {
        for msg in &turn.messages {
            let role_str = msg.role.to_string();
            *analysis.message_counts.entry(role_str).or_insert(0) += 1;
        }
    }

    // Analyze tool usage and detect tool chains
    let mut current_chain: Option<Vec<(usize, String)>> = None;

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        let mut tool_uses_in_turn = 0;
        let mut tool_name_in_turn: Option<String> = None;

        for msg in &turn.messages {
            // Detect corrections in assistant messages
            if msg.role == rhizome_moss_sessions::Role::Assistant {
                for block in &msg.content {
                    if let ContentBlock::Text { text } = block {
                        if let Some((category, excerpt)) = detect_correction(text) {
                            analysis.corrections.push(Correction {
                                turn: turn_idx,
                                text: excerpt,
                                category,
                            });
                        }
                    }
                }
            }

            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse { name, .. } => {
                        let stat = analysis
                            .tool_stats
                            .entry(name.clone())
                            .or_insert_with(|| ToolStats::new(name));
                        stat.calls += 1;
                        tool_uses_in_turn += 1;
                        tool_name_in_turn = Some(name.clone());
                    }
                    ContentBlock::ToolResult {
                        is_error, content, ..
                    } => {
                        if *is_error {
                            // Try to attribute error to most recent tool
                            // For now, just track in error patterns
                            let category = categorize_error(content);
                            let pattern = analysis
                                .error_patterns
                                .iter_mut()
                                .find(|p| p.category == category);

                            if let Some(p) = pattern {
                                p.count += 1;
                                if p.examples.len() < 3 {
                                    p.examples.push(content.chars().take(100).collect());
                                }
                            } else {
                                let mut p = ErrorPattern::new(category);
                                p.count = 1;
                                p.examples.push(content.chars().take(100).collect());
                                analysis.error_patterns.push(p);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Track parallel opportunities (turns with single tool call)
        if tool_uses_in_turn == 1 {
            analysis.parallel_opportunities += 1;

            // Build tool chains
            if let Some(tool_name) = tool_name_in_turn {
                match &mut current_chain {
                    Some(chain) => {
                        chain.push((turn_idx, tool_name));
                    }
                    None => {
                        current_chain = Some(vec![(turn_idx, tool_name)]);
                    }
                }
            }
        } else {
            // Chain broken - save if length >= 3
            if let Some(chain) = current_chain.take() {
                if chain.len() >= 3 {
                    let tools: Vec<String> = chain.iter().map(|(_, name)| name.clone()).collect();
                    let turn_range = (chain.first().unwrap().0, chain.last().unwrap().0);
                    analysis.tool_chains.push(ToolChain { tools, turn_range });
                }
            }
        }
    }

    // Handle final chain
    if let Some(chain) = current_chain {
        if chain.len() >= 3 {
            let tools: Vec<String> = chain.iter().map(|(_, name)| name.clone()).collect();
            let turn_range = (chain.first().unwrap().0, chain.last().unwrap().0);
            analysis.tool_chains.push(ToolChain { tools, turn_range });
        }
    }

    // Analyze token usage
    analysis.total_turns = session.turns.len();
    for turn in &session.turns {
        if let Some(usage) = &turn.token_usage {
            analysis.token_stats.api_calls += 1;
            analysis.token_stats.total_input += usage.input;
            analysis.token_stats.total_output += usage.output;
            if let Some(cr) = usage.cache_read {
                analysis.token_stats.cache_read += cr;
            }
            if let Some(cc) = usage.cache_create {
                analysis.token_stats.cache_create += cc;
            }

            let context = usage.input + usage.cache_read.unwrap_or(0);
            analysis.token_stats.update_context(context);
        }
    }

    // Sort error patterns by count
    analysis
        .error_patterns
        .sort_by(|a, b| b.count.cmp(&a.count));

    analysis
}
