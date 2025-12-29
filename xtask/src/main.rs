use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("build-grammars") => build_grammars(&args[2..]),
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
    eprintln!("  build-grammars [--out <dir>]  Compile tree-sitter grammars to shared libraries");
    eprintln!("  help                          Show this message");
}

fn build_grammars(args: &[String]) {
    let out_dir = parse_out_dir(args);
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
        "Found {} grammars, compiling to {}",
        grammars.len(),
        out_dir.display()
    );

    let mut success = 0;
    let mut failed = 0;

    for (lang, crate_dir) in &grammars {
        match compile_grammar(lang, crate_dir, &out_dir) {
            Ok(size) => {
                println!("  {lang}: {}", human_size(size));
                success += 1;
            }
            Err(e) => {
                eprintln!("  {lang}: FAILED - {e}");
                failed += 1;
            }
        }
    }

    println!("\nCompiled {success} grammars ({failed} failed)");
}

fn parse_out_dir(args: &[String]) -> PathBuf {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--out" && i + 1 < args.len() {
            return PathBuf::from(&args[i + 1]);
        }
        i += 1;
    }
    PathBuf::from("target/grammars")
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

    let lib_ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };

    let out_file = out_dir.join(format!("{lang}.{lib_ext}"));

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

    // Copy highlight queries if available
    let highlights_scm = crate_dir.join("queries/highlights.scm");
    if highlights_scm.exists() {
        let dest = out_dir.join(format!("{lang}.highlights.scm"));
        let _ = fs::copy(&highlights_scm, &dest);
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
