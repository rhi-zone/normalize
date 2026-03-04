#![allow(warnings, clippy::all, unexpected_cfgs)]
mod args;
mod debug_query;
mod error_context;
mod print_diff;
mod worker;

pub use args::{ContextArgs, InputArgs, OutputArgs, OverwriteArgs};
pub use debug_query::DebugFormat;
pub use error_context::{ErrorContext, exit_with_error};
pub use print_diff::DiffStyles;
pub use worker::{Items, MaxItemCounter, PathWorker, StdInWorker, Worker};

// Stub types for inspect.rs features (not vendored - used by scan/verify only)
#[derive(Clone, Debug, Default)]
pub struct FileTrace;
impl FileTrace {
    pub fn print_file(
        &self,
        _path: &std::path::Path,
        _lang: super::lang::Lang,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn add_scanned(&self) {}
    pub fn add_skipped(&self) {}
}
#[derive(Clone, Debug, Default)]
pub struct RunTrace {
    pub inner: FileTrace,
}
impl RunTrace {
    pub fn print(&self) -> anyhow::Result<()> {
        Ok(())
    }
    pub fn print_file(
        &self,
        path: &std::path::Path,
        lang: super::lang::Lang,
    ) -> anyhow::Result<()> {
        self.inner.print_file(path, lang)
    }
}
#[derive(Clone, Debug, Default)]
pub struct ScanTrace;
impl ScanTrace {
    pub fn print_file(
        &self,
        _path: &std::path::Path,
        _lang: super::lang::Lang,
        _rules: &[impl std::any::Any],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
#[derive(Clone, Debug, Default)]
pub struct RuleTrace {
    pub file_trace: ScanTrace,
    pub effective_rule_count: usize,
    pub skipped_rule_count: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Granularity;
impl Granularity {
    pub fn run_trace(&self) -> RunTrace {
        RunTrace::default()
    }
    pub fn project_trace(&self) -> ProjectTrace {
        ProjectTrace
    }
    pub fn scan_trace(&self) -> ScanTrace {
        ScanTrace
    }
}
impl std::str::FromStr for Granularity {
    type Err = String;
    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Ok(Self)
    }
}
#[derive(Clone, Debug, Default)]
pub struct ProjectTrace;
impl ProjectTrace {
    pub fn print_project<T>(&self, _project: &anyhow::Result<T>) -> anyhow::Result<()> {
        Ok(())
    }
}

use crate::ast_grep::lang::Lang;

use anyhow::{Context, Result, anyhow};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    terminal::{Clear, ClearType},
};
use smallvec::{SmallVec, smallvec};

use ast_grep_core::Pattern;
use ast_grep_core::language::Language;
use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_core::{Matcher, tree_sitter::StrDoc};

use std::fs::read_to_string;
use std::io::Write;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::str::FromStr;

type AstGrep = ast_grep_core::AstGrep<StrDoc<Lang>>;

fn read_char() -> Result<char> {
    loop {
        if let Event::Key(evt) = event::read()? {
            match evt.code {
                KeyCode::Tab => break Ok('\t'),
                KeyCode::Enter => break Ok('\n'),
                KeyCode::Char('c') if evt.modifiers.contains(KeyModifiers::CONTROL) => {
                    break Ok('q');
                }
                KeyCode::Char(c) => break Ok(c),
                _ => (),
            }
        }
    }
}

/// Prompts for user input on STDOUT
fn prompt_reply_stdout(prompt: &str) -> Result<char> {
    let mut stdout = std::io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;
    terminal::enable_raw_mode()?;
    let ret = read_char();
    terminal::disable_raw_mode()?;
    ret
}

// clear screen
pub fn clear() -> Result<()> {
    execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
    Ok(())
}

pub fn run_in_alternate_screen<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    execute!(stdout(), EnterAlternateScreen)?;
    clear()?;
    let ret = f();
    execute!(stdout(), LeaveAlternateScreen)?;
    ret
}

pub fn prompt(prompt_text: &str, letters: &str, default: Option<char>) -> Result<char> {
    loop {
        let input = prompt_reply_stdout(prompt_text)?;
        if let Some(default) = default {
            if input == '\n' {
                return Ok(default);
            }
        }
        if letters.contains(input) {
            return Ok(input);
        }
        eprintln!("Unrecognized command, try again?")
    }
}

pub(crate) fn read_file(path: &Path) -> Result<String> {
    let file_content = read_to_string(path)
        .with_context(|| format!("Cannot read file {}", path.to_string_lossy()))?;
    // skip large files or empty file
    if file_too_large(&file_content) {
        Err(anyhow!("File is too large"))
    } else if file_content.is_empty() {
        Err(anyhow!("File is empty"))
    } else {
        Ok(file_content)
    }
}

// sub_matchers are the injected languages
// e.g. js/css in html
pub fn filter_file_pattern<'a>(
    path: &Path,
    lang: Lang,
    root_matcher: Option<&'a Pattern>,
    sub_matchers: &'a [(Lang, Pattern)],
) -> Result<SmallVec<[MatchUnit<&'a Pattern>; 1]>> {
    let file_content = read_file(path)?;
    let grep = lang.ast_grep(&file_content);
    let do_match = |ast_grep: AstGrep, matcher: &'a Pattern| {
        let fixed = matcher.fixed_string();
        if !fixed.is_empty() && !file_content.contains(&*fixed) {
            return None;
        }
        Some(MatchUnit {
            grep: ast_grep,
            path: path.to_path_buf(),
            matcher,
        })
    };
    let mut ret = smallvec![];
    if let Some(matcher) = root_matcher {
        ret.extend(do_match(grep.clone(), matcher));
    }
    let injections = grep.get_injections(|s| Lang::from_str(s).ok());
    let sub_units = injections.into_iter().filter_map(|inner| {
        let (_, matcher) = sub_matchers.iter().find(|i| *inner.lang() == i.0)?;
        let injected = inner;
        do_match(injected, matcher)
    });
    ret.extend(sub_units);
    Ok(ret)
}

pub fn filter_file_rule(path: &Path, lang: Lang) -> Result<SmallVec<[AstGrep; 1]>> {
    let file_content = read_file(path)?;
    let grep = lang.ast_grep(file_content);
    let mut ret = smallvec![grep.clone()];
    let injections = grep.get_injections(|s| Lang::from_str(s).ok());
    for root in injections {
        ret.push(root);
    }
    Ok(ret)
}

const MAX_FILE_SIZE: usize = 3_000_000;
const MAX_LINE_COUNT: usize = 200_000;

// skip files that are too large in size AND have too many lines
fn file_too_large(file_content: &str) -> bool {
    // the && operator is intentional here to include more files
    file_content.len() > MAX_FILE_SIZE && file_content.lines().count() > MAX_LINE_COUNT
}

/// A single atomic unit where matches happen.
/// It contains the file path, sg instance and matcher.
/// An analogy to compilation unit in C programming language.
pub struct MatchUnit<M: Matcher> {
    pub path: PathBuf,
    pub grep: AstGrep,
    pub matcher: M,
}
