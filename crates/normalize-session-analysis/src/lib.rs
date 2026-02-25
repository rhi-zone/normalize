//! Session analysis types and functions.
//!
//! This module computes analytics from parsed Session data.
//! Analysis is intentionally in the CLI, not the parsing library,
//! because what metrics matter is subjective and consumer-specific.

use normalize_chat_sessions::{ContentBlock, Session};
use normalize_output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Statistics for a single tool.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct TokenStats {
    pub total_input: u64,
    pub total_output: u64,
    pub cache_read: u64,
    pub cache_create: u64,
    pub min_context: u64,
    pub max_context: u64,
    pub api_calls: usize,
}

/// Model pricing information (per million tokens).
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub name: &'static str,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

impl ModelPricing {
    /// Anthropic Claude pricing (as of Feb 2026).
    pub const SONNET_4_5: ModelPricing = ModelPricing {
        name: "Claude Sonnet 4.5",
        input_per_mtok: 3.0,
        output_per_mtok: 15.0,
        cache_write_per_mtok: 3.75,
        cache_read_per_mtok: 0.30,
    };

    pub const OPUS_4_5: ModelPricing = ModelPricing {
        name: "Claude Opus 4.5/4.6",
        input_per_mtok: 5.0,
        output_per_mtok: 25.0,
        cache_write_per_mtok: 6.25,
        cache_read_per_mtok: 0.50,
    };

    pub const OPUS_3: ModelPricing = ModelPricing {
        name: "Claude Opus 3/4/4.1",
        input_per_mtok: 15.0,
        output_per_mtok: 75.0,
        cache_write_per_mtok: 18.75,
        cache_read_per_mtok: 1.50,
    };

    pub const HAIKU_4_5: ModelPricing = ModelPricing {
        name: "Claude Haiku 4.5",
        input_per_mtok: 1.0,
        output_per_mtok: 5.0,
        cache_write_per_mtok: 1.25,
        cache_read_per_mtok: 0.10,
    };

    pub const HAIKU_3_5: ModelPricing = ModelPricing {
        name: "Claude Haiku 3.5",
        input_per_mtok: 0.80,
        output_per_mtok: 4.0,
        cache_write_per_mtok: 1.0,
        cache_read_per_mtok: 0.08,
    };

    pub const HAIKU_3: ModelPricing = ModelPricing {
        name: "Claude Haiku 3",
        input_per_mtok: 0.25,
        output_per_mtok: 1.25,
        cache_write_per_mtok: 0.30,
        cache_read_per_mtok: 0.03,
    };

    /// Look up pricing from a model identifier string (e.g. "claude-opus-4-6").
    pub fn from_model_str(model: &str) -> Option<&'static ModelPricing> {
        let m = model.to_lowercase();
        if m.contains("opus") {
            if m.contains("4-5") || m.contains("4.5") || m.contains("4-6") || m.contains("4.6") {
                Some(&Self::OPUS_4_5)
            } else {
                Some(&Self::OPUS_3)
            }
        } else if m.contains("sonnet") {
            Some(&Self::SONNET_4_5)
        } else if m.contains("haiku") {
            if m.contains("4") {
                Some(&Self::HAIKU_4_5)
            } else if m.contains("3-5") || m.contains("3.5") {
                Some(&Self::HAIKU_3_5)
            } else {
                Some(&Self::HAIKU_3)
            }
        } else {
            None
        }
    }

    /// Calculate cost for a single turn's token usage.
    pub fn calculate_turn_cost(&self, usage: &normalize_chat_sessions::TokenUsage) -> f64 {
        let input_cost = (usage.input as f64 / 1_000_000.0) * self.input_per_mtok;
        let output_cost = (usage.output as f64 / 1_000_000.0) * self.output_per_mtok;
        let cache_write_cost =
            (usage.cache_create.unwrap_or(0) as f64 / 1_000_000.0) * self.cache_write_per_mtok;
        let cache_read_cost =
            (usage.cache_read.unwrap_or(0) as f64 / 1_000_000.0) * self.cache_read_per_mtok;
        input_cost + output_cost + cache_write_cost + cache_read_cost
    }

    /// Calculate cost for given token usage.
    pub fn calculate_cost(&self, stats: &TokenStats) -> CostBreakdown {
        let input_cost = (stats.total_input as f64 / 1_000_000.0) * self.input_per_mtok;
        let output_cost = (stats.total_output as f64 / 1_000_000.0) * self.output_per_mtok;
        let cache_write_cost =
            (stats.cache_create as f64 / 1_000_000.0) * self.cache_write_per_mtok;
        let cache_read_cost = (stats.cache_read as f64 / 1_000_000.0) * self.cache_read_per_mtok;

        // Cache savings = what we would have paid without cache
        let without_cache_input = stats.total_input + stats.cache_read;
        let without_cache_cost = (without_cache_input as f64 / 1_000_000.0) * self.input_per_mtok;
        let with_cache_cost = input_cost + cache_read_cost;
        let cache_savings = without_cache_cost - with_cache_cost;

        CostBreakdown {
            model: self.name,
            input_cost,
            output_cost,
            cache_write_cost,
            cache_read_cost,
            total_cost: input_cost + output_cost + cache_write_cost + cache_read_cost,
            cache_savings,
        }
    }
}

/// Cost breakdown for a session.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, Deserialize)]
pub struct CostBreakdown {
    pub model: &'static str,
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_write_cost: f64,
    pub cache_read_cost: f64,
    pub total_cost: f64,
    pub cache_savings: f64,
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
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, Deserialize)]
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
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, Deserialize)]
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

    /// Estimate potential API call savings if parallelized.
    pub fn potential_savings(&self) -> usize {
        if self.len() <= 1 { 0 } else { self.len() - 1 }
    }

    /// Check if chain contains only read-like operations (safe to parallelize).
    pub fn is_safe_parallel(&self) -> bool {
        self.tools.iter().all(|tool| {
            matches!(
                tool.as_str(),
                "Read" | "Glob" | "Grep" | "Bash" | "Task" | "WebFetch" | "WebSearch"
            )
        })
    }
}

/// Type of correction made by the assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema, Deserialize)]
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
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, Deserialize)]
pub struct Correction {
    pub turn: usize,
    pub text: String,
    pub category: CorrectionType,
}

/// File operation statistics.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct FileOperation {
    pub path: String,
    pub reads: usize,
    pub edits: usize,
    pub writes: usize,
}

impl FileOperation {
    pub fn total(&self) -> usize {
        self.reads + self.edits + self.writes
    }
}

/// Statistics for a command category (e.g., "build", "test", "git").
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct CommandStats {
    pub category: String,
    pub commands: Vec<CommandDetail>,
    pub total_calls: usize,
    pub total_errors: usize,
    /// Sum of output tokens for turns containing this category.
    pub output_tokens: u64,
}

/// Detail for a specific command pattern within a category.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct CommandDetail {
    pub pattern: String,
    pub calls: usize,
    pub errors: usize,
}

/// A command pattern that failed and was retried.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct RetryHotspot {
    pub pattern: String,
    pub attempts: usize,
    pub failures: usize,
    pub output_tokens: u64,
    pub turn_indices: Vec<usize>,
}

/// A common tool pattern across sessions.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, Deserialize)]
pub struct ToolPattern {
    pub tools: Vec<String>,
    pub occurrences: usize,
}

impl ToolPattern {
    pub fn pattern_str(&self) -> String {
        self.tools.join(" → ")
    }
}

/// Token deduplication statistics.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
pub struct DedupTokenStats {
    /// Input tokens representing genuinely new content.
    pub unique_input: u64,
    /// Output tokens (always unique).
    pub unique_output: u64,
    /// Total billed tokens (input + output).
    pub total_billed: u64,
    /// Ratio of unique tokens to total billed (0.0-1.0).
    pub uniqueness_ratio: f64,
}

/// Complete analysis of a session.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema, Deserialize)]
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
    /// Context size per turn (input + cache_read)
    pub context_per_turn: Vec<u64>,
    /// File operation frequency (Read/Edit/Write)
    pub file_operations: HashMap<String, FileOperation>,
    /// Common tool patterns (multi-session aggregate only)
    pub tool_patterns: Vec<ToolPattern>,
    /// Bash command statistics by category
    pub command_stats: Vec<CommandStats>,
    /// Commands that failed and were retried
    pub retry_hotspots: Vec<RetryHotspot>,
    /// Actual cost computed from per-turn model pricing (None if no models found).
    pub actual_cost: Option<f64>,
    /// Token deduplication statistics.
    pub dedup_tokens: Option<DedupTokenStats>,
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

    /// Format as compact text (markdown, LLM-friendly, no colors).
    pub fn format_text(&self) -> String {
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

            // Cost breakdown
            lines.push("## Cost Estimate".to_string());
            lines.push(String::new());

            if let Some(actual) = self.actual_cost {
                lines.push(format!("**Actual cost**: ${:.2}", actual));
                lines.push(String::new());

                let sonnet = ModelPricing::SONNET_4_5.calculate_cost(ts);
                let opus = ModelPricing::OPUS_4_5.calculate_cost(ts);
                let haiku = ModelPricing::HAIKU_4_5.calculate_cost(ts);
                lines.push("**What-if pricing:**".to_string());
                lines.push(format!("  - {}: ${:.2}", sonnet.model, sonnet.total_cost));
                lines.push(format!("  - {}: ${:.2}", opus.model, opus.total_cost));
                lines.push(format!("  - {}: ${:.2}", haiku.model, haiku.total_cost));
            } else {
                let sonnet = ModelPricing::SONNET_4_5.calculate_cost(ts);
                lines.push(format!(
                    "**{} (default)**: ${:.2}",
                    sonnet.model, sonnet.total_cost
                ));
                lines.push(format!("  - Input: ${:.2}", sonnet.input_cost));
                lines.push(format!("  - Output: ${:.2}", sonnet.output_cost));
                if sonnet.cache_write_cost > 0.0 {
                    lines.push(format!("  - Cache write: ${:.2}", sonnet.cache_write_cost));
                }
                if sonnet.cache_read_cost > 0.0 {
                    lines.push(format!("  - Cache read: ${:.2}", sonnet.cache_read_cost));
                }
                if sonnet.cache_savings > 0.0 {
                    let savings_pct =
                        (sonnet.cache_savings / (sonnet.total_cost + sonnet.cache_savings)) * 100.0;
                    lines.push(format!(
                        "  - Cache savings: ${:.2} ({:.1}%)",
                        sonnet.cache_savings, savings_pct
                    ));
                }
                lines.push(String::new());

                let opus = ModelPricing::OPUS_4_5.calculate_cost(ts);
                let haiku = ModelPricing::HAIKU_4_5.calculate_cost(ts);
                lines.push("**Alternative models:**".to_string());
                lines.push(format!(
                    "  - {}: ${:.2} ({:.1}x)",
                    opus.model,
                    opus.total_cost,
                    opus.total_cost / sonnet.total_cost
                ));
                lines.push(format!(
                    "  - {}: ${:.2} ({:.1}x)",
                    haiku.model,
                    haiku.total_cost,
                    haiku.total_cost / sonnet.total_cost
                ));
            }
            lines.push(String::new());

            // Token efficiency
            if let Some(dedup) = &self.dedup_tokens {
                lines.push("## Token Efficiency".to_string());
                lines.push(String::new());
                lines.push(format!(
                    "- **Unique input**: {}",
                    format_tokens(dedup.unique_input)
                ));
                lines.push(format!(
                    "- **Unique output**: {}",
                    format_tokens(dedup.unique_output)
                ));
                lines.push(format!(
                    "- **Uniqueness ratio**: {:.1}%",
                    dedup.uniqueness_ratio * 100.0
                ));
                let redundant = dedup
                    .total_billed
                    .saturating_sub(dedup.unique_input + dedup.unique_output);
                lines.push(format!(
                    "- **Redundant context**: {}",
                    format_tokens(redundant)
                ));
                lines.push(String::new());
            }

            // Token growth
            if !self.context_per_turn.is_empty() && self.context_per_turn.iter().any(|&c| c > 0) {
                lines.push("## Context Growth".to_string());
                lines.push(String::new());

                // Show growth at key intervals
                let intervals = if self.context_per_turn.len() <= 10 {
                    (0..self.context_per_turn.len()).collect::<Vec<_>>()
                } else {
                    let step = self.context_per_turn.len() / 10;
                    (0..10)
                        .map(|i| i * step)
                        .chain(std::iter::once(self.context_per_turn.len() - 1))
                        .collect()
                };

                for idx in intervals {
                    if idx < self.context_per_turn.len() {
                        let context = self.context_per_turn[idx];
                        if context > 0 {
                            let warning = if context >= 100_000 {
                                " ⚠️ APPROACHING LIMIT"
                            } else if context >= 80_000 {
                                " ⚠️ High"
                            } else {
                                ""
                            };
                            lines.push(format!(
                                "- Turn {}: {}{}",
                                idx,
                                format_tokens(context),
                                warning
                            ));
                        }
                    }
                }
                lines.push(String::new());
            }
        }

        // Command breakdown
        if !self.command_stats.is_empty() {
            lines.push("## Command Breakdown".to_string());
            lines.push(String::new());
            lines.push("| Category | Calls | Errors | ~Output Tokens |".to_string());
            lines.push("|----------|-------|--------|----------------|".to_string());
            for stat in &self.command_stats {
                lines.push(format!(
                    "| {} | {} | {} | {} |",
                    stat.category,
                    stat.total_calls,
                    stat.total_errors,
                    format_tokens(stat.output_tokens)
                ));
            }
            lines.push(String::new());

            // Top commands across all categories
            let mut all_commands: Vec<&CommandDetail> = self
                .command_stats
                .iter()
                .flat_map(|s| &s.commands)
                .collect();
            all_commands.sort_by(|a, b| b.calls.cmp(&a.calls));
            if !all_commands.is_empty() {
                lines.push("Top commands:".to_string());
                for cmd in all_commands.iter().take(10) {
                    if cmd.errors > 0 {
                        lines.push(format!(
                            "- {}: {} calls ({} errors)",
                            cmd.pattern, cmd.calls, cmd.errors
                        ));
                    } else {
                        lines.push(format!("- {}: {} calls", cmd.pattern, cmd.calls));
                    }
                }
                lines.push(String::new());
            }
        }

        // Retry hotspots
        if !self.retry_hotspots.is_empty() {
            lines.push("## Retry Hotspots".to_string());
            lines.push(String::new());
            for hotspot in &self.retry_hotspots {
                lines.push(format!(
                    "- **{}** — {} failures / {} attempts, ~{} output tokens",
                    hotspot.pattern,
                    hotspot.failures,
                    hotspot.attempts,
                    format_tokens(hotspot.output_tokens)
                ));
            }
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

        // File operations heatmap
        if !self.file_operations.is_empty() {
            lines.push("## File Operations".to_string());
            lines.push(String::new());
            let mut ops: Vec<_> = self.file_operations.values().collect();
            ops.sort_by_key(|b| std::cmp::Reverse(b.total()));
            lines.push("| File | Reads | Edits | Writes | Total |".to_string());
            lines.push("|------|-------|-------|--------|-------|".to_string());
            for op in ops.iter().take(20) {
                lines.push(format!(
                    "| {} | {} | {} | {} | {} |",
                    op.path,
                    op.reads,
                    op.edits,
                    op.writes,
                    op.total()
                ));
            }
            lines.push(String::new());
        }

        // Parallelization hints
        if !self.tool_chains.is_empty() {
            let mut sorted_chains = self.tool_chains.clone();
            sorted_chains.sort_by_key(|b| std::cmp::Reverse(b.potential_savings()));

            let top_opportunities: Vec<_> = sorted_chains
                .iter()
                .filter(|c| c.potential_savings() >= 2)
                .take(5)
                .collect();

            if !top_opportunities.is_empty() {
                lines.push("## Parallelization Opportunities".to_string());
                lines.push(String::new());

                let total_savings: usize =
                    self.tool_chains.iter().map(|c| c.potential_savings()).sum();
                lines.push(format!(
                    "**Estimated savings**: {} API calls could be reduced by running tools in parallel",
                    total_savings
                ));
                lines.push(String::new());

                for chain in &top_opportunities {
                    let tools_str = chain.tools.join(" → ");
                    let safe_marker = if chain.is_safe_parallel() {
                        " ✓ Safe"
                    } else {
                        ""
                    };
                    lines.push(format!(
                        "- **Turns {}-{}**: {} API calls → 1 call (save {}){}",
                        chain.turn_range.0,
                        chain.turn_range.1,
                        chain.len(),
                        chain.potential_savings(),
                        safe_marker
                    ));
                    lines.push(format!("  Tools: {}", tools_str));
                }
                lines.push(String::new());
            }
        }

        // Tool patterns (multi-session only)
        if !self.tool_patterns.is_empty() {
            lines.push("## Common Tool Patterns".to_string());
            lines.push(String::new());
            lines.push("Frequent sequences across all sessions:".to_string());
            lines.push(String::new());
            for pattern in self.tool_patterns.iter().take(10) {
                lines.push(format!(
                    "- **{}×**: {}",
                    pattern.occurrences,
                    pattern.pattern_str()
                ));
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

    /// Format as pretty text with colors and bar charts.
    pub fn format_pretty(&self) -> String {
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

            // Cost breakdown
            writeln!(out, "\x1b[1;36m━━━ Cost Estimate ━━━\x1b[0m").unwrap();

            if let Some(actual) = self.actual_cost {
                writeln!(
                    out,
                    "\x1b[1mActual cost:\x1b[0m \x1b[32m${:.2}\x1b[0m",
                    actual
                )
                .unwrap();

                let sonnet = ModelPricing::SONNET_4_5.calculate_cost(ts);
                let opus = ModelPricing::OPUS_4_5.calculate_cost(ts);
                let haiku = ModelPricing::HAIKU_4_5.calculate_cost(ts);
                writeln!(
                    out,
                    "What-if: {} ${:.2} | {} ${:.2} | {} ${:.2}",
                    sonnet.model,
                    sonnet.total_cost,
                    opus.model,
                    opus.total_cost,
                    haiku.model,
                    haiku.total_cost
                )
                .unwrap();
            } else {
                let sonnet = ModelPricing::SONNET_4_5.calculate_cost(ts);
                writeln!(
                    out,
                    "\x1b[1m{}\x1b[0m: \x1b[32m${:.2}\x1b[0m",
                    sonnet.model, sonnet.total_cost
                )
                .unwrap();
                if sonnet.cache_savings > 0.0 {
                    let savings_pct =
                        (sonnet.cache_savings / (sonnet.total_cost + sonnet.cache_savings)) * 100.0;
                    writeln!(
                        out,
                        "  Cache savings: \x1b[33m${:.2}\x1b[0m ({:.1}%)",
                        sonnet.cache_savings, savings_pct
                    )
                    .unwrap();
                }
                writeln!(
                    out,
                    "  Input: ${:.2} | Output: ${:.2}",
                    sonnet.input_cost, sonnet.output_cost
                )
                .unwrap();

                let opus = ModelPricing::OPUS_4_5.calculate_cost(ts);
                let haiku = ModelPricing::HAIKU_4_5.calculate_cost(ts);
                writeln!(
                    out,
                    "If {}: ${:.2} (\x1b[31m{:.1}x\x1b[0m) | If {}: ${:.2} (\x1b[32m{:.1}x\x1b[0m)",
                    opus.model,
                    opus.total_cost,
                    opus.total_cost / sonnet.total_cost,
                    haiku.model,
                    haiku.total_cost,
                    haiku.total_cost / sonnet.total_cost
                )
                .unwrap();
            }

            // Token efficiency
            if let Some(dedup) = &self.dedup_tokens {
                writeln!(out).unwrap();
                writeln!(out, "\x1b[1;36m━━━ Token Efficiency ━━━\x1b[0m").unwrap();
                writeln!(
                    out,
                    "Unique input: {} | Unique output: {}",
                    format_tokens(dedup.unique_input),
                    format_tokens(dedup.unique_output)
                )
                .unwrap();
                writeln!(
                    out,
                    "Uniqueness: \x1b[33m{:.1}%\x1b[0m",
                    dedup.uniqueness_ratio * 100.0
                )
                .unwrap();
                let redundant = dedup
                    .total_billed
                    .saturating_sub(dedup.unique_input + dedup.unique_output);
                writeln!(out, "Redundant context: {}", format_tokens(redundant)).unwrap();
            }
            writeln!(out).unwrap();

            // Token growth visualization
            if !self.context_per_turn.is_empty() && self.context_per_turn.iter().any(|&c| c > 0) {
                writeln!(out, "\x1b[1;36m━━━ Context Growth ━━━\x1b[0m").unwrap();
                for line in token_growth_chart(&self.context_per_turn, 20) {
                    writeln!(out, "{}", line).unwrap();
                }
                writeln!(out).unwrap();
            }
        }

        // Command breakdown with bar charts
        if !self.command_stats.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ Command Breakdown ━━━\x1b[0m").unwrap();

            let max_calls = self
                .command_stats
                .first()
                .map(|s| s.total_calls)
                .unwrap_or(1);
            let max_cat_len = self
                .command_stats
                .iter()
                .map(|s| s.category.len())
                .max()
                .unwrap_or(8);

            for stat in &self.command_stats {
                let bar_width = 20;
                let filled =
                    (stat.total_calls as f64 / max_calls as f64 * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

                let error_str = if stat.total_errors > 0 {
                    format!(
                        " (\x1b[31m{} error{}\x1b[0m)",
                        stat.total_errors,
                        if stat.total_errors == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                };

                writeln!(
                    out,
                    "{:>width$}  {} {:>3} calls{} ~{}",
                    stat.category,
                    bar,
                    stat.total_calls,
                    error_str,
                    format_tokens(stat.output_tokens),
                    width = max_cat_len
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        // Retry hotspots
        if !self.retry_hotspots.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ Retry Hotspots ━━━\x1b[0m").unwrap();
            for hotspot in &self.retry_hotspots {
                writeln!(
                    out,
                    "\x1b[33m⚠\x1b[0m {} — {}/{} failed, ~{} output tokens burned",
                    hotspot.pattern,
                    hotspot.failures,
                    hotspot.attempts,
                    format_tokens(hotspot.output_tokens)
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }

        // File operations heatmap
        if !self.file_operations.is_empty() {
            writeln!(out, "\x1b[1;36m━━━ File Operations ━━━\x1b[0m").unwrap();
            let mut ops: Vec<_> = self.file_operations.values().collect();
            ops.sort_by_key(|b| std::cmp::Reverse(b.total()));

            for op in ops.iter().take(15) {
                let bar_width = 20;
                let max_total = ops.first().map(|o| o.total()).unwrap_or(1);
                let filled = (op.total() as f64 / max_total as f64 * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

                // Build readable operation summary
                let mut parts = Vec::new();
                if op.reads > 0 {
                    parts.push(format!(
                        "\x1b[36m{} read{}\x1b[0m",
                        op.reads,
                        if op.reads == 1 { "" } else { "s" }
                    ));
                }
                if op.edits > 0 {
                    parts.push(format!(
                        "\x1b[33m{} edit{}\x1b[0m",
                        op.edits,
                        if op.edits == 1 { "" } else { "s" }
                    ));
                }
                if op.writes > 0 {
                    parts.push(format!(
                        "\x1b[32m{} write{}\x1b[0m",
                        op.writes,
                        if op.writes == 1 { "" } else { "s" }
                    ));
                }
                let ops_str = parts.join(", ");
                writeln!(out, "{} {} {}", bar, ops_str, op.path).unwrap();
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

        // Parallelization opportunities
        if !self.tool_chains.is_empty() {
            let mut sorted_chains = self.tool_chains.clone();
            sorted_chains.sort_by_key(|b| std::cmp::Reverse(b.potential_savings()));

            let top_opportunities: Vec<_> = sorted_chains
                .iter()
                .filter(|c| c.potential_savings() >= 2)
                .take(5)
                .collect();

            if !top_opportunities.is_empty() {
                writeln!(out).unwrap();
                writeln!(out, "\x1b[1;36m━━━ Parallelization Hints ━━━\x1b[0m").unwrap();

                let total_savings: usize =
                    self.tool_chains.iter().map(|c| c.potential_savings()).sum();
                writeln!(
                    out,
                    "Potential savings: \x1b[33m{} API calls\x1b[0m",
                    total_savings
                )
                .unwrap();

                for chain in &top_opportunities {
                    let safe_marker = if chain.is_safe_parallel() {
                        "\x1b[32m✓\x1b[0m"
                    } else {
                        "\x1b[33m⚠\x1b[0m"
                    };
                    writeln!(
                        out,
                        "{} Turns {}-{}: \x1b[33m{} → 1\x1b[0m (save {})",
                        safe_marker,
                        chain.turn_range.0,
                        chain.turn_range.1,
                        chain.len(),
                        chain.potential_savings()
                    )
                    .unwrap();
                    let tools_str = chain.tools.join(" → ");
                    writeln!(out, "   {}", tools_str).unwrap();
                }
            }
        }

        // Tool patterns (multi-session aggregate)
        if !self.tool_patterns.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "\x1b[1;36m━━━ Common Tool Patterns ━━━\x1b[0m").unwrap();
            writeln!(out, "Frequent sequences across all sessions:").unwrap();
            writeln!(out).unwrap();
            for pattern in self.tool_patterns.iter().take(10) {
                writeln!(
                    out,
                    "\x1b[33m{:>3}×\x1b[0m {}",
                    pattern.occurrences,
                    pattern.pattern_str()
                )
                .unwrap();
            }
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

/// Implement OutputFormatter trait for consistent output handling.
impl OutputFormatter for SessionAnalysis {
    fn format_text(&self) -> String {
        // Call the inherent method (markdown format)
        SessionAnalysis::format_text(self)
    }

    fn format_pretty(&self) -> String {
        // Call the inherent method (colored bar charts)
        SessionAnalysis::format_pretty(self)
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

/// Generate ASCII bar chart for token growth.
fn token_growth_chart(context_per_turn: &[u64], width: usize) -> Vec<String> {
    if context_per_turn.is_empty() {
        return vec![];
    }

    let max_context = *context_per_turn.iter().max().unwrap_or(&1);
    let threshold_80k = 80_000;
    let threshold_100k = 100_000;

    let mut lines = Vec::new();

    // Sample turns if too many (show every Nth turn)
    let sample_rate = if context_per_turn.len() > 20 {
        context_per_turn.len() / 20
    } else {
        1
    };

    for (idx, &context) in context_per_turn.iter().enumerate() {
        if context == 0 {
            continue; // Skip turns without token usage
        }
        if idx % sample_rate != 0 && idx != context_per_turn.len() - 1 {
            continue; // Skip non-sampled turns, but always show last
        }

        let filled = ((context as f64 / max_context as f64) * width as f64) as usize;
        let bar = "▓".repeat(filled) + &"░".repeat(width.saturating_sub(filled));

        // Color based on threshold
        let color = if context >= threshold_100k {
            "\x1b[31m" // Red: dangerous
        } else if context >= threshold_80k {
            "\x1b[33m" // Yellow: warning
        } else {
            "\x1b[32m" // Green: ok
        };

        let warning = if context >= threshold_100k {
            " [!] APPROACHING LIMIT"
        } else if context >= threshold_80k {
            " [!] High context"
        } else {
            ""
        };

        lines.push(format!(
            "Turn {:>3}: {}{}{}\x1b[0m {}{}",
            idx,
            color,
            bar,
            " ",
            format_tokens(context),
            warning
        ));
    }

    lines
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

/// Extract file path from tool input JSON.
fn extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Write" | "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                return Some(normalize_path(path));
            }
        }
        _ => {}
    }
    None
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

/// Split a shell command line on `&&`, `;`, and `||` into individual commands.
fn split_command_chain(cmd: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = cmd.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b';' {
            let part = cmd[start..i].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = i + 1;
        } else if i + 1 < len && bytes[i] == b'&' && bytes[i + 1] == b'&' {
            let part = cmd[start..i].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = i + 2;
            i += 1; // skip extra char
        } else if i + 1 < len && bytes[i] == b'|' && bytes[i + 1] == b'|' {
            let part = cmd[start..i].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = i + 2;
            i += 1;
        }
        i += 1;
    }
    let part = cmd[start..].trim();
    if !part.is_empty() {
        parts.push(part);
    }
    // Filter out comments and empty-ish entries
    parts.into_iter().filter(|p| !p.starts_with('#')).collect()
}

fn categorize_cargo(sub: &str) -> (&'static str, String) {
    match sub {
        "build" | "b" => ("build", "cargo build".to_string()),
        "test" | "t" | "nextest" => ("test", "cargo test".to_string()),
        "clippy" => ("lint", "cargo clippy".to_string()),
        "fmt" => ("lint", "cargo fmt".to_string()),
        "add" | "install" => ("install", format!("cargo {}", sub)),
        _ => ("build", format!("cargo {}", sub)),
    }
}

fn categorize_npm_run(runner: &str, script: &str) -> (&'static str, String) {
    if script.contains("build") {
        ("build", format!("{} run build", runner))
    } else if script.contains("test") {
        ("test", format!("{} run test", runner))
    } else if script.contains("lint") {
        ("lint", format!("{} run lint", runner))
    } else if script.contains("format") || script.contains("fmt") {
        ("lint", format!("{} run {}", runner, script))
    } else {
        ("other", format!("{} run {}", runner, script))
    }
}

fn categorize_js_runner(base_name: &str, sub: &str, effective: &[&str]) -> (&'static str, String) {
    match sub {
        "run" => {
            let script = effective.get(2).copied().unwrap_or("?");
            categorize_npm_run(base_name, script)
        }
        "build" => ("build", format!("{} build", base_name)),
        "test" => ("test", format!("{} test", base_name)),
        "install" | "i" | "add" | "ci" => ("install", format!("{} install", base_name)),
        _ => ("other", format!("{} {}", base_name, sub)),
    }
}

/// Categorize a single shell command and return (category, normalized_pattern).
///
/// The normalized pattern is the base command + subcommand (e.g. "cargo test", "npm run build").
pub fn categorize_command(cmd: &str) -> (&'static str, String) {
    // Strip leading env vars (KEY=val cmd ...) and cd prefixes
    let cmd = cmd.trim();
    // Skip env var assignments at the start
    let effective = cmd
        .split_whitespace()
        .skip_while(|w| w.contains('=') && !w.starts_with('-'))
        .collect::<Vec<_>>();
    if effective.is_empty() {
        return ("other", cmd.to_string());
    }

    let base = effective[0];
    let sub = effective.get(1).copied().unwrap_or("");

    // Extract the binary name from any path (e.g. ./target/debug/cargo -> cargo)
    let base_name = base.rsplit('/').next().unwrap_or(base);

    match base_name {
        "cargo" => categorize_cargo(sub),
        "npm" | "npx" | "yarn" | "pnpm" => categorize_js_runner(base_name, sub, &effective),

        // Build tools
        "make" | "cmake" | "ninja" => ("build", base_name.to_string()),
        "tsc" => ("build", "tsc".to_string()),
        "webpack" | "vite" | "esbuild" | "rollup" | "parcel" => ("build", base_name.to_string()),

        // Test tools
        "pytest" | "jest" | "vitest" | "mocha" => ("test", base_name.to_string()),
        "go" if sub == "test" => ("test", "go test".to_string()),
        "ruby" if sub == "-e" || sub == "test" => ("test", "ruby test".to_string()),
        "rspec" | "phpunit" => ("test", base_name.to_string()),

        // Lint/format tools
        "eslint" | "prettier" | "ruff" | "black" | "flake8" | "mypy" | "pylint" | "rubocop"
        | "biome" | "oxlint" => ("lint", base_name.to_string()),

        // Git
        "git" | "gh" => {
            let git_sub = if sub.is_empty() { "git" } else { sub };
            ("git", format!("{} {}", base_name, git_sub))
        }

        // Install/dependency
        "pip" | "pip3" if sub == "install" => ("install", "pip install".to_string()),
        "apt" | "apt-get" | "brew" | "dnf" | "pacman" | "nix" => {
            ("install", format!("{} {}", base_name, sub))
        }

        // Search/read tools
        "find" | "grep" | "rg" | "ag" | "fd" => ("search", base_name.to_string()),
        "ls" | "cat" | "head" | "tail" | "wc" | "file" | "stat" | "tree" | "less" => {
            ("search", base_name.to_string())
        }

        _ => ("other", base_name.to_string()),
    }
}

/// Detect retry hotspots from a sequence of command invocations.
///
/// Takes `(turn_idx, normalized_pattern, was_error)` triples and groups
/// them by pattern. A hotspot = pattern with >= 2 failures out of >= 3 attempts.
fn detect_retry_hotspots(
    invocations: &[(usize, String, bool)],
    output_tokens_per_turn: &[u64],
) -> Vec<RetryHotspot> {
    // Group by normalized pattern
    let mut by_pattern: HashMap<String, Vec<(usize, bool)>> = HashMap::new();
    for (turn_idx, pattern, was_error) in invocations {
        by_pattern
            .entry(pattern.clone())
            .or_default()
            .push((*turn_idx, *was_error));
    }

    let mut hotspots = Vec::new();
    for (pattern, entries) in &by_pattern {
        let attempts = entries.len();
        let failures = entries.iter().filter(|(_, err)| *err).count();
        if failures >= 2 && attempts >= 3 {
            let turn_indices: Vec<usize> = entries.iter().map(|(idx, _)| *idx).collect();
            let output_tokens: u64 = turn_indices
                .iter()
                .filter_map(|&idx| output_tokens_per_turn.get(idx))
                .sum();
            hotspots.push(RetryHotspot {
                pattern: pattern.clone(),
                attempts,
                failures,
                output_tokens,
                turn_indices,
            });
        }
    }

    // Sort by failures descending, then by output_tokens descending
    hotspots.sort_by(|a, b| {
        b.failures
            .cmp(&a.failures)
            .then(b.output_tokens.cmp(&a.output_tokens))
    });

    hotspots
}

/// Build command stats from invocation data.
fn build_command_stats(
    invocations: &[(usize, String, bool, &'static str)],
    output_tokens_per_turn: &[u64],
) -> Vec<CommandStats> {
    // Group by category
    let mut by_category: HashMap<&str, HashMap<String, (usize, usize)>> = HashMap::new();
    let mut category_turns: HashMap<&str, Vec<usize>> = HashMap::new();

    for (turn_idx, pattern, was_error, category) in invocations {
        let commands = by_category.entry(category).or_default();
        let entry = commands.entry(pattern.clone()).or_insert((0, 0));
        entry.0 += 1;
        if *was_error {
            entry.1 += 1;
        }
        category_turns.entry(category).or_default().push(*turn_idx);
    }

    let mut stats: Vec<CommandStats> = by_category
        .into_iter()
        .map(|(category, commands)| {
            let total_calls: usize = commands.values().map(|(c, _)| c).sum();
            let total_errors: usize = commands.values().map(|(_, e)| e).sum();

            // Deduplicate turn indices and sum output tokens
            let mut turns: Vec<usize> = category_turns.get(category).cloned().unwrap_or_default();
            turns.sort_unstable();
            turns.dedup();
            let output_tokens: u64 = turns
                .iter()
                .filter_map(|&idx| output_tokens_per_turn.get(idx))
                .sum();

            let mut details: Vec<CommandDetail> = commands
                .into_iter()
                .map(|(pattern, (calls, errors))| CommandDetail {
                    pattern,
                    calls,
                    errors,
                })
                .collect();
            details.sort_by(|a, b| b.calls.cmp(&a.calls));

            CommandStats {
                category: category.to_string(),
                commands: details,
                total_calls,
                total_errors,
                output_tokens,
            }
        })
        .collect();

    // Sort by total_calls descending
    stats.sort_by(|a, b| b.total_calls.cmp(&a.total_calls));
    stats
}

/// Extract all subsequences of length 2-5 from tool chains.
pub fn extract_tool_patterns(chains: &[ToolChain]) -> Vec<ToolPattern> {
    let mut pattern_counts: HashMap<Vec<String>, usize> = HashMap::new();

    for chain in chains {
        // Extract all subsequences of length 2-5
        for len in 2..=5.min(chain.tools.len()) {
            for start in 0..=chain.tools.len().saturating_sub(len) {
                let subsequence: Vec<String> = chain.tools[start..start + len].to_vec();
                *pattern_counts.entry(subsequence).or_insert(0) += 1;
            }
        }
    }

    // Convert to ToolPattern vec and filter out single occurrences
    let mut patterns: Vec<ToolPattern> = pattern_counts
        .into_iter()
        .filter(|(_, count)| *count >= 2) // Only keep patterns that occur 2+ times
        .map(|(tools, occurrences)| ToolPattern { tools, occurrences })
        .collect();

    // Sort by occurrence count (descending), then by pattern length (descending)
    patterns.sort_by(|a, b| {
        b.occurrences
            .cmp(&a.occurrences)
            .then(b.tools.len().cmp(&a.tools.len()))
    });

    patterns
}

/// Analyze a parsed session and compute statistics.
pub fn analyze_session(session: &Session) -> SessionAnalysis {
    let mut analysis = SessionAnalysis::new(session.path.clone(), &session.format);

    // Count message types by role, splitting user messages into human vs tool_result
    for turn in &session.turns {
        for msg in &turn.messages {
            let role_str = msg.role.to_string();
            *analysis.message_counts.entry(role_str).or_insert(0) += 1;

            // Break down user messages: human text vs tool results
            if msg.role == normalize_chat_sessions::Role::User {
                let has_tool_result = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, normalize_chat_sessions::ContentBlock::ToolResult { .. }));
                if has_tool_result {
                    *analysis
                        .message_counts
                        .entry("tool_result".to_string())
                        .or_insert(0) += 1;
                } else {
                    *analysis
                        .message_counts
                        .entry("human".to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }

    // Analyze tool usage, detect tool chains, and collect command data
    let mut current_chain: Option<Vec<(usize, String)>> = None;

    // For command analysis: (turn_idx, pattern, was_error, category)
    let mut command_invocations: Vec<(usize, String, bool, &'static str)> = Vec::new();
    // For retry detection: (turn_idx, pattern, was_error)
    let mut retry_candidates: Vec<(usize, String, bool)> = Vec::new();
    // Output tokens per turn index (populated in the token usage pass below)
    let mut output_tokens_per_turn: Vec<u64> = Vec::new();

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        let mut tool_uses_in_turn = 0;
        let mut tool_name_in_turn: Option<String> = None;

        // Collect Bash tool_use IDs and their commands for this turn
        let mut bash_commands: HashMap<String, Vec<(String, &'static str)>> = HashMap::new();
        // Track which tool_use IDs had errors
        let mut tool_errors: HashMap<String, bool> = HashMap::new();

        for msg in &turn.messages {
            // Detect corrections in assistant messages
            if msg.role == normalize_chat_sessions::Role::Assistant {
                for block in &msg.content {
                    if let ContentBlock::Text { text } = block
                        && let Some((category, excerpt)) = detect_correction(text)
                    {
                        analysis.corrections.push(Correction {
                            turn: turn_idx,
                            text: excerpt,
                            category,
                        });
                    }
                }
            }

            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        let stat = analysis
                            .tool_stats
                            .entry(name.clone())
                            .or_insert_with(|| ToolStats::new(name));
                        stat.calls += 1;
                        tool_uses_in_turn += 1;
                        tool_name_in_turn = Some(name.clone());

                        // Track file operations
                        if let Some(file_path) = extract_file_path(name, input) {
                            let op = analysis
                                .file_operations
                                .entry(file_path.clone())
                                .or_insert_with(|| FileOperation {
                                    path: file_path.clone(),
                                    ..Default::default()
                                });
                            match name.as_str() {
                                "Read" => op.reads += 1,
                                "Edit" => op.edits += 1,
                                "Write" => op.writes += 1,
                                _ => {}
                            }
                        }

                        // Collect Bash commands for categorization
                        if name == "Bash"
                            && let Some(cmd) = input.get("command").and_then(|v| v.as_str())
                        {
                            let subcmds = split_command_chain(cmd);
                            let mut entries = Vec::new();
                            for subcmd in subcmds {
                                let (cat, pattern) = categorize_command(subcmd);
                                entries.push((pattern, cat));
                            }
                            bash_commands.insert(id.clone(), entries);
                        }
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        is_error,
                        content,
                        ..
                    } => {
                        // Track error status for Bash tool matching
                        tool_errors.insert(tool_use_id.clone(), *is_error);

                        if *is_error {
                            // Attribute error to tool stat
                            // Find which tool this result belongs to by scanning the turn
                            for m in &turn.messages {
                                for b in &m.content {
                                    if let ContentBlock::ToolUse { id, name, .. } = b
                                        && id == tool_use_id
                                        && let Some(stat) = analysis.tool_stats.get_mut(name)
                                    {
                                        stat.errors += 1;
                                    }
                                }
                            }

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

        // Process collected bash commands for this turn
        for (tool_id, entries) in &bash_commands {
            let was_error = tool_errors.get(tool_id).copied().unwrap_or(false);
            for (pattern, category) in entries {
                command_invocations.push((turn_idx, pattern.clone(), was_error, category));
                retry_candidates.push((turn_idx, pattern.clone(), was_error));
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
            if let Some(chain) = current_chain.take()
                && chain.len() >= 3
            {
                let tools: Vec<String> = chain.iter().map(|(_, name)| name.clone()).collect();
                let turn_range = (chain.first().unwrap().0, chain.last().unwrap().0);
                analysis.tool_chains.push(ToolChain { tools, turn_range });
            }
        }
    }

    // Handle final chain
    if let Some(chain) = current_chain
        && chain.len() >= 3
    {
        let tools: Vec<String> = chain.iter().map(|(_, name)| name.clone()).collect();
        let turn_range = (chain.first().unwrap().0, chain.last().unwrap().0);
        analysis.tool_chains.push(ToolChain { tools, turn_range });
    }

    // Analyze token usage and build output_tokens_per_turn
    analysis.total_turns = session.turns.len();
    let mut actual_cost_sum: f64 = 0.0;
    let mut has_model_pricing = false;
    let mut prev_context: u64 = 0;
    let mut unique_input: u64 = 0;

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
            analysis.context_per_turn.push(context);
            output_tokens_per_turn.push(usage.output);

            // Actual cost from per-turn model
            if let Some(model_str) = &usage.model
                && let Some(pricing) = ModelPricing::from_model_str(model_str)
            {
                actual_cost_sum += pricing.calculate_turn_cost(usage);
                has_model_pricing = true;
            }

            // Dedup: unique input = only context growth
            unique_input += context.saturating_sub(prev_context);
            prev_context = context;
        } else {
            analysis.context_per_turn.push(0);
            output_tokens_per_turn.push(0);
        }
    }

    if has_model_pricing {
        analysis.actual_cost = Some(actual_cost_sum);
    }

    // Compute dedup token stats
    let total_billed = analysis.token_stats.total_input
        + analysis.token_stats.cache_read
        + analysis.token_stats.total_output;
    if total_billed > 0 {
        let unique_output = analysis.token_stats.total_output;
        let unique_total = unique_input + unique_output;
        analysis.dedup_tokens = Some(DedupTokenStats {
            unique_input,
            unique_output,
            total_billed,
            uniqueness_ratio: unique_total as f64 / total_billed as f64,
        });
    }

    // Build command stats and retry hotspots
    analysis.command_stats = build_command_stats(&command_invocations, &output_tokens_per_turn);
    analysis.retry_hotspots = detect_retry_hotspots(&retry_candidates, &output_tokens_per_turn);

    // Sort error patterns by count
    analysis
        .error_patterns
        .sort_by(|a, b| b.count.cmp(&a.count));

    analysis
}
