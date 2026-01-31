//! Show/analyze a specific session.

use super::analyze::{cmd_sessions_analyze, cmd_sessions_analyze_multi, cmd_sessions_jq};
use super::{resolve_session_paths, resolve_session_paths_literal};
use crate::output::{OutputFormat, OutputFormatter};
use normalize_chat_sessions::{ContentBlock, Role, Session};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

/// Report wrapping a parsed session for OutputFormatter display.
///
/// Default text output is a summary (metadata, user prompts, tool usage, errors).
/// Use `full(true)` for complete conversation output.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct SessionShowReport {
    session: Session,
    #[serde(skip)]
    show_full: bool,
}

impl SessionShowReport {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            show_full: false,
        }
    }

    pub fn full(mut self, full: bool) -> Self {
        self.show_full = full;
        self
    }
}

impl OutputFormatter for SessionShowReport {
    fn format_text(&self) -> String {
        if self.show_full {
            return self.format_full_text();
        }
        self.format_summary_text()
    }

    fn format_pretty(&self) -> String {
        if self.show_full {
            return self.format_full_pretty();
        }
        self.format_summary_pretty()
    }
}

impl SessionShowReport {
    // ── Summary view (default) ──────────────────────────────────────────

    fn format_summary_text(&self) -> String {
        let mut out = String::new();
        let s = &self.session;

        // Metadata header
        if let Some(id) = &s.metadata.session_id {
            let _ = writeln!(out, "# Session {}", id);
        }
        if let Some(model) = &s.metadata.model {
            let _ = writeln!(out, "model: {}", model);
        }
        if let Some(ts) = &s.metadata.timestamp {
            let _ = writeln!(out, "time: {}", ts);
        }
        if let Some(project) = &s.metadata.project {
            let _ = writeln!(out, "project: {}", project);
        }

        // Stats line
        let tokens = s.total_tokens();
        let _ = write!(
            out,
            "{} turns, {} messages",
            s.turns.len(),
            s.message_count()
        );
        if tokens.input > 0 || tokens.output > 0 {
            let _ = write!(
                out,
                " | {}in {}out",
                format_tokens(tokens.input),
                format_tokens(tokens.output),
            );
        }
        let _ = writeln!(out);
        let _ = writeln!(out);

        // User prompts
        let prompts = extract_user_prompts(s);
        if !prompts.is_empty() {
            let _ = writeln!(out, "## Prompts");
            let _ = writeln!(out);
            for (turn_idx, text) in &prompts {
                let _ = writeln!(out, "Turn {}: {}", turn_idx, truncate(text, 120));
            }
            let _ = writeln!(out);
        }

        // Tool usage
        let tool_counts = count_tools(s);
        if !tool_counts.is_empty() {
            let _ = writeln!(out, "## Tools");
            let _ = writeln!(out);
            let mut tools: Vec<_> = tool_counts.iter().collect();
            tools.sort_by(|a, b| b.1.cmp(a.1));
            for (name, count) in &tools {
                let _ = writeln!(out, "  {} {}", count, name);
            }
            let _ = writeln!(out);
        }

        // Errors
        let errors = collect_errors(s);
        if !errors.is_empty() {
            let _ = writeln!(out, "## Errors ({})", errors.len());
            let _ = writeln!(out);
            for (turn_idx, text) in errors.iter().take(5) {
                let _ = writeln!(out, "Turn {}: {}", turn_idx, truncate(text, 100));
            }
            if errors.len() > 5 {
                let _ = writeln!(out, "  ... and {} more", errors.len() - 5);
            }
            let _ = writeln!(out);
        }

        // Files touched
        let files = collect_file_ops(s);
        if !files.is_empty() {
            let _ = writeln!(out, "## Files");
            let _ = writeln!(out);
            let mut files: Vec<_> = files.iter().collect();
            files.sort_by(|a, b| {
                let a_total = a.1.0 + a.1.1 + a.1.2;
                let b_total = b.1.0 + b.1.1 + b.1.2;
                b_total.cmp(&a_total)
            });
            for (path, (reads, edits, writes)) in files.iter().take(15) {
                let mut parts = Vec::new();
                if *reads > 0 {
                    parts.push(format!("{}r", reads));
                }
                if *edits > 0 {
                    parts.push(format!("{}e", edits));
                }
                if *writes > 0 {
                    parts.push(format!("{}w", writes));
                }
                let _ = writeln!(out, "  {} {}", parts.join("/"), path);
            }
            if files.len() > 15 {
                let _ = writeln!(out, "  ... and {} more files", files.len() - 15);
            }
        }

        out
    }

    fn format_summary_pretty(&self) -> String {
        use nu_ansi_term::Color::{Blue, Cyan, Green, Red, Yellow};

        let mut out = String::new();
        let s = &self.session;

        // Metadata header
        if let Some(id) = &s.metadata.session_id {
            let _ = writeln!(out, "{}", Green.bold().paint(format!("# Session {}", id)));
        }
        if let Some(model) = &s.metadata.model {
            let _ = writeln!(out, "{} {}", Cyan.paint("model:"), model);
        }
        if let Some(ts) = &s.metadata.timestamp {
            let _ = writeln!(out, "{}  {}", Cyan.paint("time:"), ts);
        }
        if let Some(project) = &s.metadata.project {
            let _ = writeln!(out, "{} {}", Cyan.paint("project:"), project);
        }

        // Stats line
        let tokens = s.total_tokens();
        let _ = write!(
            out,
            "{}",
            Cyan.paint(format!(
                "{} turns, {} messages",
                s.turns.len(),
                s.message_count()
            ))
        );
        if tokens.input > 0 || tokens.output > 0 {
            let _ = write!(
                out,
                " | {}",
                Cyan.paint(format!(
                    "{}in {}out",
                    format_tokens(tokens.input),
                    format_tokens(tokens.output),
                ))
            );
        }
        let _ = writeln!(out);
        let _ = writeln!(out);

        // User prompts
        let prompts = extract_user_prompts(s);
        if !prompts.is_empty() {
            let _ = writeln!(out, "{}", Blue.bold().paint("## Prompts"));
            let _ = writeln!(out);
            for (turn_idx, text) in &prompts {
                let _ = writeln!(
                    out,
                    "{} {}",
                    Cyan.paint(format!("Turn {}:", turn_idx)),
                    truncate(text, 120)
                );
            }
            let _ = writeln!(out);
        }

        // Tool usage
        let tool_counts = count_tools(s);
        if !tool_counts.is_empty() {
            let _ = writeln!(out, "{}", Green.bold().paint("## Tools"));
            let _ = writeln!(out);
            let mut tools: Vec<_> = tool_counts.iter().collect();
            tools.sort_by(|a, b| b.1.cmp(a.1));
            for (name, count) in &tools {
                let _ = writeln!(out, "  {} {}", Yellow.paint(format!("{:>3}", count)), name);
            }
            let _ = writeln!(out);
        }

        // Errors
        let errors = collect_errors(s);
        if !errors.is_empty() {
            let _ = writeln!(
                out,
                "{}",
                Red.bold().paint(format!("## Errors ({})", errors.len()))
            );
            let _ = writeln!(out);
            for (turn_idx, text) in errors.iter().take(5) {
                let _ = writeln!(
                    out,
                    "  {} {}",
                    Red.paint(format!("Turn {}:", turn_idx)),
                    truncate(text, 100)
                );
            }
            if errors.len() > 5 {
                let _ = writeln!(
                    out,
                    "  {}",
                    Red.paint(format!("... and {} more", errors.len() - 5))
                );
            }
            let _ = writeln!(out);
        }

        // Files touched
        let files = collect_file_ops(s);
        if !files.is_empty() {
            let _ = writeln!(out, "{}", Cyan.bold().paint("## Files"));
            let _ = writeln!(out);
            let mut files: Vec<_> = files.iter().collect();
            files.sort_by(|a, b| {
                let a_total = a.1.0 + a.1.1 + a.1.2;
                let b_total = b.1.0 + b.1.1 + b.1.2;
                b_total.cmp(&a_total)
            });
            for (path, (reads, edits, writes)) in files.iter().take(15) {
                let mut parts = Vec::new();
                if *reads > 0 {
                    parts.push(format!("{}r", reads));
                }
                if *edits > 0 {
                    parts.push(format!("{}e", edits));
                }
                if *writes > 0 {
                    parts.push(format!("{}w", writes));
                }
                let _ = writeln!(
                    out,
                    "  {} {}",
                    Yellow.paint(format!("{:>8}", parts.join("/"))),
                    path
                );
            }
            if files.len() > 15 {
                let _ = writeln!(
                    out,
                    "  {}",
                    Cyan.paint(format!("... and {} more files", files.len() - 15))
                );
            }
        }

        out
    }

    // ── Full conversation view (--full) ─────────────────────────────────

    fn format_full_text(&self) -> String {
        let mut out = String::new();

        if let Some(id) = &self.session.metadata.session_id {
            let _ = writeln!(out, "# Session {}", id);
        }
        if let Some(model) = &self.session.metadata.model {
            let _ = writeln!(out, "model: {}", model);
        }
        if let Some(ts) = &self.session.metadata.timestamp {
            let _ = writeln!(out, "time: {}", ts);
        }
        if let Some(project) = &self.session.metadata.project {
            let _ = writeln!(out, "project: {}", project);
        }
        let _ = writeln!(out);

        for (turn_idx, turn) in self.session.turns.iter().enumerate() {
            for msg in &turn.messages {
                for block in &msg.content {
                    let _ = writeln!(
                        out,
                        "=== Turn {} | {} ===",
                        turn_idx,
                        format_role_and_type(&msg.role, block)
                    );
                    format_block_text(&mut out, block);
                    let _ = writeln!(out);
                }
            }
        }

        out
    }

    fn format_full_pretty(&self) -> String {
        use nu_ansi_term::Color::{Blue, Cyan, Green, Yellow};

        let mut out = String::new();

        if let Some(id) = &self.session.metadata.session_id {
            let _ = writeln!(out, "{}", Green.bold().paint(format!("# Session {}", id)));
        }
        if let Some(model) = &self.session.metadata.model {
            let _ = writeln!(out, "{} {}", Cyan.paint("model:"), model);
        }
        if let Some(ts) = &self.session.metadata.timestamp {
            let _ = writeln!(out, "{}  {}", Cyan.paint("time:"), ts);
        }
        if let Some(project) = &self.session.metadata.project {
            let _ = writeln!(out, "{} {}", Cyan.paint("project:"), project);
        }
        let _ = writeln!(out);

        for (turn_idx, turn) in self.session.turns.iter().enumerate() {
            for msg in &turn.messages {
                for block in &msg.content {
                    let header_color = match msg.role {
                        Role::User => Blue,
                        Role::Assistant => Green,
                        Role::System => Yellow,
                    };
                    let header = format!(
                        "=== Turn {} | {} ===",
                        turn_idx,
                        format_role_and_type(&msg.role, block)
                    );
                    let _ = writeln!(out, "{}", header_color.bold().paint(header));
                    format_block_pretty(&mut out, block);
                    let _ = writeln!(out);
                }
            }
        }

        // Summary footer
        let total_turns = self.session.turns.len();
        let total_messages = self.session.message_count();
        let tokens = self.session.total_tokens();
        let _ = write!(
            out,
            "{}",
            Cyan.paint(format!(
                "{} turns, {} messages",
                total_turns, total_messages
            ))
        );
        if tokens.input > 0 || tokens.output > 0 {
            let _ = write!(
                out,
                " | {}",
                Cyan.paint(format!(
                    "{}in {}out",
                    format_tokens(tokens.input),
                    format_tokens(tokens.output),
                ))
            );
        }
        let _ = writeln!(out);

        out
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Extract the first text block from each user message, with turn index.
fn extract_user_prompts(session: &Session) -> Vec<(usize, String)> {
    let mut prompts = Vec::new();
    for (turn_idx, turn) in session.turns.iter().enumerate() {
        for msg in &turn.messages {
            if msg.role != Role::User {
                continue;
            }
            for block in &msg.content {
                if let ContentBlock::Text { text } = block {
                    if !text.trim().is_empty() {
                        prompts.push((turn_idx, text.clone()));
                        break;
                    }
                }
            }
        }
    }
    prompts
}

/// Count tool calls by name.
fn count_tools(session: &Session) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for turn in &session.turns {
        for msg in &turn.messages {
            for block in &msg.content {
                if let ContentBlock::ToolUse { name, .. } = block {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

/// Collect error tool results with turn index.
fn collect_errors(session: &Session) -> Vec<(usize, String)> {
    let mut errors = Vec::new();
    for (turn_idx, turn) in session.turns.iter().enumerate() {
        for msg in &turn.messages {
            for block in &msg.content {
                if let ContentBlock::ToolResult {
                    is_error: true,
                    content,
                    ..
                } = block
                {
                    errors.push((turn_idx, content.clone()));
                }
            }
        }
    }
    errors
}

/// Collect file operations: path -> (reads, edits, writes).
fn collect_file_ops(session: &Session) -> HashMap<String, (usize, usize, usize)> {
    let mut ops: HashMap<String, (usize, usize, usize)> = HashMap::new();
    for turn in &session.turns {
        for msg in &turn.messages {
            for block in &msg.content {
                if let ContentBlock::ToolUse { name, input, .. } = block {
                    let path = match name.as_str() {
                        "Read" | "Write" | "Edit" => {
                            input.get("file_path").and_then(|v| v.as_str())
                        }
                        _ => None,
                    };
                    if let Some(path) = path {
                        let entry = ops.entry(normalize_path(path)).or_default();
                        match name.as_str() {
                            "Read" => entry.0 += 1,
                            "Edit" => entry.1 += 1,
                            "Write" => entry.2 += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    ops
}

/// Normalize a file path for display (strip common prefixes).
fn normalize_path(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }
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

/// Format token count with K/M suffix.
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M ", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K ", tokens as f64 / 1_000.0)
    } else {
        format!("{} ", tokens)
    }
}

/// Truncate a string to max chars, adding "..." if truncated. Collapses newlines.
fn truncate(s: &str, max: usize) -> String {
    let collapsed: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = collapsed.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max])
    }
}

/// Format a content block as plain text.
fn format_block_text(out: &mut String, block: &ContentBlock) {
    match block {
        ContentBlock::Text { text } => {
            let _ = writeln!(out, "{}", text);
        }
        ContentBlock::ToolUse { name, input, .. } => {
            let _ = writeln!(out, "Tool: {}", name);
            let _ = writeln!(
                out,
                "Input: {}",
                serde_json::to_string_pretty(input).unwrap_or_else(|_| format!("{:?}", input))
            );
        }
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            if *is_error {
                let _ = writeln!(out, "[ERROR]");
            }
            let _ = writeln!(out, "{}", content);
        }
        ContentBlock::Thinking { text } => {
            let _ = writeln!(out, "[THINKING]");
            let _ = writeln!(out, "{}", text);
        }
    }
}

/// Format a content block with ANSI colors.
fn format_block_pretty(out: &mut String, block: &ContentBlock) {
    use nu_ansi_term::Color::{Cyan, Red, Yellow};

    match block {
        ContentBlock::Text { text } => {
            let _ = writeln!(out, "{}", text);
        }
        ContentBlock::ToolUse { name, input, .. } => {
            let _ = writeln!(
                out,
                "{} {}",
                Cyan.bold().paint("Tool:"),
                Cyan.paint(name.as_str())
            );
            let _ = writeln!(
                out,
                "Input: {}",
                serde_json::to_string_pretty(input).unwrap_or_else(|_| format!("{:?}", input))
            );
        }
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            if *is_error {
                let _ = writeln!(out, "{}", Red.bold().paint("[ERROR]"));
            }
            let _ = writeln!(out, "{}", content);
        }
        ContentBlock::Thinking { text } => {
            let _ = writeln!(out, "{}", Yellow.paint("[THINKING]"));
            let _ = writeln!(out, "{}", text);
        }
    }
}

/// Show/analyze a specific session or sessions matching a pattern.
#[allow(clippy::too_many_arguments)]
pub fn cmd_sessions_show(
    session_id: &str,
    project: Option<&Path>,
    jq_filter: Option<&str>,
    format: Option<&str>,
    analyze: bool,
    full: bool,
    output_format: &OutputFormat,
    filter: Option<&str>,
    grep_pattern: Option<&str>,
    errors_only: bool,
    ngrams: Option<usize>,
    case_insensitive: bool,
    exact: bool,
) -> i32 {
    // Find matching session files
    let paths = if exact {
        resolve_session_paths_literal(session_id, project, format)
    } else {
        resolve_session_paths(session_id, project, format)
    };

    if paths.is_empty() {
        eprintln!("No sessions found matching: {}", session_id);
        return 1;
    }

    // If --analyze with multiple sessions, aggregate
    if analyze && paths.len() > 1 {
        return cmd_sessions_analyze_multi(&paths, format, output_format);
    }

    // If --analyze with single session
    if analyze {
        return cmd_sessions_analyze(&paths[0], format, output_format);
    }

    // If --jq with multiple sessions, apply to all
    if let Some(jq) = jq_filter {
        let mut exit_code = 0;
        for path in &paths {
            let code = cmd_sessions_jq(path, jq);
            if code != 0 {
                exit_code = code;
            }
        }
        return exit_code;
    }

    // If --filter or --grep or --ngrams with message analysis
    if filter.is_some() || grep_pattern.is_some() || errors_only || ngrams.is_some() {
        use normalize_chat_sessions::{FormatRegistry, LogFormat};

        let registry = FormatRegistry::new();
        let log_format: &dyn LogFormat = match format {
            Some(name) => match registry.get(name) {
                Some(f) => f,
                None => {
                    eprintln!("Unknown format: {}", name);
                    return 1;
                }
            },
            None => registry.get("claude").unwrap(),
        };

        let path = &paths[0];
        let session = match log_format.parse(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse session: {}", e);
                return 1;
            }
        };

        if let Some(n) = ngrams {
            return cmd_sessions_ngrams(&session, n, case_insensitive);
        }

        return cmd_sessions_filter(&session, filter, grep_pattern, errors_only);
    }

    // Default: parse and display via OutputFormatter
    let path = &paths[0];
    let session = match parse_session_for_show(path, format) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    let report = SessionShowReport::new(session).full(full);
    report.print(output_format);
    0
}

/// Parse a session file for the show command.
fn parse_session_for_show(path: &Path, format: Option<&str>) -> Result<Session, String> {
    use normalize_chat_sessions::{FormatRegistry, LogFormat};

    let registry = FormatRegistry::new();
    let log_format: &dyn LogFormat = match format {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").unwrap(),
    };

    log_format.parse(path)
}

/// Filter and display messages from a session.
fn cmd_sessions_filter(
    session: &Session,
    filter: Option<&str>,
    grep_pattern: Option<&str>,
    errors_only: bool,
) -> i32 {
    let mut shown = 0;

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        for msg in &turn.messages {
            // Check if we need to filter by role
            let role_str = match msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };

            // Filter by content block type
            for block in &msg.content {
                // Determine if this block matches the filter
                let block_type = match block {
                    ContentBlock::Text { .. } => role_str, // Text belongs to the message role
                    ContentBlock::ToolUse { .. } => "tool_use",
                    ContentBlock::ToolResult { .. } => "tool_result",
                    ContentBlock::Thinking { .. } => "thinking",
                };

                // Apply filter
                if let Some(f) = filter {
                    if f != block_type {
                        continue;
                    }
                }

                // Apply errors_only filter
                if errors_only {
                    match block {
                        ContentBlock::ToolResult { is_error, .. } => {
                            if !is_error {
                                continue;
                            }
                        }
                        _ => continue,
                    }
                }

                // Extract text for grep matching
                let text = match block {
                    ContentBlock::Text { text } => text.as_str(),
                    ContentBlock::ToolUse { name, input, .. } => &format!("{}: {}", name, input),
                    ContentBlock::ToolResult { content, .. } => content.as_str(),
                    ContentBlock::Thinking { text } => text.as_str(),
                };

                // Apply grep filter
                if let Some(pattern) = grep_pattern {
                    if !text.to_lowercase().contains(&pattern.to_lowercase()) {
                        continue;
                    }
                }

                // Display the matching content
                println!(
                    "=== Turn {} | {} ===",
                    turn_idx,
                    format_role_and_type(&msg.role, block)
                );
                match block {
                    ContentBlock::Text { text } => {
                        println!("{}", text);
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        println!("Tool: {}", name);
                        println!(
                            "Input: {}",
                            serde_json::to_string_pretty(input)
                                .unwrap_or_else(|_| format!("{:?}", input))
                        );
                    }
                    ContentBlock::ToolResult {
                        content, is_error, ..
                    } => {
                        if *is_error {
                            println!("[ERROR]");
                        }
                        println!("{}", content);
                    }
                    ContentBlock::Thinking { text } => {
                        println!("[THINKING]");
                        println!("{}", text);
                    }
                }
                println!();
                shown += 1;
            }
        }
    }

    if shown == 0 {
        eprintln!("No matching messages found");
        return 1;
    }

    0
}

fn format_role_and_type(role: &Role, block: &ContentBlock) -> String {
    let role_str = match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    };

    let type_str = match block {
        ContentBlock::Text { .. } => "text",
        ContentBlock::ToolUse { .. } => "tool_use",
        ContentBlock::ToolResult { .. } => "tool_result",
        ContentBlock::Thinking { .. } => "thinking",
    };

    format!("{}/{}", role_str, type_str)
}

/// Extract and display common n-grams (word sequences) from session messages.
fn cmd_sessions_ngrams(session: &Session, n: usize, case_insensitive: bool) -> i32 {
    // Validate n is in reasonable range
    if n < 2 || n > 4 {
        eprintln!("N-gram length must be 2-4");
        return 1;
    }

    let mut ngram_counts: HashMap<Vec<String>, usize> = HashMap::new();

    // Extract text from all assistant messages
    for turn in &session.turns {
        for msg in &turn.messages {
            if msg.role != Role::Assistant {
                continue;
            }

            for block in &msg.content {
                let text = match block {
                    ContentBlock::Text { text } => text.as_str(),
                    ContentBlock::Thinking { text } => text.as_str(),
                    _ => continue,
                };

                // Tokenize: split on whitespace and punctuation, filter empty
                let words: Vec<String> = text
                    .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
                    .filter(|w| !w.is_empty())
                    .map(|w| {
                        if case_insensitive {
                            w.to_lowercase()
                        } else {
                            w.to_string()
                        }
                    })
                    .collect();

                // Generate n-grams
                for window in words.windows(n) {
                    let ngram = window.to_vec();
                    *ngram_counts.entry(ngram).or_insert(0) += 1;
                }
            }
        }
    }

    // Filter out single occurrences and sort by frequency
    let mut ngrams: Vec<_> = ngram_counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect();
    ngrams.sort_by(|a, b| b.1.cmp(&a.1));

    if ngrams.is_empty() {
        eprintln!("No repeated {}-grams found", n);
        return 1;
    }

    // Display top 30
    println!("=== Top {}-grams ===\n", n);
    for (ngram, count) in ngrams.iter().take(30) {
        println!("{}x {}", count, ngram.join(" "));
    }

    if ngrams.len() > 30 {
        println!("\n({} more unique {}-grams)", ngrams.len() - 30, n);
    }

    0
}
