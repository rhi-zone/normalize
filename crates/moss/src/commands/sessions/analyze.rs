//! Session analysis functions.

use crate::sessions::{
    SessionAnalysis, ToolStats, analyze_session, parse_session, parse_session_with_format,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Analyze a session and output statistics.
pub fn cmd_sessions_analyze(path: &Path, format: Option<&str>, json: bool, pretty: bool) -> i32 {
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

    if json {
        println!("{}", serde_json::to_string_pretty(&analysis).unwrap());
    } else if pretty {
        println!("{}", analysis.format_pretty());
    } else {
        println!("{}", analysis.format_text());
    }
    0
}

/// Analyze multiple sessions and aggregate statistics.
pub fn cmd_sessions_analyze_multi(
    paths: &[PathBuf],
    format: Option<&str>,
    json: bool,
    pretty: bool,
) -> i32 {
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
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            }
        }
    }

    if session_count == 0 {
        eprintln!("No sessions could be analyzed");
        return 1;
    }

    // Extract common tool patterns from all chains
    use crate::sessions::extract_tool_patterns;
    aggregate.tool_patterns = extract_tool_patterns(&all_chains);

    // Update format to show aggregate info
    aggregate.format = format!("aggregate ({} sessions)", session_count);

    if json {
        println!("{}", serde_json::to_string_pretty(&aggregate).unwrap());
    } else if pretty {
        println!("{}", aggregate.format_pretty());
    } else {
        println!("{}", aggregate.format_text());
    }

    0
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
