// rg subcommand — embeds grep-* as a drop-in `rg` (ripgrep) replacement.
mod cli;

use cli::Cli;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{
    Searcher, SearcherBuilder, Sink, SinkContext, SinkContextKind, SinkFinish, SinkMatch,
};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// Run rg with the given arguments (not including argv[0]).
///
/// Entry point for both `normalize rg [args...]` and `rg -> normalize` symlink.
pub fn run_rg(args: impl Iterator<Item = OsString>) -> ExitCode {
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
            "rg (normalize {}) [grep-searcher]",
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

fn real_main(cli: &Cli) -> Result<bool, String> {
    let color = cli.use_color(&io::stdout());

    // --files: list files that would be searched, no pattern needed
    if cli.files {
        let paths = search_paths(cli);
        let mut out = io::stdout();
        for path in walk_paths(&paths, cli) {
            let p = path.map_err(|e| e.to_string())?;
            let sep = if cli.null { "\0" } else { "\n" };
            let _ = write!(out, "{}{sep}", p.display());
        }
        return Ok(true);
    }

    // Determine patterns
    let patterns: Vec<&str> = if !cli.patterns.is_empty() {
        cli.patterns.iter().map(String::as_str).collect()
    } else if let Some(p) = &cli.pattern {
        vec![p.as_str()]
    } else {
        return Err(
            "No pattern given. Use -e PATTERN or provide PATTERN as first argument.".to_string(),
        );
    };

    // Build matcher
    let pattern = if patterns.len() == 1 {
        patterns[0].to_string()
    } else {
        patterns
            .iter()
            .map(|p| format!("(?:{p})"))
            .collect::<Vec<_>>()
            .join("|")
    };

    let effective_ignore_case =
        cli.ignore_case || (cli.smart_case && pattern.chars().all(|c| !c.is_uppercase()));

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(effective_ignore_case)
        .word(cli.word_regexp)
        .fixed_strings(cli.fixed_strings)
        .multi_line(cli.multiline)
        .dot_matches_new_line(cli.multiline)
        .build(&pattern)
        .map_err(|e| format!("Invalid pattern: {e}"))?;

    // Build searcher
    let searcher = SearcherBuilder::new()
        .line_number(true) // always track; sink decides whether to show
        .before_context(if cli.invert_match {
            0
        } else {
            cli.before_context
        })
        .after_context(if cli.invert_match {
            0
        } else {
            cli.after_context
        })
        .passthru(cli.invert_match)
        .build();

    let paths = search_paths(cli);
    let multiple_paths = has_multiple_paths(&paths);

    // Auto-detect display options based on TTY
    let is_tty = io::stdout().is_terminal();
    let show_line_number = cli.line_number.unwrap_or(is_tty);
    let no_heading = cli.no_heading.unwrap_or(!is_tty);
    let show_filename = cli.with_filename.unwrap_or(multiple_paths);

    let mut any_match = false;
    let mut total_matches: u64 = 0;
    let mut files_searched: u64 = 0;
    let mut files_with_match: u64 = 0;

    let mut out = io::stdout();

    for entry in walk_paths(&paths, cli) {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                let _ = writeln!(io::stderr(), "error: {e}");
                continue;
            }
        };

        files_searched += 1;

        let path_str = path.to_string_lossy().into_owned();
        let effective_show_filename = show_filename || !no_heading;

        let mut sink = RgSink::new(
            &path_str,
            effective_show_filename,
            no_heading,
            show_line_number,
            cli.count,
            cli.files_with_matches,
            cli.files_without_match,
            cli.quiet,
            cli.null,
            cli.max_count,
            cli.before_context,
            cli.after_context,
            cli.invert_match,
            cli.only_matching,
            color,
        );

        let result = if path_str == "-" || path_str == "/dev/stdin" {
            searcher
                .clone()
                .search_reader(&matcher, io::stdin().lock(), &mut sink)
        } else {
            searcher.clone().search_path(&matcher, &path, &mut sink)
        };

        if let Err(e) = result {
            let _ = writeln!(io::stderr(), "{path_str}: {e}");
            continue;
        }

        if sink.had_match {
            any_match = true;
            files_with_match += 1;
        }
        total_matches += sink.match_count;

        // Flush sink output
        let _ = out.write_all(&sink.buf);

        // Print heading after file output (for --heading mode with multiple files)
        // Heading (filename separator) is already handled in the sink.

        // For --files-without-match: handled inside sink
        if cli.files_without_match && !sink.had_match {
            let sep = if cli.null { "\0" } else { "\n" };
            let _ = write!(out, "{path_str}{sep}");
        }
    }

    if cli.stats {
        let _ = writeln!(
            out,
            "\n{total_matches} matches\n{files_with_match} matched lines\n{files_searched} files searched"
        );
    }

    Ok(any_match)
}

fn search_paths(cli: &Cli) -> Vec<PathBuf> {
    if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths.clone()
    }
}

fn has_multiple_paths(paths: &[PathBuf]) -> bool {
    if paths.len() > 1 {
        return true;
    }
    if paths.len() == 1 && paths[0].is_dir() {
        return true;
    }
    false
}

fn walk_paths<'a>(
    paths: &'a [PathBuf],
    cli: &'a Cli,
) -> impl Iterator<Item = Result<PathBuf, ignore::Error>> + 'a {
    // If a single path is "-", return stdin marker
    if paths.len() == 1 && paths[0].as_os_str() == "-" {
        return WalkIter::Stdin(std::iter::once(Ok(PathBuf::from("-"))));
    }

    let mut builder = ignore::WalkBuilder::new(&paths[0]);
    for p in &paths[1..] {
        builder.add(p);
    }
    builder
        .hidden(!cli.hidden)
        .git_ignore(!cli.no_ignore && !cli.no_ignore_vcs)
        .git_global(!cli.no_ignore)
        .git_exclude(!cli.no_ignore)
        .ignore(!cli.no_ignore)
        .follow_links(cli.follow)
        .max_depth(cli.max_depth);

    if !cli.globs.is_empty() {
        let mut override_builder = ignore::overrides::OverrideBuilder::new(".");
        for glob in &cli.globs {
            let _ = override_builder.add(glob);
        }
        if let Ok(overrides) = override_builder.build() {
            builder.overrides(overrides);
        }
    }

    WalkIter::Walk(
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
            .into_iter(),
    )
}

// Helper enum to unify iterator types
enum WalkIter<A, B> {
    Stdin(A),
    Walk(B),
}

impl<A, B, T> Iterator for WalkIter<A, B>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            WalkIter::Stdin(a) => a.next(),
            WalkIter::Walk(b) => b.next(),
        }
    }
}

// ANSI color codes
const COLOR_PATH: &str = "\x1b[35m"; // magenta
const COLOR_LINE: &str = "\x1b[32m"; // green
const _COLOR_MATCH: &str = "\x1b[1;31m"; // bold red (reserved for future match highlighting)
const COLOR_SEP: &str = "\x1b[36m"; // cyan
const COLOR_RESET: &str = "\x1b[0m";

struct RgSink {
    path: String,
    show_filename: bool,
    no_heading: bool,
    show_line_number: bool,
    count_only: bool,
    files_with_matches: bool,
    files_without_match: bool,
    quiet: bool,
    null: bool,
    max_count: Option<u64>,
    before_context: usize,
    after_context: usize,
    invert_match: bool,
    only_matching: bool,
    color: bool,

    // State
    pub match_count: u64,
    pub had_match: bool,
    printed_heading: bool,

    // Output buffer (flushed after each file)
    pub buf: Vec<u8>,
}

impl RgSink {
    #[allow(clippy::too_many_arguments)]
    fn new(
        path: &str,
        show_filename: bool,
        no_heading: bool,
        show_line_number: bool,
        count_only: bool,
        files_with_matches: bool,
        files_without_match: bool,
        quiet: bool,
        null: bool,
        max_count: Option<u64>,
        before_context: usize,
        after_context: usize,
        invert_match: bool,
        only_matching: bool,
        color: bool,
    ) -> Self {
        Self {
            path: path.to_string(),
            show_filename,
            no_heading,
            show_line_number,
            count_only,
            files_with_matches,
            files_without_match,
            quiet,
            null,
            max_count,
            before_context,
            after_context,
            invert_match,
            only_matching,
            color,
            match_count: 0,
            had_match: false,
            printed_heading: false,
            buf: Vec::new(),
        }
    }

    fn print_heading(&mut self) {
        if self.printed_heading || self.no_heading || !self.show_filename {
            return;
        }
        if self.color {
            let _ = write!(self.buf, "\n{COLOR_PATH}{}{COLOR_RESET}\n", self.path);
        } else {
            let _ = writeln!(self.buf, "\n{}", self.path);
        }
        self.printed_heading = true;
    }

    fn print_line(
        &mut self,
        bytes: &[u8],
        line_num: Option<u64>,
        is_match: bool,
    ) -> io::Result<()> {
        if self.quiet || self.count_only || self.files_with_matches || self.files_without_match {
            return Ok(());
        }

        let line = String::from_utf8_lossy(bytes);
        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        let sep = if is_match { ':' } else { '-' };

        if !self.no_heading && self.show_filename {
            self.print_heading();
        }

        if self.no_heading && self.show_filename {
            if self.color {
                write!(self.buf, "{COLOR_PATH}{}{COLOR_RESET}{sep}", self.path)?;
            } else {
                write!(self.buf, "{}{sep}", self.path)?;
            }
        }

        if self.show_line_number
            && let Some(n) = line_num
        {
            let num_str = if self.color {
                format!("{COLOR_LINE}{n}{COLOR_RESET}{sep}")
            } else {
                format!("{n}{sep}")
            };
            self.buf.extend_from_slice(num_str.as_bytes());
        }

        writeln!(self.buf, "{line}")?;
        Ok(())
    }

    fn at_limit(&self) -> bool {
        self.max_count.is_some_and(|m| self.match_count >= m)
    }
}

impl Sink for RgSink {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
        if self.invert_match {
            // In passthru mode: matched lines are what we want to SKIP
            return Ok(!self.at_limit());
        }

        self.had_match = true;
        self.match_count += 1;

        if !self.quiet && !self.count_only && !self.files_with_matches && !self.files_without_match
        {
            if self.only_matching {
                // Print only the matched portion; re-find the match span
                // We print the full line for now since we don't have the span here
                // TODO: use matcher to re-find span for exact -o output
                self.print_line(mat.bytes(), mat.line_number(), true)?;
            } else {
                self.print_line(mat.bytes(), mat.line_number(), true)?;
            }
        }

        Ok(!self.at_limit())
    }

    fn context(&mut self, _searcher: &Searcher, ctx: &SinkContext<'_>) -> Result<bool, io::Error> {
        if self.invert_match {
            // In passthru mode: context lines are non-matching lines — print them
            self.had_match = true;
            self.match_count += 1;

            if !self.quiet
                && !self.count_only
                && !self.files_with_matches
                && !self.files_without_match
            {
                self.print_line(ctx.bytes(), ctx.line_number(), true)?;
            }

            return Ok(!self.at_limit());
        }

        // Normal context (before/after a match)
        if self.before_context > 0 || self.after_context > 0 {
            let is_before = matches!(ctx.kind(), SinkContextKind::Before);
            let _ = is_before; // used for future color distinction
            self.print_line(ctx.bytes(), ctx.line_number(), false)?;
        }
        Ok(true)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> Result<bool, io::Error> {
        if !self.quiet
            && !self.count_only
            && !self.files_with_matches
            && !self.files_without_match
            && !self.invert_match
            && (self.before_context > 0 || self.after_context > 0)
        {
            if self.color {
                let _ = writeln!(self.buf, "{COLOR_SEP}--{COLOR_RESET}");
            } else {
                let _ = writeln!(self.buf, "--");
            }
        }
        Ok(true)
    }

    fn finish(&mut self, _searcher: &Searcher, _fin: &SinkFinish) -> Result<(), io::Error> {
        if self.count_only && !self.quiet {
            if self.show_filename {
                writeln!(self.buf, "{}:{}", self.path, self.match_count)?;
            } else {
                writeln!(self.buf, "{}", self.match_count)?;
            }
        }
        if self.files_with_matches && self.had_match {
            let sep = if self.null { "\0" } else { "\n" };
            write!(self.buf, "{}{sep}", self.path)?;
        }
        Ok(())
    }
}
