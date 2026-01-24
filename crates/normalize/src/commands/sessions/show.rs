//! Show/analyze a specific session.

use super::analyze::{cmd_sessions_analyze, cmd_sessions_analyze_multi, cmd_sessions_jq};
use super::resolve_session_paths;
use rhi_normalize_sessions::{ContentBlock, Role, Session};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Show/analyze a specific session or sessions matching a pattern.
#[allow(clippy::too_many_arguments)]
pub fn cmd_sessions_show(
    session_id: &str,
    project: Option<&Path>,
    jq_filter: Option<&str>,
    format: Option<&str>,
    analyze: bool,
    json: bool,
    pretty: bool,
    filter: Option<&str>,
    grep_pattern: Option<&str>,
    errors_only: bool,
    ngrams: Option<usize>,
    case_insensitive: bool,
) -> i32 {
    // Find matching session files
    let paths = resolve_session_paths(session_id, project, format);

    if paths.is_empty() {
        eprintln!("No sessions found matching: {}", session_id);
        return 1;
    }

    // If --analyze with multiple sessions, aggregate
    if analyze && paths.len() > 1 {
        return cmd_sessions_analyze_multi(&paths, format, json, pretty);
    }

    // If --analyze with single session
    if analyze {
        return cmd_sessions_analyze(&paths[0], format, json, pretty);
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
        use rhi_normalize_sessions::{FormatRegistry, LogFormat};

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

    // Default: dump the raw JSONL (only first match for non-glob)
    let path = &paths[0];
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open {}: {}", path.display(), e);
            return 1;
        }
    };

    let reader = BufReader::new(file);
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    for line in reader.lines() {
        match line {
            Ok(l) => {
                let _ = writeln!(stdout, "{}", l);
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                return 1;
            }
        }
    }

    0
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

    use std::collections::HashMap;

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
        println!("{}× {}", count, ngram.join(" "));
    }

    if ngrams.len() > 30 {
        println!("\n({} more unique {}-grams)", ngrams.len() - 30, n);
    }

    0
}
