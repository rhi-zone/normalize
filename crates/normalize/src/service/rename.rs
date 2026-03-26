//! Top-level rename service for `normalize rename`.
//!
//! Cross-file symbol rename: resolves the target via the facts index, finds all call
//! sites and import statements, checks for name conflicts, then applies (or previews
//! with `--dry-run`) the batch edit across every affected file.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::config::NormalizeConfig;
use crate::output::OutputFormatter;
use crate::shadow::{EditInfo, Shadow};
use crate::{edit, path_resolve};

// ── Report types ─────────────────────────────────────────────────────────────

/// A single rename site: one location where the identifier was (or would be) changed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RenameSite {
    /// Relative file path.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// Role of this site: `"definition"`, `"call"`, or `"import"`.
    pub kind: String,
}

/// A name conflict that would be introduced by the rename.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ConflictInfo {
    /// Relative file path where the conflict was detected.
    pub file: String,
    /// Human-readable description of the conflict.
    pub description: String,
}

/// Report returned by `normalize rename`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RenameReport {
    /// The fully-qualified target that was renamed (e.g. `src/lib.rs/old_fn`).
    pub symbol: String,
    /// Name before the rename.
    pub old_name: String,
    /// Name after the rename.
    pub new_name: String,
    /// All sites that were (or would be) updated.
    pub sites: Vec<RenameSite>,
    /// Conflicts detected before applying (populated when `--force` was NOT supplied
    /// and conflicts exist; causes the rename to abort with `Err`).
    pub conflicts: Vec<ConflictInfo>,
    /// Whether this was a dry run (no files written).
    pub dry_run: bool,
    /// Whether the rename was applied (false on dry-run or when conflicts aborted it).
    pub applied: bool,
}

impl OutputFormatter for RenameReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();

        if !self.conflicts.is_empty() {
            let _ = writeln!(
                out,
                "Rename '{}' → '{}' would cause conflicts:",
                self.old_name, self.new_name
            );
            for c in &self.conflicts {
                let _ = writeln!(out, "  {}: {}", c.file, c.description);
            }
            let _ = writeln!(out, "Use --force to override.");
            return out;
        }

        if self.sites.is_empty() {
            let _ = writeln!(
                out,
                "No rename sites found for '{}' in {}",
                self.old_name, self.symbol
            );
            return out;
        }

        let verb = if self.dry_run {
            "Would rename"
        } else {
            "Renamed"
        };
        let _ = writeln!(
            out,
            "{} '{}' → '{}' ({} site{}):",
            verb,
            self.old_name,
            self.new_name,
            self.sites.len(),
            if self.sites.len() == 1 { "" } else { "s" }
        );
        for site in &self.sites {
            let _ = writeln!(out, "  {}:{}: {}", site.file, site.line, site.kind);
        }
        out
    }

    fn format_pretty(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();

        if !self.conflicts.is_empty() {
            let _ = writeln!(
                out,
                "\x1b[1;31mConflicts\x1b[0m renaming '{}' → '{}':",
                self.old_name, self.new_name
            );
            for c in &self.conflicts {
                let _ = writeln!(out, "  \x1b[2m{}\x1b[0m {}", c.file, c.description);
            }
            let _ = writeln!(out, "Use \x1b[1m--force\x1b[0m to override.");
            return out;
        }

        if self.sites.is_empty() {
            let _ = writeln!(
                out,
                "No rename sites found for '\x1b[1m{}\x1b[0m' in {}",
                self.old_name, self.symbol
            );
            return out;
        }

        let verb = if self.dry_run {
            "\x1b[1;33mWould rename\x1b[0m"
        } else {
            "\x1b[1;32mRenamed\x1b[0m"
        };
        let _ = writeln!(
            out,
            "{} '{}' → '\x1b[1m{}\x1b[0m' ({} site{}):",
            verb,
            self.old_name,
            self.new_name,
            self.sites.len(),
            if self.sites.len() == 1 { "" } else { "s" }
        );
        for site in &self.sites {
            let kind_color = match site.kind.as_str() {
                "definition" => "\x1b[1;32m",
                "call" => "\x1b[0;36m",
                _ => "\x1b[0;33m", // import
            };
            let _ = writeln!(
                out,
                "  \x1b[2m{}:{}\x1b[0m  {}[{}]\x1b[0m",
                site.file, site.line, kind_color, site.kind
            );
        }
        out
    }
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Rename a symbol across its definition, all call sites, and all import statements.
///
/// Returns a `RenameReport` with every site touched. On conflict (and `!force`), returns
/// an `Err` describing the conflicts; the `RenameReport` is embedded in the error message.
///
/// Gracefully degrades when the facts index is unavailable: renames the definition only.
#[allow(dead_code)]
pub(crate) async fn do_rename_report(
    target: &str,
    new_name: &str,
    root: Option<&Path>,
    dry_run: bool,
    force: bool,
    message: Option<&str>,
) -> Result<RenameReport, String> {
    use std::collections::HashSet;

    let root: PathBuf = root
        .map(|p| p.to_path_buf())
        // normalize-syntax-allow: rust/unwrap-in-impl - current_dir() only fails if cwd deleted
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let config = NormalizeConfig::load(&root);
    let shadow_enabled = config.shadow.enabled();

    // Resolve the target path/symbol
    let unified = path_resolve::resolve_unified(target, &root)
        .ok_or_else(|| format!("No matches for: {}", target))?;

    if unified.symbol_path.is_empty() {
        return Err(format!(
            "Target must include a symbol name (e.g. path/SymbolName), got: {}",
            target
        ));
    }

    // normalize-syntax-allow: rust/unwrap-in-impl - symbol_path non-empty (checked above)
    let old_name = unified.symbol_path.last().unwrap().as_str();
    let def_rel_path = unified.file_path.clone();
    let def_abs_path = root.join(&def_rel_path);

    let def_content = std::fs::read_to_string(&def_abs_path)
        .map_err(|e| format!("Error reading {}: {}", def_rel_path, e))?;
    let editor = edit::Editor::new();

    // Find definition location
    let loc = editor
        .find_symbol(&def_abs_path, &def_content, old_name, false)
        .ok_or_else(|| format!("Symbol '{}' not found in {}", old_name, def_rel_path))?;

    // Try to open index for cross-file awareness (graceful degradation)
    let (callers, importers) = async {
        match crate::index::ensure_ready(&root).await {
            Ok(idx) => {
                let callers = idx
                    .find_callers(old_name, &def_rel_path)
                    .await
                    .unwrap_or_default();
                let importers = idx
                    .find_symbol_importers(old_name)
                    .await
                    .unwrap_or_default();
                (callers, importers)
            }
            Err(e) => {
                eprintln!(
                    "warning: index not available ({}); renaming definition only",
                    e
                );
                (vec![], vec![])
            }
        }
    }
    .await;

    // ── Conflict detection ──────────────────────────────────────────────────
    let mut conflict_list: Vec<ConflictInfo> = vec![];

    if !force {
        // 1. Does new_name already exist as a symbol in the definition file?
        if editor
            .find_symbol(&def_abs_path, &def_content, new_name, false)
            .is_some()
        {
            conflict_list.push(ConflictInfo {
                file: def_rel_path.clone(),
                description: format!("symbol '{}' already exists", new_name),
            });
        }

        // 2. Does any importer file already import something named new_name?
        #[allow(clippy::collapsible_if)]
        if !importers.is_empty() {
            if let Some(idx) = crate::index::open_if_enabled(&root).await {
                for (file, _, _, _) in &importers {
                    if idx.has_import_named(file, new_name).await.unwrap_or(false) {
                        conflict_list.push(ConflictInfo {
                            file: file.clone(),
                            description: format!("already imports '{}'", new_name),
                        });
                    }
                }
            }
        }

        if !conflict_list.is_empty() {
            // Return a report with conflicts so --json consumers get structured data,
            // but also return Err so the CLI exits non-zero.
            let report = RenameReport {
                symbol: target.to_string(),
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
                sites: vec![],
                conflicts: conflict_list,
                dry_run,
                applied: false,
            };
            return Err(report.format_text());
        }
    }

    // ── Collect all sites ─────────────────────────────────────────────────────
    let mut sites: Vec<RenameSite> = vec![];

    // Definition site
    sites.push(RenameSite {
        file: def_rel_path.clone(),
        line: loc.start_line,
        kind: "definition".to_string(),
    });

    // Call sites
    for (file, _, line, _) in &callers {
        sites.push(RenameSite {
            file: file.clone(),
            line: *line,
            kind: "call".to_string(),
        });
    }

    // Import sites
    for (file, _, _, line) in &importers {
        sites.push(RenameSite {
            file: file.clone(),
            line: *line,
            kind: "import".to_string(),
        });
    }

    // ── Shadow: snapshot before writes ───────────────────────────────────────
    let all_files: HashSet<String> = sites.iter().map(|s| s.file.clone()).collect();

    if !dry_run && shadow_enabled {
        let abs_paths: Vec<_> = all_files.iter().map(|f| root.join(f)).collect();
        let shadow = Shadow::new(&root);
        if let Err(e) =
            shadow.before_edit(&abs_paths.iter().map(|p| p.as_path()).collect::<Vec<_>>())
        {
            eprintln!("warning: shadow git: {}", e);
        }
    }

    // ── Apply edits ───────────────────────────────────────────────────────────
    let mut modified: Vec<String> = vec![];

    // 1. Definition file
    if let Some(new_content) =
        editor.rename_identifier_in_line(&def_content, loc.start_line, old_name, new_name)
    {
        if dry_run {
            modified.push(def_rel_path.clone());
        } else {
            match std::fs::write(&def_abs_path, &new_content) {
                Ok(_) => modified.push(def_rel_path.clone()),
                Err(e) => eprintln!("error writing {}: {}", def_rel_path, e),
            }
        }
    }

    // 2. Call sites — group by file to read each file once
    let mut callers_by_file: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (file, _, line, _) in &callers {
        callers_by_file.entry(file.clone()).or_default().push(*line);
    }

    for (rel_path, lines) in &callers_by_file {
        let abs_path = root.join(rel_path);
        let mut content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut changed = false;
        for &line_no in lines {
            if let Some(new_content) =
                editor.rename_identifier_in_line(&content, line_no, old_name, new_name)
            {
                content = new_content;
                changed = true;
            }
        }
        if changed {
            if dry_run {
                if !modified.contains(rel_path) {
                    modified.push(rel_path.clone());
                }
            } else {
                match std::fs::write(&abs_path, &content) {
                    Ok(_) => {
                        if !modified.contains(rel_path) {
                            modified.push(rel_path.clone());
                        }
                    }
                    Err(e) => eprintln!("error writing {}: {}", rel_path, e),
                }
            }
        }
    }

    // 3. Import sites — group by file to read each file once
    let mut importers_by_file: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (file, _, _, line) in &importers {
        importers_by_file
            .entry(file.clone())
            .or_default()
            .push(*line);
    }

    for (rel_path, lines) in &importers_by_file {
        let abs_path = root.join(rel_path);
        let mut content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut changed = false;
        for &line_no in lines {
            if let Some(new_content) =
                editor.rename_identifier_in_line(&content, line_no, old_name, new_name)
            {
                content = new_content;
                changed = true;
            }
        }
        if changed {
            if dry_run {
                if !modified.contains(rel_path) {
                    modified.push(rel_path.clone());
                }
            } else {
                match std::fs::write(&abs_path, &content) {
                    Ok(_) => {
                        if !modified.contains(rel_path) {
                            modified.push(rel_path.clone());
                        }
                    }
                    Err(e) => eprintln!("error writing {}: {}", rel_path, e),
                }
            }
        }
    }

    // ── Shadow: commit after writes ───────────────────────────────────────────
    if !dry_run && shadow_enabled && !modified.is_empty() {
        let abs_paths: Vec<_> = modified.iter().map(|f| root.join(f)).collect();
        let shadow = Shadow::new(&root);
        let info = EditInfo {
            operation: "rename".to_string(),
            target: format!("{} -> {}", old_name, new_name),
            files: abs_paths,
            message: message.map(String::from),
            workflow: None,
        };
        if let Err(e) = shadow.after_edit(&info) {
            eprintln!("warning: shadow git: {}", e);
        }
    }

    let applied = !dry_run && !modified.is_empty();
    Ok(RenameReport {
        symbol: target.to_string(),
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        sites,
        conflicts: vec![],
        dry_run,
        applied,
    })
}
