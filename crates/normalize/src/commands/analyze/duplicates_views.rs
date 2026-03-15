//! Unified duplicates command — groups duplicate-functions, duplicate-blocks,
//! similar-functions, similar-blocks, and clusters behind `--scope`, `--similar`,
//! and `--cluster` flags.

use normalize_analyze::ranked::RankStats;
use normalize_output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Scope of duplicate detection: function-level or block-level.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateScope {
    #[default]
    Functions,
    Blocks,
}

impl FromStr for DuplicateScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "functions" | "function" => Ok(DuplicateScope::Functions),
            "blocks" | "block" => Ok(DuplicateScope::Blocks),
            _ => Err(format!(
                "invalid scope '{}': expected 'functions' or 'blocks'",
                s
            )),
        }
    }
}

impl fmt::Display for DuplicateScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DuplicateScope::Functions => write!(f, "functions"),
            DuplicateScope::Blocks => write!(f, "blocks"),
        }
    }
}

/// Detection mode.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateMode {
    #[default]
    Exact,
    Similar,
    Clusters,
}

impl FromStr for DuplicateMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "exact" => Ok(DuplicateMode::Exact),
            "similar" | "fuzzy" => Ok(DuplicateMode::Similar),
            "clusters" | "cluster" => Ok(DuplicateMode::Clusters),
            _ => Err(format!(
                "invalid mode '{}': expected 'exact', 'similar', or 'clusters'",
                s
            )),
        }
    }
}

impl fmt::Display for DuplicateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DuplicateMode::Exact => write!(f, "exact"),
            DuplicateMode::Similar => write!(f, "similar"),
            DuplicateMode::Clusters => write!(f, "clusters"),
        }
    }
}

/// A code location within a duplicate group.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CodeLocation {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
}

impl CodeLocation {
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// A directory pair that had many duplicate/similar pairs suppressed.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SuppressedDirectoryPair {
    /// First directory (or the sole directory if both files share it).
    pub dir_a: String,
    /// Second directory (empty if same as dir_a).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_b: Option<String>,
    /// Number of pairs/groups suppressed.
    pub pair_count: usize,
}

/// A cluster of pairs sharing the same body pattern across many files (different method names).
/// Suppressed because the pattern is too widespread to be actionable duplication.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SuppressedBodyPatternGroup {
    /// Number of similar-function pairs suppressed in this cluster.
    pub pair_count: usize,
    /// Number of distinct files whose functions participate in this cluster.
    pub file_count: usize,
    /// Number of distinct method names in this cluster (typically >1 for the cross-name case).
    pub name_count: usize,
    /// A representative method name from this cluster (most common, or first alphabetically).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative_name: Option<String>,
}

/// A group of duplicate/similar code locations.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicateGroup {
    pub locations: Vec<CodeLocation>,
    pub line_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_count: Option<usize>,
}

/// Unified duplicates report covering all detection modes.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DuplicatesReport {
    pub mode: DuplicateMode,
    pub scope: DuplicateScope,
    pub files_scanned: usize,
    pub items_analyzed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairs_analyzed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elide_identifiers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elide_literals: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicated_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed_same_name: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<RankStats>,
    pub groups: Vec<DuplicateGroup>,
    /// Directory pairs suppressed because they had too many parallel-impl pairs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed_directory_pairs: Vec<SuppressedDirectoryPair>,
    /// Body-pattern clusters suppressed because the same body appears across too many files.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed_body_pattern_groups: Vec<SuppressedBodyPatternGroup>,
    /// For show_source rendering (not serialized).
    #[serde(skip)]
    pub show_source: bool,
    #[serde(skip)]
    pub roots: Vec<PathBuf>,
}

impl DuplicatesReport {
    /// Number of duplicate groups found.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total lines of code involved in duplicates (exact mode).
    pub fn duplicated_line_count(&self) -> usize {
        self.duplicated_lines.unwrap_or(0)
    }
}

/// Resolve a (potentially repo-prefixed) relative path to an absolute path.
fn resolve_file_path(roots: &[PathBuf], rel: &str) -> PathBuf {
    if roots.len() == 1 {
        return roots[0].join(rel);
    }
    let path = std::path::Path::new(rel);
    let mut components = path.components();
    if let Some(first) = components.next() {
        let repo_name = first.as_os_str();
        if let Some(root) = roots.iter().find(|r| r.file_name() == Some(repo_name)) {
            return root.join(components.as_path());
        }
    }
    roots[0].join(rel)
}

fn render_source(roots: &[PathBuf], file: &str, start_line: usize, end_line: usize) -> Vec<String> {
    let full_path = resolve_file_path(roots, file);
    let mut lines = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&full_path) {
        let src_lines: Vec<&str> = content.lines().collect();
        let start = start_line.saturating_sub(1);
        let end = end_line.min(src_lines.len());
        lines.push(String::new());
        lines.push(format!("   --- {} ---", file));
        for (j, src_line) in src_lines[start..end].iter().enumerate() {
            lines.push(format!("        {:4} | {}", start + j + 1, src_line));
        }
    }
    lines
}

impl OutputFormatter for DuplicatesReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        // Header
        let title = match (self.mode, self.scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => "Duplicate Function Detection",
            (DuplicateMode::Exact, DuplicateScope::Blocks) => "Duplicate Block Detection",
            (DuplicateMode::Similar, DuplicateScope::Functions) => {
                "Similar Function Detection (fuzzy)"
            }
            (DuplicateMode::Similar, DuplicateScope::Blocks) => "Similar Block Detection (fuzzy)",
            (DuplicateMode::Clusters, _) => "Structural Clusters",
        };
        out.push(title.to_string());
        out.push(String::new());

        // Stats
        out.push(format!("Files scanned:      {}", self.files_scanned));
        let items_label = match (self.mode, self.scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => "Functions hashed",
            (DuplicateMode::Exact, DuplicateScope::Blocks) => "Blocks hashed",
            (DuplicateMode::Similar, DuplicateScope::Functions) => "Functions analyzed",
            (DuplicateMode::Similar, DuplicateScope::Blocks) => "Blocks analyzed",
            (DuplicateMode::Clusters, _) => "Functions analyzed",
        };
        out.push(format!(
            "{:<20}{}",
            items_label.to_string() + ":",
            self.items_analyzed
        ));

        if let Some(pairs) = self.pairs_analyzed {
            out.push(format!("Pairs analyzed:     {}", pairs));
        }

        if let Some(threshold) = self.threshold {
            out.push(format!("Threshold:          {:.0}%", threshold * 100.0));
        }

        match self.mode {
            DuplicateMode::Exact => {
                out.push(format!("Duplicate groups:   {}", self.groups.len()));
                if let Some(dl) = self.duplicated_lines {
                    out.push(format!("Duplicated lines:   ~{}", dl));
                }
                if let Some(suppressed) = self.suppressed_same_name
                    && suppressed > 0
                {
                    out.push(format!(
                        "Suppressed: {} same-name groups (likely trait impls; use --include-trait-impls to show)",
                        suppressed
                    ));
                }
            }
            DuplicateMode::Similar => {
                out.push(format!("Similar pairs:      {}", self.groups.len()));
            }
            DuplicateMode::Clusters => {
                let total_fns: usize = self.groups.iter().map(|g| g.locations.len()).sum();
                out.push(format!(
                    "Clusters found:     {}  ({} functions)",
                    self.groups.len(),
                    total_fns
                ));
            }
        }

        if !self.suppressed_directory_pairs.is_empty() {
            let total: usize = self
                .suppressed_directory_pairs
                .iter()
                .map(|s| s.pair_count)
                .sum();
            out.push(format!(
                "Suppressed: {} pairs across {} directory groups (likely parallel implementations; use --include-trait-impls to show)",
                total,
                self.suppressed_directory_pairs.len()
            ));
            for s in &self.suppressed_directory_pairs {
                if let Some(dir_b) = &s.dir_b {
                    out.push(format!(
                        "   {} <-> {}  ({} pairs)",
                        s.dir_a, dir_b, s.pair_count
                    ));
                } else {
                    out.push(format!("   within {}  ({} pairs)", s.dir_a, s.pair_count));
                }
            }
        }

        if !self.suppressed_body_pattern_groups.is_empty() {
            let total_pairs: usize = self
                .suppressed_body_pattern_groups
                .iter()
                .map(|g| g.pair_count)
                .sum();
            out.push(format!(
                "Suppressed: {} pairs across {} body-pattern clusters (same body across many files, different method names; use --include-trait-impls to show)",
                total_pairs,
                self.suppressed_body_pattern_groups.len()
            ));
            for g in &self.suppressed_body_pattern_groups {
                let name_part = if let Some(name) = &g.representative_name {
                    format!(" (e.g. `{}`)", name)
                } else {
                    String::new()
                };
                out.push(format!(
                    "   {} pairs suppressed: same body pattern across {} files ({} method names){}",
                    g.pair_count, g.file_count, g.name_count, name_part
                ));
            }
        }

        if self.groups.is_empty() {
            out.push(String::new());
            let empty_msg = match (self.mode, self.scope) {
                (DuplicateMode::Exact, DuplicateScope::Functions) => {
                    "No duplicate functions detected."
                }
                (DuplicateMode::Exact, DuplicateScope::Blocks) => "No duplicate blocks detected.",
                (DuplicateMode::Similar, DuplicateScope::Functions) => {
                    "No similar functions detected."
                }
                (DuplicateMode::Similar, DuplicateScope::Blocks) => "No similar blocks detected.",
                (DuplicateMode::Clusters, _) => "No function clusters detected.",
            };
            out.push(empty_msg.to_string());
            return out.join("\n");
        }

        out.push(String::new());

        let max_groups = match self.mode {
            DuplicateMode::Exact if matches!(self.scope, DuplicateScope::Functions) => 20,
            _ => 30,
        };

        for (i, group) in self.groups.iter().take(max_groups).enumerate() {
            match self.mode {
                DuplicateMode::Exact => {
                    if i == 0 {
                        let heading = match self.scope {
                            DuplicateScope::Functions => "Duplicate Groups (sorted by size):",
                            DuplicateScope::Blocks => "",
                        };
                        if !heading.is_empty() {
                            out.push(heading.to_string());
                            out.push(String::new());
                        }
                    }
                    match self.scope {
                        DuplicateScope::Functions => {
                            out.push(format!(
                                "{}. {} lines, {} instances:",
                                i + 1,
                                group.line_count,
                                group.locations.len()
                            ));
                        }
                        DuplicateScope::Blocks => {
                            out.push(format!(
                                "{}. {} lines \u{00d7} {} locations",
                                i + 1,
                                group.line_count,
                                group.locations.len()
                            ));
                        }
                    }
                }
                DuplicateMode::Similar => {
                    let sim = group.similarity.unwrap_or(0.0);
                    out.push(format!(
                        "{}. {:.0}% similar  ({} lines)",
                        i + 1,
                        sim * 100.0,
                        group.line_count,
                    ));
                }
                DuplicateMode::Clusters => {
                    let sim = group.similarity.unwrap_or(0.0);
                    let pc = group.pair_count.unwrap_or(0);
                    out.push(format!(
                        "{}. {} functions  {} lines  avg {:.0}% similar  ({} pairs)",
                        i + 1,
                        group.locations.len(),
                        group.line_count,
                        sim * 100.0,
                        pc,
                    ));
                }
            }

            for loc in &group.locations {
                match (self.mode, self.scope) {
                    (DuplicateMode::Exact, DuplicateScope::Functions) => {
                        out.push(format!(
                            "   {}:{}-{} ({})",
                            loc.file,
                            loc.start_line,
                            loc.end_line,
                            loc.symbol.as_deref().unwrap_or("?")
                        ));
                        if self.show_source {
                            out.extend(render_source(
                                &self.roots,
                                &loc.file,
                                loc.start_line,
                                loc.end_line,
                            ));
                        }
                    }
                    (DuplicateMode::Exact, DuplicateScope::Blocks) => {
                        out.push(format!(
                            "   {}:{}-{}",
                            loc.file, loc.start_line, loc.end_line
                        ));
                    }
                    (DuplicateMode::Similar, DuplicateScope::Functions) => {
                        out.push(format!(
                            "   {}:{}  ({}:{}-{})",
                            loc.file,
                            loc.symbol.as_deref().unwrap_or("?"),
                            loc.file,
                            loc.start_line,
                            loc.end_line
                        ));
                    }
                    (DuplicateMode::Similar, DuplicateScope::Blocks) => {
                        out.push(format!(
                            "   {}:{}-{}",
                            loc.file, loc.start_line, loc.end_line
                        ));
                    }
                    (DuplicateMode::Clusters, _) => {
                        out.push(format!(
                            "   {}:{}  (lines {}-{})",
                            loc.file,
                            loc.symbol.as_deref().unwrap_or("?"),
                            loc.start_line,
                            loc.end_line,
                        ));
                    }
                }
            }

            // Show source for blocks in exact/similar modes (first location only for exact blocks)
            if self.show_source
                && matches!(self.mode, DuplicateMode::Exact)
                && matches!(self.scope, DuplicateScope::Blocks)
                && let Some(first) = group.locations.first()
            {
                out.extend(render_source(
                    &self.roots,
                    &first.file,
                    first.start_line,
                    first.end_line,
                ));
            }
            if self.show_source && matches!(self.mode, DuplicateMode::Similar) {
                for loc in &group.locations {
                    out.extend(render_source(
                        &self.roots,
                        &loc.file,
                        loc.start_line,
                        loc.end_line,
                    ));
                }
            }

            out.push(String::new());
        }

        if self.groups.len() > max_groups {
            let label = match self.mode {
                DuplicateMode::Exact => "groups",
                DuplicateMode::Similar => "pairs",
                DuplicateMode::Clusters => "clusters",
            };
            out.push(format!(
                "... and {} more {}",
                self.groups.len() - max_groups,
                label
            ));
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;

        let mut out = Vec::new();

        let title = match (self.mode, self.scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => "# Duplicate Function Detection",
            (DuplicateMode::Exact, DuplicateScope::Blocks) => "# Duplicate Block Detection",
            (DuplicateMode::Similar, DuplicateScope::Functions) => {
                "# Similar Function Detection (fuzzy)"
            }
            (DuplicateMode::Similar, DuplicateScope::Blocks) => "# Similar Block Detection (fuzzy)",
            (DuplicateMode::Clusters, _) => "# Structural Clusters",
        };
        out.push(Color::Cyan.bold().paint(title).to_string());
        out.push(String::new());

        out.push(format!("Files scanned:      {}", self.files_scanned));
        let items_label = match (self.mode, self.scope) {
            (DuplicateMode::Exact, DuplicateScope::Functions) => "Functions hashed",
            (DuplicateMode::Exact, DuplicateScope::Blocks) => "Blocks hashed",
            (DuplicateMode::Similar, DuplicateScope::Functions) => "Functions analyzed",
            (DuplicateMode::Similar, DuplicateScope::Blocks) => "Blocks analyzed",
            (DuplicateMode::Clusters, _) => "Functions analyzed",
        };
        out.push(format!(
            "{:<20}{}",
            items_label.to_string() + ":",
            self.items_analyzed
        ));

        if let Some(pairs) = self.pairs_analyzed {
            out.push(format!("Pairs analyzed:     {}", pairs));
        }
        if let Some(threshold) = self.threshold {
            out.push(format!("Threshold:          {:.0}%", threshold * 100.0));
        }

        match self.mode {
            DuplicateMode::Exact => {
                out.push(format!("Duplicate groups:   {}", self.groups.len()));
                if let Some(dl) = self.duplicated_lines {
                    out.push(format!("Duplicated lines:   ~{}", dl));
                }
                if let Some(suppressed) = self.suppressed_same_name
                    && suppressed > 0
                {
                    out.push(format!("Suppressed: {} same-name groups", suppressed));
                }
            }
            DuplicateMode::Similar => {
                out.push(format!("Similar pairs:      {}", self.groups.len()));
            }
            DuplicateMode::Clusters => {
                let total_fns: usize = self.groups.iter().map(|g| g.locations.len()).sum();
                out.push(format!(
                    "Clusters found:     {}  ({} functions)",
                    self.groups.len(),
                    total_fns
                ));
            }
        }

        if !self.suppressed_directory_pairs.is_empty() {
            let total: usize = self
                .suppressed_directory_pairs
                .iter()
                .map(|s| s.pair_count)
                .sum();
            out.push(
                Color::Fixed(245)
                    .paint(format!(
                        "Suppressed: {} pairs across {} directory groups (parallel implementations)",
                        total,
                        self.suppressed_directory_pairs.len()
                    ))
                    .to_string(),
            );
            for s in &self.suppressed_directory_pairs {
                let line = if let Some(dir_b) = &s.dir_b {
                    format!("   {} <-> {}  ({} pairs)", s.dir_a, dir_b, s.pair_count)
                } else {
                    format!("   within {}  ({} pairs)", s.dir_a, s.pair_count)
                };
                out.push(Color::Fixed(245).paint(line).to_string());
            }
        }

        if !self.suppressed_body_pattern_groups.is_empty() {
            let total_pairs: usize = self
                .suppressed_body_pattern_groups
                .iter()
                .map(|g| g.pair_count)
                .sum();
            out.push(
                Color::Fixed(245)
                    .paint(format!(
                        "Suppressed: {} pairs across {} body-pattern clusters (same body, different method names)",
                        total_pairs,
                        self.suppressed_body_pattern_groups.len()
                    ))
                    .to_string(),
            );
            for g in &self.suppressed_body_pattern_groups {
                let name_part = if let Some(name) = &g.representative_name {
                    format!(" (e.g. `{}`)", name)
                } else {
                    String::new()
                };
                let line = format!(
                    "   {} pairs: body pattern across {} files ({} method names){}",
                    g.pair_count, g.file_count, g.name_count, name_part
                );
                out.push(Color::Fixed(245).paint(line).to_string());
            }
        }

        if self.groups.is_empty() {
            out.push(String::new());
            out.push(Color::Green.paint("No duplicates detected.").to_string());
            return out.join("\n");
        }

        out.push(String::new());

        let max_groups = 30;
        for (i, group) in self.groups.iter().take(max_groups).enumerate() {
            let header = match self.mode {
                DuplicateMode::Exact => {
                    format!(
                        "{}. {} lines, {} instances",
                        i + 1,
                        group.line_count,
                        group.locations.len()
                    )
                }
                DuplicateMode::Similar => {
                    format!(
                        "{}. {:.0}% similar  ({} lines)",
                        i + 1,
                        group.similarity.unwrap_or(0.0) * 100.0,
                        group.line_count
                    )
                }
                DuplicateMode::Clusters => {
                    format!(
                        "{}. {} functions  {} lines  avg {:.0}% similar  ({} pairs)",
                        i + 1,
                        group.locations.len(),
                        group.line_count,
                        group.similarity.unwrap_or(0.0) * 100.0,
                        group.pair_count.unwrap_or(0),
                    )
                }
            };
            out.push(Color::Yellow.bold().paint(header).to_string());

            for loc in &group.locations {
                let loc_str = if let Some(sym) = &loc.symbol {
                    format!(
                        "   {}:{}  (lines {}-{})",
                        loc.file, sym, loc.start_line, loc.end_line
                    )
                } else {
                    format!("   {}:{}-{}", loc.file, loc.start_line, loc.end_line)
                };
                out.push(loc_str);
            }
            out.push(String::new());
        }

        if self.groups.len() > max_groups {
            out.push(format!("... and {} more", self.groups.len() - max_groups));
        }

        out.join("\n")
    }
}
