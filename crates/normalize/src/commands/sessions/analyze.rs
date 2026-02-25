//! Session analysis functions.

use crate::output::{OutputFormat, OutputFormatter};
use crate::sessions::{
    DedupTokenStats, SessionAnalysis, ToolStats, analyze_session, parse_session,
    parse_session_with_format,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Analyze a session and output statistics.
pub fn cmd_sessions_analyze(
    path: &Path,
    format: Option<&str>,
    output_format: &OutputFormat,
) -> i32 {
    // Parse the session
    let session = if let Some(fmt) = format {
        parse_session_with_format(path, fmt)
    } else {
        parse_session(path)
    };

    let session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to parse session: {}", e);
            return 1;
        }
    };

    // Analyze the parsed session
    let analysis = analyze_session(&session);
    analysis.print(output_format);
    0
}

/// Analyze multiple sessions and aggregate statistics.
pub fn cmd_sessions_analyze_multi(
    paths: &[PathBuf],
    format: Option<&str>,
    output_format: &OutputFormat,
) -> i32 {
    match aggregate_sessions(paths, format) {
        Some(aggregate) => {
            aggregate.print(output_format);
            0
        }
        None => {
            eprintln!("No sessions could be analyzed");
            1
        }
    }
}

/// Aggregate multiple sessions into a single analysis. Returns None if no sessions could be parsed.
pub fn aggregate_sessions(paths: &[PathBuf], format: Option<&str>) -> Option<SessionAnalysis> {
    let mut aggregate = SessionAnalysis::new(PathBuf::from("."), "aggregate");
    let mut session_count = 0;
    let mut all_chains = Vec::new();

    for path in paths {
        // Parse the session
        let session = if let Some(fmt) = format {
            parse_session_with_format(path, fmt)
        } else {
            parse_session(path)
        };

        match session {
            Ok(s) => {
                let a = analyze_session(&s);
                session_count += 1;

                // Aggregate message counts
                for (k, v) in a.message_counts {
                    *aggregate.message_counts.entry(k).or_insert(0) += v;
                }

                // Aggregate tool stats
                for (k, v) in a.tool_stats {
                    let stat = aggregate
                        .tool_stats
                        .entry(k.clone())
                        .or_insert_with(|| ToolStats::new(&k));
                    stat.calls += v.calls;
                    stat.errors += v.errors;
                }

                // Aggregate token stats
                aggregate.token_stats.total_input += a.token_stats.total_input;
                aggregate.token_stats.total_output += a.token_stats.total_output;
                aggregate.token_stats.cache_read += a.token_stats.cache_read;
                aggregate.token_stats.cache_create += a.token_stats.cache_create;
                aggregate.token_stats.api_calls += a.token_stats.api_calls;
                if a.token_stats.min_context > 0 {
                    aggregate
                        .token_stats
                        .update_context(a.token_stats.min_context);
                }
                if a.token_stats.max_context > 0 {
                    aggregate
                        .token_stats
                        .update_context(a.token_stats.max_context);
                }

                // Aggregate file tokens
                for (k, v) in a.file_tokens {
                    *aggregate.file_tokens.entry(k).or_insert(0) += v;
                }

                aggregate.total_turns += a.total_turns;
                aggregate.parallel_opportunities += a.parallel_opportunities;

                // Collect tool chains for pattern analysis
                all_chains.extend(a.tool_chains);

                // Aggregate command stats by category
                for cs in a.command_stats {
                    if let Some(existing) = aggregate
                        .command_stats
                        .iter_mut()
                        .find(|s| s.category == cs.category)
                    {
                        existing.total_calls += cs.total_calls;
                        existing.total_errors += cs.total_errors;
                        existing.output_tokens += cs.output_tokens;
                        // Merge command details
                        for detail in cs.commands {
                            if let Some(ed) = existing
                                .commands
                                .iter_mut()
                                .find(|d| d.pattern == detail.pattern)
                            {
                                ed.calls += detail.calls;
                                ed.errors += detail.errors;
                            } else {
                                existing.commands.push(detail);
                            }
                        }
                    } else {
                        aggregate.command_stats.push(cs);
                    }
                }

                // Aggregate retry hotspots by pattern
                for rh in a.retry_hotspots {
                    if let Some(existing) = aggregate
                        .retry_hotspots
                        .iter_mut()
                        .find(|h| h.pattern == rh.pattern)
                    {
                        existing.attempts += rh.attempts;
                        existing.failures += rh.failures;
                        existing.output_tokens += rh.output_tokens;
                        existing.turn_indices.extend(rh.turn_indices);
                    } else {
                        aggregate.retry_hotspots.push(rh);
                    }
                }

                // Aggregate actual cost
                if let Some(cost) = a.actual_cost {
                    *aggregate.actual_cost.get_or_insert(0.0) += cost;
                }

                // Aggregate dedup token stats
                if let Some(dedup) = a.dedup_tokens {
                    let agg = aggregate
                        .dedup_tokens
                        .get_or_insert(DedupTokenStats::default());
                    agg.unique_input += dedup.unique_input;
                    agg.unique_output += dedup.unique_output;
                    agg.total_billed += dedup.total_billed;
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            }
        }
    }

    if session_count == 0 {
        return None;
    }

    // Sort aggregated command stats and details
    aggregate
        .command_stats
        .sort_by(|a, b| b.total_calls.cmp(&a.total_calls));
    for cs in &mut aggregate.command_stats {
        cs.commands.sort_by(|a, b| b.calls.cmp(&a.calls));
    }
    aggregate
        .retry_hotspots
        .sort_by(|a, b| b.failures.cmp(&a.failures));

    // Extract common tool patterns from all chains
    use crate::sessions::extract_tool_patterns;
    aggregate.tool_patterns = extract_tool_patterns(&all_chains);

    // Recompute uniqueness_ratio for aggregate dedup stats
    if let Some(dedup) = &mut aggregate.dedup_tokens
        && dedup.total_billed > 0
    {
        dedup.uniqueness_ratio =
            (dedup.unique_input + dedup.unique_output) as f64 / dedup.total_billed as f64;
    }

    // Update format to show aggregate info
    aggregate.format = format!("aggregate ({} sessions)", session_count);

    Some(aggregate)
}

/// Apply jq filter to each line of a JSONL file.
pub fn cmd_sessions_jq(path: &Path, filter: &str) -> i32 {
    use jaq_core::load::{Arena, File as JaqFile, Loader};
    use jaq_core::{Compiler, Ctx, RcIter};
    use jaq_json::Val;

    // Set up loader with standard library
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let arena = Arena::default();

    // Parse the filter
    let program = JaqFile {
        code: filter,
        path: (),
    };

    let modules = match loader.load(&arena, program) {
        Ok(m) => m,
        Err(errs) => {
            for e in errs {
                eprintln!("jq parse error: {:?}", e);
            }
            return 1;
        }
    };

    // Compile the filter
    let filter_compiled = match Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
    {
        Ok(f) => f,
        Err(errs) => {
            for e in errs {
                eprintln!("jq compile error: {:?}", e);
            }
            return 1;
        }
    };

    // Process each line
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
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Read error: {}", e);
                return 1;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let json_val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let val = Val::from(json_val);
        let inputs = RcIter::new(core::iter::empty());
        let out = filter_compiled.run((Ctx::new([], &inputs), val));

        for result in out {
            match result {
                Ok(v) => {
                    let _ = writeln!(stdout, "{}", v);
                }
                Err(e) => {
                    eprintln!("jq error: {:?}", e);
                }
            }
        }
    }

    0
}
