//! Output helpers for the `architecture` CLI verb.
//!
//! Mirrors the main crate's `crate::output` re-export so the report modules
//! moved out of `normalize/src/commands/analyze/` compile unchanged: they refer
//! to `crate::output::{OutputFormatter, pretty_ranked_table}`.

pub use normalize_output::*;

use normalize_rank::ranked::{RankEntry, format_ranked_table};
use nu_ansi_term::{Color, Style};

/// House-style `format_pretty()` for any [`RankEntry`] table.
///
/// Renders the same table as [`format_ranked_table`] (so column widths match
/// text mode exactly), then bolds the `#` title and colors each data row via
/// `row_color(entry)` when it returns `Some`. Coloring whole rows (rather than
/// cells) keeps the width math correct — ANSI escapes wrap the already-padded
/// line and never enter the width computation. Pass `|_| None` for a plain
/// bold-title-only table.
pub fn pretty_ranked_table<E: RankEntry>(
    title: &str,
    entries: &[E],
    empty_message: Option<&str>,
    row_color: impl Fn(&E) -> Option<Color>,
) -> String {
    let table = format_ranked_table(title, entries, empty_message);
    let lines: Vec<&str> = table.lines().collect();
    // Layout from format_ranked_table: title, blank, [header, separator, rows...]
    // or title, blank, empty_message.
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut data_row_idx = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push(Style::new().bold().paint(*line).to_string());
        } else if i >= 4 && !entries.is_empty() {
            // Data rows start after title, blank, header, separator.
            match entries.get(data_row_idx).and_then(&row_color) {
                Some(color) => out.push(color.paint(*line).to_string()),
                None => out.push((*line).to_string()),
            }
            data_row_idx += 1;
        } else {
            out.push((*line).to_string());
        }
    }
    out.join("\n")
}
