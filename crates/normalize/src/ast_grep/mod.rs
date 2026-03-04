// ast-grep subcommand — embeds ast-grep-core as a drop-in `ast-grep`/`sg` replacement.
mod cli;

use ast_grep_core::{Pattern, tree_sitter::LanguageExt};
use cli::Cli;
use normalize_languages::{GrammarLoader, ast_grep::DynLang};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;

/// Run ast-grep with the given arguments (not including argv[0]).
///
/// Entry point for both `normalize ast-grep [args...]` and `sg -> normalize` symlink.
pub fn run_ast_grep(args: impl Iterator<Item = OsString>) -> ExitCode {
    let mut out = io::stdout();
    let mut err = io::stderr();

    let cli = match Cli::parse(args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(2);
        }
    };

    if cli.version {
        let _ = writeln!(
            out,
            "ast-grep (normalize {}) [ast-grep-core]",
            env!("CARGO_PKG_VERSION")
        );
        return ExitCode::SUCCESS;
    }
    if cli.help {
        let _ = write!(out, "{}", include_str!("help.txt"));
        return ExitCode::SUCCESS;
    }

    match real_main(&cli) {
        Ok(found) => {
            if found {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            ExitCode::from(2)
        }
    }
}

fn grammar_loader() -> &'static GrammarLoader {
    static LOADER: OnceLock<GrammarLoader> = OnceLock::new();
    LOADER.get_or_init(GrammarLoader::new)
}

fn real_main(cli: &Cli) -> Result<bool, String> {
    let pattern_str = cli
        .pattern
        .as_deref()
        .ok_or("No pattern given. Use --pattern/-p PATTERN.")?;

    let paths = search_paths(cli);
    let mut any_match = false;

    let is_tty = io::stdout().is_terminal();
    let use_color = std::env::var("NO_COLOR").is_err() && is_tty;

    for path in walk_paths(&paths, cli) {
        let path = match path {
            Ok(p) => p,
            Err(e) => {
                let _ = writeln!(io::stderr(), "error: {e}");
                continue;
            }
        };

        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                let _ = writeln!(io::stderr(), "{}: {e}", path.display());
                continue;
            }
        };

        // Determine language
        let lang = if let Some(lang_name) = &cli.lang {
            get_lang(lang_name)
        } else {
            get_lang_for_path(&path)
        };

        let lang = match lang {
            Some(l) => l,
            None => continue, // skip files we don't know how to parse
        };

        // Compile pattern for this language
        let pattern = match Pattern::try_new(pattern_str, lang.clone()) {
            Ok(p) => p,
            Err(e) => {
                let _ = writeln!(io::stderr(), "{}: pattern error: {}", path.display(), e);
                continue;
            }
        };

        // Parse and search
        let grep = lang.ast_grep(&src);
        let root = grep.root();
        let matches: Vec<_> = root.find_all(&pattern).collect();

        if matches.is_empty() {
            continue;
        }

        any_match = true;

        if cli.files_with_matches {
            let sep = "\n";
            let _ = write!(io::stdout(), "{}{sep}", path.display());
            continue;
        }

        let path_str = path.display().to_string();

        if cli.json {
            for m in &matches {
                let range = m.range();
                let start = byte_offset_to_line_col(&src, range.start);
                let end = byte_offset_to_line_col(&src, range.end);
                let json = serde_json::json!({
                    "file": path_str,
                    "start": { "line": start.0, "column": start.1, "byte_offset": range.start },
                    "end": { "line": end.0, "column": end.1, "byte_offset": range.end },
                    "text": m.text(),
                });
                println!("{json}");
            }
        } else {
            let show_filename = !cli.no_filename;

            // Group by file with heading (like rg --heading in tty mode)
            if show_filename && is_tty {
                if use_color {
                    println!("\x1b[35m{path_str}\x1b[0m");
                } else {
                    println!("{path_str}");
                }
            }

            for m in &matches {
                let range = m.range();
                let (line_no, col) = byte_offset_to_line_col(&src, range.start);
                let line = get_line(&src, line_no.saturating_sub(1));
                if use_color {
                    if show_filename && !is_tty {
                        print!("\x1b[35m{path_str}\x1b[0m:");
                    }
                    print!("\x1b[32m{line_no}\x1b[0m:\x1b[32m{col}\x1b[0m:");
                    // Highlight the match in the line
                    let line_start = src[..range.start].rfind('\n').map_or(0, |i| i + 1);
                    let match_col_start = range.start - line_start;
                    let match_col_end = (range.end - line_start).min(line.len());
                    if match_col_start < line.len() && match_col_end <= line.len() {
                        print!("{}", &line[..match_col_start]);
                        print!("\x1b[1;31m{}\x1b[0m", &line[match_col_start..match_col_end]);
                        println!("{}", &line[match_col_end..]);
                    } else {
                        println!("{line}");
                    }
                } else {
                    if show_filename && !is_tty {
                        print!("{path_str}:");
                    }
                    println!("{line_no}:{col}:{line}");
                }
            }
        }
    }

    Ok(any_match)
}

fn get_lang(name: &str) -> Option<DynLang> {
    let loader = grammar_loader();
    // Try exact name first, then common aliases
    let name_lower = name.to_lowercase();
    let grammar_name = match name_lower.as_str() {
        "js" | "javascript" => "javascript",
        "ts" | "typescript" => "typescript",
        "tsx" => "tsx",
        "jsx" => "javascript",
        "py" | "python" => "python",
        "rs" | "rust" => "rust",
        "go" | "golang" => "go",
        "rb" | "ruby" => "ruby",
        "java" => "java",
        "kt" | "kotlin" => "kotlin",
        "swift" => "swift",
        "c" => "c",
        "cpp" | "cc" | "cxx" | "c++" => "cpp",
        "cs" | "csharp" | "c#" => "c-sharp",
        "php" => "php",
        "sh" | "bash" | "shell" => "bash",
        "lua" => "lua",
        "r" => "r",
        "scala" => "scala",
        "hs" | "haskell" => "haskell",
        "ml" | "ocaml" => "ocaml",
        "ex" | "elixir" => "elixir",
        "erl" | "erlang" => "erlang",
        "clj" | "clojure" => "clojure",
        "dart" => "dart",
        other => other,
    };
    loader.get(grammar_name).map(DynLang::new)
}

fn get_lang_for_path(path: &Path) -> Option<DynLang> {
    let lang_support = normalize_languages::support_for_path(path)?;
    let grammar_name = lang_support.grammar_name();
    grammar_loader().get(grammar_name).map(DynLang::new)
}

fn search_paths(cli: &Cli) -> Vec<PathBuf> {
    if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths.clone()
    }
}

fn walk_paths<'a>(
    paths: &'a [PathBuf],
    cli: &'a Cli,
) -> impl Iterator<Item = Result<PathBuf, ignore::Error>> + 'a {
    let mut builder = ignore::WalkBuilder::new(&paths[0]);
    for p in &paths[1..] {
        builder.add(p);
    }
    builder
        .hidden(!cli.hidden)
        .git_ignore(!cli.no_ignore)
        .git_global(!cli.no_ignore)
        .git_exclude(!cli.no_ignore)
        .ignore(!cli.no_ignore)
        .follow_links(cli.follow)
        .max_depth(cli.max_depth);

    builder
        .build()
        .filter_map(|entry| match entry {
            Ok(e) => {
                if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    Some(Ok(e.into_path()))
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        })
        .collect::<Vec<_>>()
        .into_iter()
}

/// Convert a byte offset to 1-based (line, column).
fn byte_offset_to_line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let before = &src[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = before.rfind('\n').map_or(offset, |i| offset - i - 1) + 1;
    (line, col)
}

/// Get the content of line N (0-based index).
fn get_line(src: &str, line_idx: usize) -> &str {
    src.lines().nth(line_idx).unwrap_or("")
}

use std::io::IsTerminal;
