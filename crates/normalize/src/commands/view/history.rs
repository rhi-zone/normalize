//! Symbol history via git log.

use super::report::{ViewHistoryCommit, ViewHistoryReport, ViewOutput};
use super::symbol::find_symbol_ci;
use crate::output::OutputFormatter;
use crate::path_resolve;
use crate::skeleton;
use std::path::Path;
use std::process::Command;

/// Show git history for a symbol.
pub fn cmd_history(
    target: Option<&str>,
    root: &Path,
    limit: usize,
    case_insensitive: bool,
    format: &crate::output::OutputFormat,
) -> i32 {
    let Some(target) = target else {
        eprintln!("--history requires a target (file/symbol path)");
        return 1;
    };

    // Parse the target path
    let Some(resolved) = path_resolve::resolve_unified(target, root) else {
        eprintln!("Could not resolve path: {}", target);
        return 1;
    };

    // We need a file with a symbol
    let file_path = resolved.file_path;
    let symbol_path = resolved.symbol_path;

    let symbol_name = symbol_path.first().cloned();

    let full_path = root.join(&file_path);
    if !full_path.exists() {
        eprintln!("File not found: {}", full_path.display());
        return 1;
    }

    // Read file and extract skeleton to find symbol range
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read {}: {}", full_path.display(), e);
            return 1;
        }
    };

    let (start_line, end_line) = if let Some(ref sym_name) = symbol_name {
        // Find the symbol
        let extractor = skeleton::SkeletonExtractor::new();
        let result = extractor.extract(&full_path, &content);

        let found = if symbol_path.len() > 1 {
            find_symbol_by_path(&result.symbols, &symbol_path, case_insensitive)
        } else {
            find_symbol_ci(&result.symbols, sym_name, case_insensitive)
        };

        match found {
            Some(sym) => (sym.start_line, sym.end_line),
            None => {
                eprintln!("Symbol '{}' not found in {}", sym_name, file_path);
                return 1;
            }
        }
    } else if !symbol_path.is_empty() {
        eprintln!("Symbol not found");
        return 1;
    } else {
        // Whole file history
        let line_count = content.lines().count();
        (1, line_count)
    };

    // Run git log for changes to these lines
    show_line_history(root, &file_path, start_line, end_line, limit, format)
}

/// Find symbol by path (parent/child).
fn find_symbol_by_path<'a>(
    symbols: &'a [skeleton::SkeletonSymbol],
    path: &[String],
    case_insensitive: bool,
) -> Option<&'a skeleton::SkeletonSymbol> {
    if path.is_empty() {
        return None;
    }

    if path.len() == 1 {
        return find_symbol_ci(symbols, &path[0], case_insensitive);
    }

    fn names_match(a: &str, b: &str, ci: bool) -> bool {
        if ci {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    }

    let mut current_symbols = symbols;
    for (i, name) in path.iter().enumerate() {
        let found = current_symbols
            .iter()
            .find(|s| names_match(&s.name, name, case_insensitive))?;
        if i == path.len() - 1 {
            return Some(found);
        }
        current_symbols = &found.children;
    }
    None
}

/// Show git history for a line range.
fn show_line_history(
    root: &Path,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    limit: usize,
    format: &crate::output::OutputFormat,
) -> i32 {
    let json = format.is_json();
    // Use git log -L to show history for line range
    let line_range = format!("{},{}:{}", start_line, end_line, file_path);

    let output = match Command::new("git")
        .current_dir(root)
        .args([
            "log",
            "-L",
            &line_range,
            "--no-patch",
            &format!("-{}", limit),
            "--format=%H%x1f%an%x1f%as%x1f%s",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to run git log: {}", e);
            return 1;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("git log failed: {}", stderr);
        return 1;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if json {
        let commits: Vec<ViewHistoryCommit> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\x1f').collect();
                if parts.len() >= 4 {
                    Some(ViewHistoryCommit {
                        hash: parts[0].to_string(),
                        author: parts[1].to_string(),
                        date: parts[2].to_string(),
                        message: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        let report = ViewOutput::History(ViewHistoryReport {
            file: file_path.to_string(),
            lines: format!("{}-{}", start_line, end_line),
            commits,
        });
        report.print(format);
    } else {
        println!("History for {} (L{}-L{}):", file_path, start_line, end_line);
        println!();

        let commits: Vec<_> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\x1f').collect();
                if parts.len() >= 4 {
                    Some((parts[0], parts[1], parts[2], parts[3]))
                } else {
                    None
                }
            })
            .collect();

        if commits.is_empty() {
            println!("  No history found.");
        } else {
            for (hash, author, date, message) in commits {
                println!("  {} {} {} {}", &hash[..8], date, author, message);
            }
        }
    }

    0
}

/// Build history view for the service layer.
pub fn build_view_history_service(
    target: &str,
    root: &Path,
    limit: usize,
    case_insensitive: bool,
) -> Result<ViewOutput, String> {
    let Some(resolved) = crate::path_resolve::resolve_unified(target, root) else {
        return Err(format!("Could not resolve path: {}", target));
    };

    let file_path = resolved.file_path;
    let symbol_path = resolved.symbol_path;
    let symbol_name = symbol_path.first().cloned();

    let full_path = root.join(&file_path);
    if !full_path.exists() {
        return Err(format!("File not found: {}", full_path.display()));
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read {}: {}", full_path.display(), e))?;

    let (start_line, end_line) = if let Some(ref sym_name) = symbol_name {
        let extractor = crate::skeleton::SkeletonExtractor::new();
        let result = extractor.extract(&full_path, &content);

        let found = if symbol_path.len() > 1 {
            find_symbol_by_path(&result.symbols, &symbol_path, case_insensitive)
        } else {
            find_symbol_ci(&result.symbols, sym_name, case_insensitive)
        };

        match found {
            Some(sym) => (sym.start_line, sym.end_line),
            None => return Err(format!("Symbol '{}' not found in {}", sym_name, file_path)),
        }
    } else if !symbol_path.is_empty() {
        return Err("Symbol not found".to_string());
    } else {
        let line_count = content.lines().count();
        (1, line_count)
    };

    build_line_history_service(root, &file_path, start_line, end_line, limit)
}

/// Build history for a line range (service layer).
fn build_line_history_service(
    root: &Path,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    limit: usize,
) -> Result<ViewOutput, String> {
    let line_range = format!("{},{}:{}", start_line, end_line, file_path);

    let output = Command::new("git")
        .current_dir(root)
        .args([
            "log",
            "-L",
            &line_range,
            "--no-patch",
            &format!("-{}", limit),
            "--format=%H%x1f%an%x1f%as%x1f%s",
        ])
        .output()
        .map_err(|e| format!("Failed to run git log: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let commits: Vec<ViewHistoryCommit> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\x1f').collect();
            if parts.len() >= 4 {
                Some(ViewHistoryCommit {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(ViewOutput::History(ViewHistoryReport {
        file: file_path.to_string(),
        lines: format!("{}-{}", start_line, end_line),
        commits,
    }))
}
