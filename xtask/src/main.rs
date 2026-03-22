use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("build-grammars") => build_grammars(&args[2..]),
        Some("bump-version") => bump_version(&args[2..]),
        Some("help") | None => print_help(),
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    eprintln!("Usage: cargo xtask <command>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  build-grammars [--out <dir>] [--force]");
    eprintln!("      Compile tree-sitter grammars to shared libraries");
    eprintln!("      --out <dir>  Output directory (default: target/grammars)");
    eprintln!("      --force      Recompile even if grammar already exists");
    eprintln!("  bump-version <new-version> [--dry-run]");
    eprintln!("      Update version strings for all normalize-* crates");
    eprintln!("      <new-version>  New semver version (e.g. 0.3.0)");
    eprintln!("      --dry-run      Show what would change without writing");
    eprintln!("  help             Show this message");
}

fn build_grammars(args: &[String]) {
    let args = parse_build_args(args);
    let (out_dir, force) = (args.out_dir, args.force);
    fs::create_dir_all(&out_dir).expect("Failed to create output directory");

    let registry_src = find_cargo_registry_src();
    let grammars = find_arborium_grammars(&registry_src);

    if grammars.is_empty() {
        eprintln!(
            "No arborium grammar crates found. Run 'cargo build' first to download dependencies."
        );
        std::process::exit(1);
    }

    println!(
        "Found {} grammars, output: {}",
        grammars.len(),
        out_dir.display()
    );

    let mut compiled = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut queries_copied = 0;

    for (lang, crate_dir) in &grammars {
        // Copy query files from arborium (highlights.scm, injections.scm, locals.scm)
        queries_copied += copy_query_files(lang, crate_dir, &out_dir);

        // Check if grammar already exists
        let lib_ext = lib_extension();
        let out_file = out_dir.join(format!("{lang}.{lib_ext}"));

        if out_file.exists() && !force {
            skipped += 1;
            continue;
        }

        match compile_grammar(lang, crate_dir, &out_dir) {
            Ok(size) => {
                println!("  {lang}: {}", human_size(size));
                compiled += 1;
            }
            Err(e) => {
                eprintln!("  {lang}: FAILED - {e}");
                failed += 1;
            }
        }
    }

    // Compile local grammars from grammars/ in the workspace root.
    // These take priority over arborium — always compiled (they override installed .so).
    let local_grammars = find_local_grammars();
    if !local_grammars.is_empty() {
        println!(
            "\nCompiling {} local grammars (grammars/):",
            local_grammars.len()
        );
        for (lang, grammar_dir) in &local_grammars {
            match compile_local_grammar(lang, grammar_dir, &out_dir) {
                Ok(size) => {
                    println!("  {lang}: {} (local)", human_size(size));
                    compiled += 1;
                }
                Err(e) => {
                    eprintln!("  {lang}: FAILED - {e}");
                    failed += 1;
                }
            }
        }
    }

    // Copy bundled query files from the workspace queries/ directory.
    // These supplement arborium grammars for languages that don't ship their own.
    // Arborium-provided files take precedence (already written above); bundled files
    // are copied only when the destination doesn't exist yet.
    let workspace_queries = find_workspace_queries_dir();
    if let Some(ref qdir) = workspace_queries {
        queries_copied += copy_bundled_queries(qdir, &out_dir);
    }

    println!("\nCompiled {compiled} grammars, skipped {skipped} (already built), {failed} failed");
    if queries_copied > 0 {
        println!("Copied {queries_copied} query files");
    }
}

struct BuildArgs {
    out_dir: PathBuf,
    force: bool,
}

fn parse_build_args(args: &[String]) -> BuildArgs {
    let mut out_dir = PathBuf::from("target/grammars");
    let mut force = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" if i + 1 < args.len() => {
                out_dir = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--force" => force = true,
            _ => {}
        }
        i += 1;
    }
    BuildArgs { out_dir, force }
}

fn lib_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

/// Copy query files (highlights.scm, injections.scm, locals.scm) if they don't exist.
/// Returns the number of files copied.
fn copy_query_files(lang: &str, crate_dir: &Path, out_dir: &Path) -> usize {
    let mut copied = 0;

    let query_files = [
        ("highlights.scm", format!("{lang}.highlights.scm")),
        ("injections.scm", format!("{lang}.injections.scm")),
        ("locals.scm", format!("{lang}.locals.scm")),
    ];

    for (src_name, dest_name) in &query_files {
        let src = crate_dir.join("queries").join(src_name);
        let dest = out_dir.join(dest_name);

        if src.exists() && !dest.exists() && fs::copy(&src, &dest).is_ok() {
            copied += 1;
        }
    }

    copied
}

/// Find the workspace `queries/` directory (sibling of `xtask/`).
fn find_workspace_queries_dir() -> Option<PathBuf> {
    // CARGO_MANIFEST_DIR is xtask/; workspace root is one level up.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let workspace_root = Path::new(&manifest_dir).parent()?;
    let qdir = workspace_root.join("queries");
    qdir.is_dir().then_some(qdir)
}

/// Copy bundled query files from the workspace `queries/` directory to `out_dir`.
/// Files are named `{lang}.{kind}.scm` (e.g., `rust.locals.scm`).
/// Skips files that already exist in `out_dir` from arborium (same content),
/// but updates them when the workspace version differs (e.g. after edits).
/// Returns the number of files copied or updated.
fn copy_bundled_queries(queries_dir: &Path, out_dir: &Path) -> usize {
    let mut copied = 0;
    let Ok(entries) = fs::read_dir(queries_dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let src = entry.path();
        if src.extension().and_then(|e| e.to_str()) != Some("scm") {
            continue;
        }
        let Some(filename) = src.file_name() else {
            continue;
        };
        let dest = out_dir.join(filename);
        let needs_copy = if dest.exists() {
            // Update if content differs (workspace edit propagates on next build).
            fs::read(&src).ok() != fs::read(&dest).ok()
        } else {
            true
        };
        if needs_copy && fs::copy(&src, &dest).is_ok() {
            copied += 1;
        }
    }
    copied
}

/// Find local grammars in the workspace `grammars/` directory.
/// Each subdirectory named `<lang>` that contains `src/parser.c` is a grammar.
/// Returns `(lang, grammar_dir)` pairs.
fn find_local_grammars() -> Vec<(String, PathBuf)> {
    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let workspace_root = match Path::new(&manifest_dir).parent() {
        Some(p) => p.to_path_buf(),
        None => return Vec::new(),
    };
    let grammars_dir = workspace_root.join("grammars");

    let Ok(entries) = fs::read_dir(&grammars_dir) else {
        return Vec::new();
    };

    let mut grammars = Vec::new();
    for entry in entries.flatten() {
        let grammar_dir = entry.path();
        if !grammar_dir.is_dir() {
            continue;
        }
        let lang = grammar_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        // Local grammar structure: src/parser.c (not grammar/src/parser.c like arborium)
        if grammar_dir.join("src/parser.c").exists() {
            grammars.push((lang, grammar_dir));
        }
    }
    grammars.sort_by(|a, b| a.0.cmp(&b.0));
    grammars
}

/// Compile a local grammar from `grammars/<lang>/`.
/// Local structure differs from arborium: parser at `src/parser.c`, scanner at `src/scanner.c`.
fn compile_local_grammar(lang: &str, grammar_dir: &Path, out_dir: &Path) -> Result<u64, String> {
    let parser_c = grammar_dir.join("src/parser.c");
    let scanner_c = grammar_dir.join("src/scanner.c");
    let out_file = out_dir.join(format!("{lang}.{}", lib_extension()));

    let mut cmd = Command::new("cc");
    cmd.arg("-shared")
        .arg("-fPIC")
        .arg("-O2")
        .arg("-I")
        .arg(grammar_dir.join("src"))
        .arg(&parser_c);

    if scanner_c.exists() {
        cmd.arg(&scanner_c);
    }

    #[cfg(target_os = "linux")]
    cmd.arg("-Wl,--unresolved-symbols=ignore-in-shared-libs");

    #[cfg(target_os = "macos")]
    cmd.arg("-undefined").arg("dynamic_lookup");

    cmd.arg("-o").arg(&out_file);

    let output = cmd.output().map_err(|e| format!("Failed to run cc: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Compilation failed: {stderr}"));
    }

    let size = fs::metadata(&out_file).map(|m| m.len()).unwrap_or(0);
    Ok(size)
}

fn find_cargo_registry_src() -> PathBuf {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .expect("No home directory");
    PathBuf::from(home).join(".cargo/registry/src")
}

fn find_arborium_grammars(registry_src: &Path) -> Vec<(String, PathBuf)> {
    let mut grammars = Vec::new();

    let Ok(entries) = fs::read_dir(registry_src) else {
        return grammars;
    };

    for entry in entries.flatten() {
        let index_dir = entry.path();
        if !index_dir.is_dir() {
            continue;
        }

        let Ok(crates) = fs::read_dir(&index_dir) else {
            continue;
        };

        for crate_entry in crates.flatten() {
            let crate_dir = crate_entry.path();
            let name = crate_dir.file_name().unwrap().to_string_lossy();

            if let Some(lang) = name.strip_prefix("arborium-") {
                // Skip non-language crates
                if matches!(lang.split('-').next(), Some("tree" | "theme" | "highlight")) {
                    continue;
                }

                // Strip version suffix (e.g., "c-sharp-2.4.5" -> "c-sharp")
                // Version is always at the end in format X.Y.Z
                let lang = strip_version_suffix(lang);

                // Check grammar exists
                if crate_dir.join("grammar/src/parser.c").exists() {
                    grammars.push((lang.to_string(), crate_dir));
                }
            }
        }
    }

    // Deduplicate - keep latest version (they're sorted lexically, so highest version wins)
    grammars.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    grammars.dedup_by(|a, b| a.0 == b.0);

    grammars.sort_by(|a, b| a.0.cmp(&b.0));
    grammars
}

fn compile_grammar(lang: &str, crate_dir: &Path, out_dir: &Path) -> Result<u64, String> {
    let parser_c = crate_dir.join("grammar/src/parser.c");
    let scanner_c = crate_dir.join("grammar/scanner.c");

    let out_file = out_dir.join(format!("{lang}.{}", lib_extension()));

    let mut cmd = Command::new("cc");
    cmd.arg("-shared")
        .arg("-fPIC")
        .arg("-O2")
        .arg("-I")
        .arg(crate_dir.join("grammar/src"))
        .arg("-I")
        .arg(crate_dir.join("grammar/include"))
        .arg("-I")
        .arg(crate_dir.join("grammar"))
        .arg("-I")
        .arg(crate_dir.join("grammar/src/tree_sitter"))
        .arg(&parser_c);

    if scanner_c.exists() {
        cmd.arg(&scanner_c);
    }

    // Scanner uses ts_calloc/ts_free - resolved at runtime
    #[cfg(target_os = "linux")]
    cmd.arg("-Wl,--unresolved-symbols=ignore-in-shared-libs");

    #[cfg(target_os = "macos")]
    cmd.arg("-undefined").arg("dynamic_lookup");

    cmd.arg("-o").arg(&out_file);

    let output = cmd.output().map_err(|e| format!("Failed to run cc: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Compilation failed: {stderr}"));
    }

    let size = fs::metadata(&out_file).map(|m| m.len()).unwrap_or(0);
    Ok(size)
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{}K", bytes / 1024)
    } else {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ─── bump-version ────────────────────────────────────────────────────────────

fn bump_version(args: &[String]) {
    // Parse args: first non-flag arg is the new version; --dry-run is a flag.
    let mut new_version: Option<String> = None;
    let mut dry_run = false;

    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            v if !v.starts_with('-') => new_version = Some(v.to_string()),
            other => {
                eprintln!("Unknown flag: {other}");
                std::process::exit(1);
            }
        }
    }

    let new_version = match new_version {
        Some(v) => v,
        None => {
            eprintln!("Usage: cargo xtask bump-version <new-version> [--dry-run]");
            std::process::exit(1);
        }
    };

    if !is_valid_semver(&new_version) {
        eprintln!("Error: '{new_version}' is not a valid semver version (expected X.Y.Z)");
        std::process::exit(1);
    }

    let workspace_root = find_workspace_root();
    let cargo_tomls = collect_cargo_tomls(&workspace_root);

    let mut package_updates = 0usize;
    let mut dep_updates = 0usize;

    for path in &cargo_tomls {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: could not read {}: {e}", path.display());
                continue;
            }
        };

        let result = apply_version_bump(&content, &new_version);
        let (pkg_count, dep_count) = (result.pkg_updates, result.dep_updates);

        if pkg_count == 0 && dep_count == 0 {
            continue;
        }

        let rel = path.strip_prefix(&workspace_root).unwrap_or(path);
        if pkg_count > 0 {
            println!(
                "{}: package version -> {new_version} ({pkg_count} update{})",
                rel.display(),
                if pkg_count == 1 { "" } else { "s" }
            );
        }
        if dep_count > 0 {
            println!(
                "{}: dependency constraints -> {new_version} ({dep_count} update{})",
                rel.display(),
                if dep_count == 1 { "" } else { "s" }
            );
        }

        if !dry_run {
            if let Err(e) = fs::write(path, result.content.as_bytes()) {
                eprintln!("Error writing {}: {e}", path.display());
                std::process::exit(1);
            }
        }

        package_updates += pkg_count;
        dep_updates += dep_count;
    }

    println!();
    if dry_run {
        println!(
            "[dry-run] Would update {package_updates} package version{}, \
             {dep_updates} dependency constraint{}",
            if package_updates == 1 { "" } else { "s" },
            if dep_updates == 1 { "" } else { "s" },
        );
    } else {
        println!(
            "Updated {package_updates} package version{}, \
             {dep_updates} dependency constraint{}",
            if package_updates == 1 { "" } else { "s" },
            if dep_updates == 1 { "" } else { "s" },
        );

        if package_updates + dep_updates > 0 {
            println!("\nRunning cargo generate-lockfile...");
            let status = Command::new("cargo")
                .arg("generate-lockfile")
                .current_dir(&workspace_root)
                .status();
            match status {
                Ok(s) if s.success() => println!("Cargo.lock updated."),
                Ok(s) => {
                    eprintln!("cargo generate-lockfile exited with {s}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to run cargo generate-lockfile: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Validate that a string is a semver version (X.Y.Z with optional pre-release/build).
/// Accepts the common `MAJOR.MINOR.PATCH[-pre][+build]` format without pulling in a dep.
fn is_valid_semver(v: &str) -> bool {
    // Strip optional build metadata (+...)
    let v = v.split('+').next().unwrap_or(v);
    // Strip optional pre-release (-...)
    let core = v.split('-').next().unwrap_or(v);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Find the workspace root by walking up from CARGO_MANIFEST_DIR (which is xtask/).
fn find_workspace_root() -> PathBuf {
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set — run via cargo xtask");
    Path::new(&manifest_dir)
        .parent()
        .expect("xtask has no parent directory")
        .to_path_buf()
}

/// Collect all Cargo.toml files to process:
/// - workspace root Cargo.toml
/// - xtask/Cargo.toml
/// - all Cargo.toml files under crates/ (recursive)
fn collect_cargo_tomls(workspace_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Workspace root
    let root_toml = workspace_root.join("Cargo.toml");
    if root_toml.exists() {
        paths.push(root_toml);
    }

    // xtask
    let xtask_toml = workspace_root.join("xtask/Cargo.toml");
    if xtask_toml.exists() {
        paths.push(xtask_toml);
    }

    // crates/ — recursive
    let crates_dir = workspace_root.join("crates");
    collect_tomls_recursive(&crates_dir, &mut paths);

    paths
}

fn collect_tomls_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_tomls_recursive(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
            out.push(path);
        }
    }
}

/// Result of applying version bumps to a single Cargo.toml.
struct BumpResult {
    content: String,
    pkg_updates: usize,
    dep_updates: usize,
}

/// Apply version bumps to the contents of a Cargo.toml.
///
/// Rules:
/// - In a `[package]` block belonging to a `normalize-*` crate: update `version = "..."`.
/// - In dependency sections (`[dependencies]`, `[dev-dependencies]`, etc.) and
///   `[workspace.package]`: update `version = "..."` on lines that also mention `normalize-`.
fn apply_version_bump(content: &str, new_version: &str) -> BumpResult {
    let mut out = String::with_capacity(content.len());
    let mut pkg_updates = 0usize;
    let mut dep_updates = 0usize;

    // State machine: track current section context.
    #[derive(Clone, Copy, PartialEq)]
    enum Section {
        Other,
        Package,      // [package] — need to see the crate name before updating version
        WorkspacePkg, // [workspace.package] — always update version
        Dependencies, // any *dependencies* section
    }

    let mut section = Section::Other;
    // For [package] blocks: buffer lines until we know the crate name.
    let mut pkg_buffer: Vec<String> = Vec::new();
    // The name found in the current [package] block.
    let mut pkg_name: Option<String> = None;
    // Whether the buffered [package] block has a version line that needs updating.
    let mut pkg_version_line: Option<usize> = None; // index into pkg_buffer

    let flush_package_buffer = |buffer: &mut Vec<String>,
                                name: &Option<String>,
                                version_line_idx: &Option<usize>,
                                new_version: &str,
                                out: &mut String,
                                pkg_updates: &mut usize| {
        let is_normalize = name
            .as_deref()
            .map(|n| n == "normalize" || n.starts_with("normalize-"))
            .unwrap_or(false);
        for (i, line) in buffer.iter().enumerate() {
            if is_normalize && Some(i) == *version_line_idx {
                let updated = replace_version_string(line, new_version);
                if updated != *line {
                    *pkg_updates += 1;
                }
                out.push_str(&updated);
            } else {
                out.push_str(line);
            }
        }
        buffer.clear();
    };

    for raw_line in content.lines() {
        // Preserve original line endings: lines() strips \n, so we re-add it.
        // We'll reassemble with \n and at the end fix up the trailing newline.
        let line = raw_line;

        let trimmed = line.trim();

        // Detect section headers.
        if trimmed.starts_with('[') {
            // Flush any pending [package] buffer before switching sections.
            if section == Section::Package && !pkg_buffer.is_empty() {
                flush_package_buffer(
                    &mut pkg_buffer,
                    &pkg_name,
                    &pkg_version_line,
                    new_version,
                    &mut out,
                    &mut pkg_updates,
                );
                pkg_name = None;
                pkg_version_line = None;
            }

            let header = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
            // Strip inline comments (e.g. `[package] # comment`)
            let header = header.split('#').next().unwrap_or(header).trim();

            section = if header == "package" {
                Section::Package
            } else if header == "workspace.package" {
                Section::WorkspacePkg
            } else if header.ends_with("dependencies")
                || header.contains("dependencies.")
                || header.starts_with("workspace.dependencies")
            {
                Section::Dependencies
            } else {
                Section::Other
            };

            if section == Section::Package {
                pkg_buffer.push(format!("{line}\n"));
                continue;
            }

            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Inside [package]: buffer and look for name/version.
        if section == Section::Package {
            // Check for `name = "..."` to learn the crate name.
            if pkg_name.is_none() {
                if let Some(name) = extract_toml_string_value(line, "name") {
                    pkg_name = Some(name);
                }
            }
            // Check for `version = "..."` (not workspace-inherited).
            if pkg_version_line.is_none() && is_version_field(line) && !line.contains("workspace") {
                pkg_version_line = Some(pkg_buffer.len());
            }
            pkg_buffer.push(format!("{line}\n"));
            continue;
        }

        // [workspace.package]: update `version = "..."` directly.
        if section == Section::WorkspacePkg && is_version_field(line) && !line.contains("workspace")
        {
            let updated = replace_version_string(line, new_version);
            if updated != line {
                pkg_updates += 1;
            }
            out.push_str(&updated);
            out.push('\n');
            continue;
        }

        // Dependency sections: update only normalize-* dep lines.
        if section == Section::Dependencies && line.contains("normalize-") {
            let updated = replace_version_string(line, new_version);
            if updated != line {
                dep_updates += 1;
            }
            out.push_str(&updated);
            out.push('\n');
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    // Flush any remaining [package] buffer.
    if section == Section::Package && !pkg_buffer.is_empty() {
        flush_package_buffer(
            &mut pkg_buffer,
            &pkg_name,
            &pkg_version_line,
            new_version,
            &mut out,
            &mut pkg_updates,
        );
    }

    // Preserve original trailing newline behaviour.
    if !content.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    BumpResult {
        content: out,
        pkg_updates,
        dep_updates,
    }
}

/// Return true if `line` is a TOML `version = "..."` assignment (key may have whitespace).
fn is_version_field(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("version") && trimmed.contains('=')
}

/// Extract the string value of a simple `key = "value"` TOML field.
fn extract_toml_string_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(key)?;
    let rest = rest.trim_start().strip_prefix('=')?;
    let rest = rest.trim();
    if rest.starts_with('"') {
        let inner = rest.trim_start_matches('"');
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

/// Replace the version string in a line like:
///   `version = "0.1.0"`
///   `normalize-foo = { path = "...", version = "0.1.0" }`
/// Only replaces the first `version = "..."` occurrence.
fn replace_version_string(line: &str, new_version: &str) -> String {
    // Find `version = "` then the closing `"`.
    let needle = "version = \"";
    if let Some(start) = line.find(needle) {
        let after_quote = start + needle.len();
        if let Some(end_offset) = line[after_quote..].find('"') {
            let end = after_quote + end_offset;
            let mut result = String::with_capacity(line.len());
            result.push_str(&line[..after_quote]);
            result.push_str(new_version);
            result.push_str(&line[end..]);
            return result;
        }
    }
    line.to_string()
}

/// Strip version suffix from crate name (e.g., "c-sharp-2.4.5" -> "c-sharp").
fn strip_version_suffix(name: &str) -> &str {
    // Match semver pattern at end: -X.Y.Z (with optional pre-release/build metadata)
    // Cargo crate names end with -MAJOR.MINOR.PATCH
    if let Some(idx) = name.rfind('-') {
        let suffix = &name[idx + 1..];
        // Check if suffix looks like semver: starts with digit, contains dot
        if suffix
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && suffix.contains('.')
        {
            return &name[..idx];
        }
    }
    name
}
