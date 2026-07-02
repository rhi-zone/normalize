//! Session analysis functions.

use crate::sessions::{
    SessionAnalysisReport, analyze_session, parse_session, parse_session_with_format,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Analyze a session and output statistics.
pub fn print_session_analysis(path: &Path, format: Option<&str>) -> i32 {
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
    println!("{}", analysis.format_text());
    0
}

/// Analyze multiple sessions and aggregate statistics, printing results.
pub fn print_sessions_analysis(paths: &[PathBuf], format: Option<&str>) -> i32 {
    match aggregate_sessions(paths, format) {
        Some(aggregate) => {
            println!("{}", aggregate.format_text());
            0
        }
        None => {
            eprintln!("No sessions could be analyzed");
            1
        }
    }
}

/// Aggregate multiple sessions into a single analysis. Returns None if no sessions could be parsed.
pub fn aggregate_sessions(
    paths: &[PathBuf],
    format: Option<&str>,
) -> Option<SessionAnalysisReport> {
    // Parse and analyze each session into a per-session report. The pure fold
    // over these reports lives with the model in `normalize-session-analysis`.
    let mut reports = Vec::new();
    for path in paths {
        let session = if let Some(fmt) = format {
            parse_session_with_format(path, fmt)
        } else {
            parse_session(path)
        };

        match session {
            Ok(s) => reports.push(analyze_session(&s)),
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            }
        }
    }

    if reports.is_empty() {
        return None;
    }

    Some(SessionAnalysisReport::aggregate(&reports))
}

/// Apply jq filter to each line of a JSONL file.
pub fn print_session_jq(path: &Path, filter: &str) -> i32 {
    use jaq_core::load::{Arena, File as JaqFile, Loader};
    use jaq_core::{Compiler, Ctx, Vars, data::JustLut};
    use jaq_json::Val;

    let loader = Loader::new(
        jaq_core::defs()
            .chain(jaq_std::defs())
            .chain(jaq_json::defs()),
    );
    let arena = Arena::default();

    let modules = match loader.load(
        &arena,
        JaqFile {
            code: filter,
            path: (),
        },
    ) {
        Ok(m) => m,
        Err(errs) => {
            for e in errs {
                eprintln!("jq parse error: {:?}", e);
            }
            return 1;
        }
    };

    let filter_compiled = match Compiler::default()
        .with_funs(
            jaq_core::funs::<JustLut<Val>>()
                .chain(jaq_std::funs::<JustLut<Val>>())
                .chain(jaq_json::funs::<JustLut<Val>>()),
        )
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

        let val: Val = match jaq_json::read::parse_single(line.as_bytes()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let ctx = Ctx::<JustLut<Val>>::new(&filter_compiled.lut, Vars::new([]));
        for result in filter_compiled.id.run((ctx, val)) {
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
