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
pub struct Granularity;
impl Granularity {
    pub fn run_trace(&self) -> RunTrace {
        RunTrace::default()
    }
    pub fn project_trace(&self) -> ProjectTrace {
        ProjectTrace
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
use smallvec::{SmallVec, smallvec};

use ast_grep_core::Pattern;
use ast_grep_core::language::Language;
use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_core::{Matcher, tree_sitter::StrDoc};

use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;

type AstGrep = ast_grep_core::AstGrep<StrDoc<Lang>>;

fn read_file(path: &Path) -> Result<String> {
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
