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
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
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

fn main() -> std::process::ExitCode {
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

    let service = normalize::service::NormalizeService::new();
    match service.cli_run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            std::process::ExitCode::FAILURE
        }
    }
}
