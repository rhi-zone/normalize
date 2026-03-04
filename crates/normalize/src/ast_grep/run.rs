#![allow(warnings, clippy::all, unexpected_cfgs)]
// Vendored from ast-grep 0.41.0 (MIT)
// Modified: SgLang → Lang, removed Fixer/rewrite, removed InteractivePrinter,
// removed ProjectConfig dependency.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use ast_grep_core::language::Language;
use ast_grep_core::tree_sitter::LanguageExt;
use ast_grep_core::{MatchStrictness, Matcher, Pattern};
use clap::{Parser, ValueEnum, builder::PossibleValue};
use ignore::WalkParallel;

use crate::ast_grep::lang::Lang;
use crate::ast_grep::print::{
    ColoredPrinter, Diff, FileNamePrinter, Heading, JSONPrinter, PrintProcessor, Printer,
};
use crate::ast_grep::utils::ErrorContext as EC;
use crate::ast_grep::utils::{ContextArgs, InputArgs, MatchUnit, OutputArgs, filter_file_pattern};
use crate::ast_grep::utils::{DebugFormat, FileTrace, RunTrace};
use crate::ast_grep::utils::{Items, PathWorker, StdInWorker, Worker};

fn lang_help() -> String {
    format!(
        "The language of the pattern. Supported languages are:\n{:?}",
        Lang::all_langs()
    )
}

const LANG_HELP_LONG: &str = "The language of the pattern. For full language list, visit https://ast-grep.github.io/reference/languages.html";

#[derive(Clone)]
struct Strictness(MatchStrictness);
impl ValueEnum for Strictness {
    fn value_variants<'a>() -> &'a [Self] {
        use MatchStrictness as M;
        &[
            Strictness(M::Cst),
            Strictness(M::Smart),
            Strictness(M::Ast),
            Strictness(M::Relaxed),
            Strictness(M::Signature),
            Strictness(M::Template),
        ]
    }
    fn to_possible_value(&self) -> Option<PossibleValue> {
        use MatchStrictness as M;
        Some(match &self.0 {
            M::Cst => PossibleValue::new("cst").help("Match exact all node"),
            M::Smart => {
                PossibleValue::new("smart").help("Match all node except source trivial nodes")
            }
            M::Ast => PossibleValue::new("ast").help("Match only ast nodes"),
            M::Relaxed => PossibleValue::new("relaxed").help("Match ast node except comments"),
            M::Signature => {
                PossibleValue::new("signature").help("Match ast node except comments, without text")
            }
            M::Template => PossibleValue::new("template")
                .help("Similar to smart but match text only, node kinds are ignored"),
        })
    }
}

#[derive(Parser)]
pub struct RunArg {
    // search pattern related options
    /// AST pattern to match.
    #[clap(short, long)]
    pattern: String,

    /// AST kind to extract sub-part of pattern to match.
    #[clap(long, value_name = "KIND")]
    selector: Option<String>,

    /// The language of the pattern query.
    #[clap(short, long, help(lang_help()), long_help=LANG_HELP_LONG)]
    lang: Option<Lang>,

    /// Print query pattern's tree-sitter AST. Requires lang be set explicitly.
    #[clap(
      long,
      requires = "lang",
      value_name="format",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pattern"
  )]
    debug_query: Option<DebugFormat>,

    /// The strictness of the pattern.
    #[clap(long)]
    strictness: Option<Strictness>,

    /// input related options
    #[clap(flatten)]
    input: InputArgs,

    /// output related options
    #[clap(flatten)]
    output: OutputArgs,

    /// context related options
    #[clap(flatten)]
    context: ContextArgs,

    /// Controls whether to print the file name as heading.
    #[clap(long, default_value = "auto", value_name = "WHEN")]
    heading: Heading,
}

impl RunArg {
    fn build_pattern(&self, lang: Lang) -> Result<Pattern> {
        let pattern = if let Some(sel) = &self.selector {
            Pattern::contextual(&self.pattern, sel, lang)
        } else {
            Pattern::try_new(&self.pattern, lang)
        }
        .context(EC::ParsePattern)?;
        if let Some(strictness) = &self.strictness {
            Ok(pattern.with_strictness(strictness.0.clone()))
        } else {
            Ok(pattern)
        }
    }

    fn debug_pattern_if_needed(&self, pattern_ret: &Result<Pattern>, lang: Lang) {
        let Some(debug_query) = &self.debug_query else {
            return;
        };
        let colored = self.output.color.should_use_color();
        if !matches!(debug_query, DebugFormat::Pattern) {
            debug_query.debug_tree(&self.pattern, lang, colored);
        } else if let Ok(pattern) = pattern_ret {
            debug_query.debug_pattern(pattern, lang, colored);
        }
    }
}

pub fn run_with_pattern(arg: RunArg) -> Result<ExitCode> {
    let context = arg.context.get();
    if arg.output.files_with_matches {
        let printer = FileNamePrinter::stdout(arg.output.color);
        return run_pattern_with_printer(arg, printer);
    }
    if let Some(json) = arg.output.json {
        let printer = JSONPrinter::stdout(json).context(context);
        return run_pattern_with_printer(arg, printer);
    }
    let printer = ColoredPrinter::stdout(arg.output.color)
        .heading(arg.heading)
        .context(context);
    run_pattern_with_printer(arg, printer)
}

fn run_pattern_with_printer(arg: RunArg, printer: impl Printer + 'static) -> Result<ExitCode> {
    let trace = arg.output.inspect.run_trace();
    if arg.input.stdin {
        RunWithSpecificLang::new(arg, trace)?.run_std_in(printer)
    } else if arg.lang.is_some() {
        RunWithSpecificLang::new(arg, trace)?.run_path(printer)
    } else {
        RunWithInferredLang { arg, trace }.run_path(printer)
    }
}

struct RunWithInferredLang {
    arg: RunArg,
    trace: RunTrace,
}
impl Worker for RunWithInferredLang {
    fn consume_items<P: Printer>(
        &self,
        items: Items<P::Processed>,
        mut printer: P,
    ) -> Result<ExitCode> {
        let printer = &mut printer;
        let mut has_matches = false;
        printer.before_print()?;
        for item in items {
            printer.process(item)?;
            has_matches = true
        }
        printer.after_print()?;
        self.trace.print()?;
        Ok(ExitCode::from(if has_matches { 0 } else { 1 }))
    }
}

impl PathWorker for RunWithInferredLang {
    fn build_walk(&self) -> Result<WalkParallel> {
        self.arg.input.walk()
    }
    fn get_trace(&self) -> &FileTrace {
        &self.trace.inner
    }

    fn produce_item<P: Printer>(
        &self,
        path: &Path,
        processor: &P::Processor,
    ) -> Result<Vec<P::Processed>> {
        let Some(lang) = Lang::from_path(path) else {
            return Ok(vec![]);
        };
        self.trace.print_file(path, lang.clone())?;
        let matcher = self.arg.build_pattern(lang.clone())?;
        // match sub region
        let sub_langs = lang.injectable_sg_langs().into_iter().flatten();
        let sub_matchers = sub_langs
            .filter_map(|l| {
                let maybe_pattern = self.arg.build_pattern(l.clone());
                maybe_pattern.ok().map(|pattern| (l, pattern))
            })
            .collect::<Vec<_>>();

        let items = filter_file_pattern(path, lang, Some(&matcher), &sub_matchers)?;
        let mut ret = Vec::with_capacity(items.len());

        for unit in items {
            let Some(processed) = match_one_file(processor, &unit)? else {
                continue;
            };
            ret.push(processed);
        }
        Ok(ret)
    }
}

struct RunWithSpecificLang {
    arg: RunArg,
    pattern: Pattern,
    stats: RunTrace,
}

impl RunWithSpecificLang {
    fn new(arg: RunArg, stats: RunTrace) -> Result<Self> {
        let lang = arg
            .lang
            .clone()
            .ok_or(anyhow::anyhow!(EC::LanguageNotSpecified))?;
        let pattern_ret = arg.build_pattern(lang.clone());
        arg.debug_pattern_if_needed(&pattern_ret, lang.clone());
        Ok(Self {
            arg,
            pattern: pattern_ret?,
            stats,
        })
    }
}

impl Worker for RunWithSpecificLang {
    fn consume_items<P: Printer>(
        &self,
        items: Items<P::Processed>,
        mut printer: P,
    ) -> Result<ExitCode> {
        printer.before_print()?;
        let mut has_matches = false;
        for item in items {
            printer.process(item)?;
            has_matches = true;
        }
        printer.after_print()?;
        self.stats.print()?;
        if !has_matches && self.pattern.has_error() {
            Err(anyhow::anyhow!(EC::PatternHasError))
        } else {
            Ok(ExitCode::from(if has_matches { 0 } else { 1 }))
        }
    }
}

impl PathWorker for RunWithSpecificLang {
    fn build_walk(&self) -> Result<WalkParallel> {
        let lang = self.arg.lang.clone().expect("must present");
        self.arg.input.walk_lang(lang)
    }
    fn get_trace(&self) -> &FileTrace {
        &self.stats.inner
    }
    fn produce_item<P: Printer>(
        &self,
        path: &Path,
        processor: &P::Processor,
    ) -> Result<Vec<P::Processed>> {
        let arg = &self.arg;
        let pattern = &self.pattern;
        let lang = arg.lang.clone().expect("must present");
        let Some(path_lang) = Lang::from_path(path) else {
            return Ok(vec![]);
        };
        self.stats.print_file(path, path_lang.clone())?;
        let (root_matcher, sub_matchers) = if path_lang == lang {
            (Some(pattern), vec![])
        } else {
            (None, vec![(lang, pattern.clone())])
        };
        let filtered = filter_file_pattern(path, path_lang, root_matcher, &sub_matchers)?;
        let mut ret = Vec::with_capacity(filtered.len());
        for unit in filtered {
            let Some(processed) = match_one_file(processor, &unit)? else {
                continue;
            };
            ret.push(processed);
        }
        Ok(ret)
    }
}

impl StdInWorker for RunWithSpecificLang {
    fn parse_stdin<P: Printer>(
        &self,
        src: String,
        processor: &P::Processor,
    ) -> Result<Vec<P::Processed>> {
        let lang = self.arg.lang.clone().expect("must present");
        let grep = lang.ast_grep(src);
        let root = grep.root();
        let mut matches = root.find_all(&self.pattern).peekable();
        if matches.peek().is_none() {
            return Ok(vec![]);
        }
        let path = Path::new("STDIN");
        let processed = processor.print_matches(matches.collect(), path)?;
        Ok(vec![processed])
    }
}

fn match_one_file<T, P: PrintProcessor<T>>(
    processor: &P,
    match_unit: &MatchUnit<impl Matcher>,
) -> Result<Option<T>> {
    let MatchUnit {
        path,
        grep,
        matcher,
    } = match_unit;

    let root = grep.root();
    let mut matches = root.find_all(matcher).peekable();
    if matches.peek().is_none() {
        return Ok(None);
    }
    let ret = processor.print_matches(matches.collect(), path)?;
    Ok(Some(ret))
}
