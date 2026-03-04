#![allow(warnings, clippy::all, unexpected_cfgs)]
use crate::ast_grep::lang::Lang;
use ast_grep_config::{RuleConfig, Severity};
use ast_grep_core::Doc;
use ast_grep_core::{Node as SgNode, meta_var::MetaVariable, tree_sitter::StrDoc};

type Node<'t, L> = SgNode<'t, StrDoc<L>>;

use std::collections::HashMap;

use super::{Diff, NodeMatch, PrintProcessor, Printer};
use anyhow::Result;
use clap::ValueEnum;
use codespan_reporting::files::SimpleFile;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;
use std::io::{Stdout, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Zero-based character position in a file.
struct Position {
    /// Zero-based line number
    line: usize,
    /// Zero-based character column in a line
    column: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Range {
    /// inclusive start, exclusive end
    byte_offset: std::ops::Range<usize>,
    start: Position,
    end: Position,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchNode<'t> {
    text: Cow<'t, str>,
    range: Range,
}
/// a sub field of leading and trailing text count around match.
/// plugin authors can use it to split `lines` into leading, matching and trailing
/// See ast-grep/ast-grep#1381
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharCount {
    leading: usize,
    trailing: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchJSON<'t, 'b> {
    text: Cow<'t, str>,
    range: Range,
    file: Cow<'b, str>,
    lines: String,
    char_count: CharCount,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement_offsets: Option<std::ops::Range<usize>>,
    language: Lang,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta_variables: Option<MetaVariables<'t>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleMatchJSON<'t, 'b> {
    #[serde(flatten)]
    matched: MatchJSON<'t, 'b>,
    rule_id: &'b str,
    severity: Severity,
    note: Option<String>,
    message: String,
}

impl<'t, 'b> RuleMatchJSON<'t, 'b> {
    fn new(nm: NodeMatch<'t>, path: &'b str, rule: &'b RuleConfig<Lang>) -> Self {
        let message = rule.get_message(&nm);
        let matched = MatchJSON::new(nm, path, (0, 0));
        Self {
            matched,
            rule_id: &rule.id,
            severity: rule.severity.clone(),
            note: rule.note.clone(),
            message,
        }
    }
    fn diff(diff: Diff<'t>, path: &'b str, rule: &'b RuleConfig<Lang>) -> Self {
        let nm = &diff.node_match;
        let message = rule.get_message(nm);
        let matched = MatchJSON::diff(diff, path, (0, 0));
        Self {
            matched,
            rule_id: &rule.id,
            severity: rule.severity.clone(),
            note: rule.note.clone(),
            message,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaVariables<'t> {
    single: HashMap<String, MatchNode<'t>>,
    multi: HashMap<String, Vec<MatchNode<'t>>>,
    transformed: HashMap<String, String>,
}
fn from_env<'t>(nm: &NodeMatch<'t>) -> Option<MetaVariables<'t>> {
    let env = nm.get_env();
    let mut vars = env.get_matched_variables().peekable();
    vars.peek()?;
    let mut single = HashMap::new();
    let mut multi = HashMap::new();
    let mut transformed = HashMap::new();
    for var in vars {
        use MetaVariable as MV;
        match var {
            MV::Capture(n, _) => {
                if let Some(node) = env.get_match(&n) {
                    single.insert(
                        n,
                        MatchNode {
                            text: node.text(),
                            range: get_range(node),
                        },
                    );
                } else if let Some(bytes) = env.get_transformed(&n) {
                    transformed.insert(n, String::from_utf8_lossy(bytes).into_owned());
                }
            }
            MV::MultiCapture(n) => {
                let nodes = env.get_multiple_matches(&n);
                multi.insert(
                    n,
                    nodes
                        .into_iter()
                        .map(|node| MatchNode {
                            text: node.text(),
                            range: get_range(&node),
                        })
                        .collect(),
                );
            }
            _ => continue,
        }
    }
    Some(MetaVariables {
        single,
        multi,
        transformed,
    })
}

fn get_range(n: &Node<'_, Lang>) -> Range {
    let start_pos = n.start_pos();
    let end_pos = n.end_pos();
    Range {
        byte_offset: n.range(),
        start: Position {
            line: start_pos.line(),
            column: start_pos.column(n),
        },
        end: Position {
            line: end_pos.line(),
            column: end_pos.column(n),
        },
    }
}

impl<'t, 'b> MatchJSON<'t, 'b> {
    fn new(nm: NodeMatch<'t>, path: &'b str, context: (u16, u16)) -> Self {
        let display = nm.display_context(context.0 as usize, context.1 as usize);
        let lines = format!("{}{}{}", display.leading, display.matched, display.trailing);
        MatchJSON {
            file: Cow::Borrowed(path),
            text: nm.text(),
            lines,
            char_count: CharCount {
                leading: display.leading.chars().count(),
                trailing: display.trailing.chars().count(),
            },
            language: nm.lang().clone(),
            replacement: None,
            replacement_offsets: None,
            range: get_range(&nm),
            meta_variables: from_env(&nm),
        }
    }

    fn diff(diff: Diff<'t>, path: &'b str, context: (u16, u16)) -> Self {
        let mut ret = Self::new(diff.node_match, path, context);
        ret.replacement = Some(diff.replacement);
        ret.replacement_offsets = Some(diff.range);
        ret
    }
}

/// Controls how to print and format JSON object in output.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum JsonStyle {
    /// Prints the matches as a pretty-printed JSON array, with indentation and line breaks.
    /// This is useful for human readability, but not for parsing by other programs.
    /// This is the default value for the `--json` option.
    Pretty,
    /// Prints each match as a separate JSON object, followed by a newline character.
    /// This is useful for streaming the output to other programs that can read one object per line.
    Stream,
    /// Prints the matches as a single-line JSON array, without any whitespace.
    /// This is useful for saving space and minimizing the output size.
    Compact,
}

pub struct JSONPrinter<W: Write> {
    output: W,
    style: JsonStyle,
    context: (u16, u16),
    // indicate if any matches happened
    matched: bool,
}
impl JSONPrinter<Stdout> {
    pub fn stdout(style: JsonStyle) -> Self {
        Self::new(std::io::stdout(), style)
    }
}

impl<W: Write> JSONPrinter<W> {
    pub fn new(output: W, style: JsonStyle) -> Self {
        // no match happened yet
        Self {
            style,
            output,
            context: (0, 0),
            matched: false,
        }
    }

    pub fn context(mut self, context: (u16, u16)) -> Self {
        self.context = context;
        self
    }
}

impl<W: Write> Printer for JSONPrinter<W> {
    type Processed = Buffer;
    type Processor = JSONProcessor;

    fn get_processor(&self) -> JSONProcessor {
        JSONProcessor {
            style: self.style,
            context: self.context,
        }
    }
    fn process(&mut self, processed: Buffer) -> Result<()> {
        if processed.is_empty() {
            return Ok(());
        }
        let output = &mut self.output;
        let matched = self.matched;
        self.matched = true;
        // print separator if there was a match before
        if matched {
            let separator = match self.style {
                JsonStyle::Pretty => ",\n",
                JsonStyle::Stream => "",
                JsonStyle::Compact => ",",
            };
            write!(output, "{separator}")?;
        } else if self.style == JsonStyle::Pretty {
            // print newline for the first match in pretty style
            writeln!(output)?;
        }
        output.write_all(&processed)?;
        Ok(())
    }

    fn before_print(&mut self) -> Result<()> {
        if self.style == JsonStyle::Stream {
            return Ok(());
        }
        write!(self.output, "[")?;
        Ok(())
    }

    fn after_print(&mut self) -> Result<()> {
        if self.style == JsonStyle::Stream {
            return Ok(());
        }
        let output = &mut self.output;
        if self.matched && self.style == JsonStyle::Pretty {
            writeln!(output)?;
        }
        writeln!(output, "]")?;
        Ok(())
    }
}

pub struct JSONProcessor {
    style: JsonStyle,
    context: (u16, u16),
}

impl JSONProcessor {
    fn print_docs<S: Serialize>(&self, mut docs: impl Iterator<Item = S>) -> Result<Buffer> {
        let mut ret = Vec::new();
        let Some(doc) = docs.next() else {
            return Ok(ret);
        };
        let output = &mut ret;
        match self.style {
            JsonStyle::Pretty => {
                serde_json::to_writer_pretty(&mut *output, &doc)?;
                for doc in docs {
                    writeln!(&mut *output, ",")?;
                    serde_json::to_writer_pretty(&mut *output, &doc)?;
                }
            }
            JsonStyle::Stream => {
                // Stream mode requires a newline after each object
                for doc in std::iter::once(doc).chain(docs) {
                    serde_json::to_writer(&mut *output, &doc)?;
                    writeln!(&mut *output)?;
                }
            }
            JsonStyle::Compact => {
                serde_json::to_writer(&mut *output, &doc)?;
                for doc in docs {
                    write!(output, ",")?;
                    serde_json::to_writer(&mut *output, &doc)?;
                }
            }
        }
        Ok(ret)
    }
}

type Buffer = Vec<u8>;

impl PrintProcessor<Buffer> for JSONProcessor {
    fn print_rule(
        &self,
        matches: Vec<NodeMatch>,
        file: SimpleFile<Cow<str>, &str>,
        rule: &RuleConfig<Lang>,
    ) -> Result<Buffer> {
        let path = file.name();
        let jsons = matches
            .into_iter()
            .map(|nm| RuleMatchJSON::new(nm, path, rule));
        self.print_docs(jsons)
    }

    fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Buffer> {
        let path = path.to_string_lossy();
        let context = self.context;
        let jsons = matches
            .into_iter()
            .map(|nm| MatchJSON::new(nm, &path, context));
        self.print_docs(jsons)
    }

    fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Buffer> {
        let path = path.to_string_lossy();
        let context = self.context;
        let jsons = diffs
            .into_iter()
            .map(|diff| MatchJSON::diff(diff, &path, context));
        self.print_docs(jsons)
    }

    fn print_rule_diffs(
        &self,
        diffs: Vec<(Diff, &RuleConfig<Lang>)>,
        path: &Path,
    ) -> Result<Buffer> {
        let path = path.to_string_lossy();
        let jsons = diffs
            .into_iter()
            .map(|(diff, rule)| RuleMatchJSON::diff(diff, &path, rule));
        self.print_docs(jsons)
    }
}
