/// Return true if daemon auto-start should be skipped for this invocation.
///
/// Skips when:
/// - The command is `daemon` (daemon subcommands manage the daemon themselves)
/// - The command is `serve` (the MCP/HTTP/LSP server is a long-running process)
/// - Any argument is a help/version/schema flag (informational, no side effects wanted)
fn should_skip_daemon_autostart(argv: &[std::ffi::OsString]) -> bool {
    let sub = argv.get(1).and_then(|s| s.to_str()).unwrap_or("");
    if matches!(sub, "daemon" | "serve") {
        return true;
    }
    argv.iter().skip(1).filter_map(|s| s.to_str()).any(|s| {
        matches!(
            s,
            "--help"
                | "-h"
                | "--version"
                | "-V"
                | "--input-schema"
                | "--output-schema"
                | "--schema"
        )
    })
}

/// Rewrite well-known command aliases to their canonical forms.
///
/// Users from other tools often try `normalize search`, `normalize lint`, etc.
/// This rewrites argv so the expected names work transparently:
/// - `search`, `find` → `grep`
/// - `lint` → `rules run`
/// - `check` → `ci`
/// - `index` → `structure rebuild`
/// - `refactor` → `edit`
fn rewrite_aliases(mut argv: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let subcmd = argv.get(1).and_then(|s| s.to_str()).map(str::to_owned);
    match subcmd.as_deref() {
        // Simple 1:1 aliases — replace argv[1] in place.
        Some("search" | "find") => {
            argv[1] = "grep".into();
        }
        Some("check") => {
            argv[1] = "ci".into();
        }
        Some("refactor") => {
            argv[1] = "edit".into();
        }
        // Compound aliases — one alias word expands to two subcommand words.
        Some("lint") => {
            argv.splice(1..2, ["rules".into(), "run".into()]);
        }
        Some("index") => {
            argv.splice(1..2, ["structure".into(), "rebuild".into()]);
        }
        _ => {}
    }
    argv
}

/// Help output styling is now handled by server-less.
/// Schema flag support for Nursery integration.
fn handle_schema_flag() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--schema") {
        let response = serde_json::json!({
            "config_path": ".normalize/config.toml",
            "format": "toml",
            "schema": schemars::schema_for!(normalize::config::NormalizeConfig)
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        );
        true
    } else {
        false
    }
}

/// Reset SIGPIPE to default behavior so piping to `head` etc. doesn't panic.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: libc::signal is a standard POSIX function. We reset SIGPIPE to default
    // behavior (terminate on broken pipe) instead of Rust's default (ignore, causing
    // write errors). This prevents panics when output is piped to commands like `head`.
    // No memory safety concerns - just changes signal disposition.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    reset_sigpipe();

    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();

    let argv0 = argv
        .first()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // argv[0] dispatch: symlink `jq -> normalize` runs jq directly.
    #[cfg(feature = "jq-cli")]
    if argv0 == "jq" {
        return normalize::jq::run_jq(argv[1..].iter().cloned());
    }

    // argv[0] dispatch: symlink `rg -> normalize` runs rg directly.
    #[cfg(feature = "rg-cli")]
    if argv0 == "rg" {
        return normalize::rg::run_rg(argv[1..].iter().cloned());
    }

    // argv[0] dispatch: symlink `ast-grep -> normalize` or `sg -> normalize` runs ast-grep.
    #[cfg(feature = "ast-grep-cli")]
    if argv0 == "ast-grep" || argv0 == "sg" {
        return normalize::ast_grep::run_ast_grep(argv[1..].iter().cloned());
    }

    // Subcommand dispatch: `normalize jq [args...]` bypasses server-less.
    #[cfg(feature = "jq-cli")]
    if argv.get(1).and_then(|s| s.to_str()) == Some("jq") {
        return normalize::jq::run_jq(argv[2..].iter().cloned());
    }

    // Subcommand dispatch: `normalize rg [args...]` bypasses server-less.
    #[cfg(feature = "rg-cli")]
    if argv.get(1).and_then(|s| s.to_str()) == Some("rg") {
        return normalize::rg::run_rg(argv[2..].iter().cloned());
    }

    // Subcommand dispatch: `normalize ast-grep [args...]` or `normalize sg [args...]`
    #[cfg(feature = "ast-grep-cli")]
    if argv
        .get(1)
        .and_then(|s| s.to_str())
        .is_some_and(|sub| sub == "ast-grep" || sub == "sg")
    {
        return normalize::ast_grep::run_ast_grep(argv[2..].iter().cloned());
    }

    // Handle --schema for Nursery integration (before clap parsing)
    if handle_schema_flag() {
        return std::process::ExitCode::SUCCESS;
    }

    // Auto-start daemon in background before running any command (if configured).
    // Skipped for daemon subcommands, serve, and informational flags.
    if !should_skip_daemon_autostart(&argv) {
        let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        normalize::daemon::maybe_start_daemon(&root);
    }

    // Rewrite command aliases so users from other tools find what they expect.
    // Simple aliases map one name to another; compound aliases expand to two subcommands.
    let argv = rewrite_aliases(argv);

    let service = normalize::service::NormalizeService::new();
    match service.cli_run_with_async(argv).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            std::process::ExitCode::FAILURE
        }
    }
}
