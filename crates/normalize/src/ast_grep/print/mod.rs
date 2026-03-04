#![allow(warnings, clippy::all, unexpected_cfgs)]
mod colored_print;
mod file_name_printer;
mod json_print;

use crate::ast_grep::lang::Lang;
use ast_grep_core::{Matcher, NodeMatch as SgNodeMatch, tree_sitter::StrDoc};

use anyhow::Result;
use clap::ValueEnum;

use std::borrow::Cow;
use std::path::Path;

pub use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::ColorChoice;
use colored_print::PrintStyles;
pub use colored_print::{ColoredPrinter, Heading, ReportStyle};
pub use file_name_printer::FileNamePrinter;
pub use json_print::{JSONPrinter, JsonStyle};

type NodeMatch<'a> = SgNodeMatch<'a, StrDoc<Lang>>;

/// A trait to process nodeMatches to diff/match output
/// it must be Send + 'static to be shared in worker thread
pub trait PrintProcessor<Output>: Send + Sync + 'static {
    fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Output>;
    fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Output>;
}

pub trait Printer {
    // processed item must be sent to printer thread
    type Processed: Send + 'static;
    type Processor: PrintProcessor<Self::Processed>;

    fn get_processor(&self) -> Self::Processor;
    /// Runs processed output from processor. This runs multiple times.
    fn process(&mut self, processed: Self::Processed) -> Result<()>;

    /// Run before all printing. One CLI will run this exactly once.
    #[inline]
    fn before_print(&mut self) -> Result<()> {
        Ok(())
    }
    /// Run after all printing. One CLI will run this exactly once.
    #[inline]
    fn after_print(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct AdditionalFix {
    pub replacement: String,
    pub range: std::ops::Range<usize>,
    pub title: Option<String>,
}

#[derive(Clone)]
pub struct Diff<'n> {
    /// the matched node
    pub node_match: NodeMatch<'n>,
    /// string content for the replacement
    pub replacement: String,
    pub range: std::ops::Range<usize>,
    pub title: Option<String>,
    pub additional_fixes: Option<Box<[AdditionalFix]>>,
}

impl<'n> Diff<'n> {
    pub fn into_list(mut self) -> Vec<Self> {
        let node_match = self.node_match.clone();
        let additional_fixes = self.additional_fixes.take();
        let mut ret = vec![self];
        ret.extend(additional_fixes.into_iter().flatten().map(|f| Self {
            node_match: node_match.clone(),
            replacement: f.replacement,
            range: f.range,
            additional_fixes: None,
            title: f.title,
        }));
        ret
    }

    /// Returns the root doc source code
    /// N.B. this can be different from node.text() because
    /// tree-sitter's root Node may not start at the begining
    pub fn get_root_text(&self) -> &'n str {
        self.node_match.root().get_text()
    }
}

#[derive(ValueEnum, Clone, Copy)]
pub enum ColorArg {
    /// Try to use colors, but don't force the issue. If the output is piped to another program,
    /// or the console isn't available on Windows, or if TERM=dumb, or if `NO_COLOR` is defined,
    /// for example, then don't use colors.
    Auto,
    /// Try very hard to emit colors. This includes emitting ANSI colors
    /// on Windows if the console API is unavailable (not implemented yet).
    Always,
    /// Ansi is like Always, except it never tries to use anything other
    /// than emitting ANSI color codes.
    Ansi,
    /// Never emit colors.
    Never,
}

impl ColorArg {
    pub fn should_use_color(self) -> bool {
        use colored_print::should_use_color;
        should_use_color(&self.into())
    }
}

impl From<ColorArg> for ColorChoice {
    fn from(arg: ColorArg) -> ColorChoice {
        use ColorArg::*;
        use std::io::IsTerminal;
        match arg {
            Auto => {
                if std::io::stdout().is_terminal() {
                    ColorChoice::Auto
                } else {
                    ColorChoice::Never
                }
            }
            Always => ColorChoice::Always,
            Ansi => ColorChoice::AlwaysAnsi,
            Never => ColorChoice::Never,
        }
    }
}
