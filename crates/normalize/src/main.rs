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

fn main() {
    reset_sigpipe();

    // Handle --schema for Nursery integration (before clap parsing)
    if handle_schema_flag() {
        return;
    }

    let service = normalize::service::NormalizeService::new();
    match service.cli_run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
