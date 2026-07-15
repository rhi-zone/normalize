/// Return true if the first-run grammar check should be skipped for this
/// invocation. Skips for `grammars` subcommands (they manage their own
/// install) and for help/version/schema flags.
fn should_skip_grammar_check(argv: &[std::ffi::OsString]) -> bool {
    let sub = argv.get(1).and_then(|s| s.to_str()).unwrap_or("");
    if matches!(sub, "grammars") {
        return true;
    }
    argv.iter().skip(1).filter_map(|s| s.to_str()).any(|s| {
        matches!(
            s,
            "--help" | "-h" | "--version" | "-V" | "--input-schema" | "--output-schema"
        )
    })
}

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
            "--help" | "-h" | "--version" | "-V" | "--input-schema" | "--output-schema"
        )
    })
}

/// Rewrite well-known command aliases to their canonical forms.
///
/// Users from other tools often try `normalize find`, `normalize lint`, etc.
/// This rewrites argv so the expected names work transparently:
/// - `find` → `grep`
/// - `lint` → `rules run`
/// - `check` → `ci`
/// - `index` → `structure rebuild`
/// - `refactor` → `edit`
///
/// Note: `search` is NOT an alias — it is the top-level semantic-search verb
/// (`normalize search <query>`), served by `normalize-semantic`.
fn rewrite_aliases(mut argv: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let subcmd = argv.get(1).and_then(|s| s.to_str()).map(str::to_owned);
    match subcmd.as_deref() {
        // Simple 1:1 aliases — replace argv[1] in place.
        Some("find") => {
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

/// Expand `@`-sigil command aliases in argv.
///
/// If `argv[1]` starts with `@`, look up the alias name in the unified alias
/// system. If it resolves to a command-syntax alias, shell-tokenize its value
/// and splice the result into argv replacing the `@name`, appending any
/// remaining user args. Non-command aliases (glob, path, sql) are left
/// untouched for downstream handling.
///
/// On expansion, logs the alias name and expanded command at debug level.
/// On failure, prints a diagnostic mentioning the alias name and continues
/// with the original argv so the user sees a normal "unknown command" error.
fn expand_command_alias(mut argv: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    let subcmd = match argv.get(1).and_then(|s| s.to_str()) {
        Some(s) if s.starts_with('@') => s,
        _ => return argv,
    };

    let alias_name = &subcmd[1..]; // Strip @
    if alias_name.is_empty() {
        return argv;
    }

    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let alias_config: normalize::filter::AliasConfig =
        normalize_config_paths::load_section_hierarchical(&root, "aliases");

    let cmd_str = match alias_config.get_command(alias_name) {
        Some(cmd) => cmd,
        None => {
            // Check if it exists as a non-command alias
            if alias_config.syntax_of(alias_name).is_some() {
                eprintln!(
                    "error: @{} is not a command alias (syntax: {})",
                    alias_name,
                    alias_config
                        .syntax_of(alias_name)
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                );
            } else {
                eprintln!("error: unknown alias @{}", alias_name);
            }
            return argv;
        }
    };

    let tokens = match shell_words::split(&cmd_str) {
        Ok(tokens) => tokens,
        Err(e) => {
            eprintln!(
                "error: alias @{}: invalid shell syntax in command value: {}",
                alias_name, e
            );
            return argv;
        }
    };

    if tokens.is_empty() {
        eprintln!("error: alias @{}: command value is empty", alias_name);
        return argv;
    }

    // Validate first token is a known subcommand (best-effort)
    let first_token = &tokens[0];
    if !is_known_subcommand(first_token) {
        tracing::warn!(
            "alias @{}: first token '{}' is not a recognized subcommand",
            alias_name,
            first_token
        );
    }

    tracing::debug!("alias @{} expanded to: {}", alias_name, cmd_str);

    // Build new argv: argv[0] + expanded tokens + remaining user args (argv[2..])
    let mut new_argv = vec![argv[0].clone()];
    new_argv.extend(tokens.into_iter().map(std::ffi::OsString::from));
    new_argv.extend(argv.drain(2..));
    new_argv
}

/// Best-effort check if a string is a known top-level normalize subcommand.
fn is_known_subcommand(name: &str) -> bool {
    matches!(
        name,
        "view"
            | "grep"
            | "context"
            | "init"
            | "update"
            | "translate"
            | "daemon"
            | "grammars"
            | "guide"
            | "generate"
            | "structure"
            | "filter"
            | "syntax"
            | "package"
            | "docs"
            | "sessions"
            | "sync"
            | "tools"
            | "edit"
            | "analyze"
            | "overview"
            | "rank"
            | "trend"
            | "budget"
            | "search"
            | "cfg"
            | "kg"
            | "ratchet"
            | "rules"
            | "serve"
            | "similarity"
            | "graph"
            | "history"
            | "ci"
            | "config"
            | "aliases"
    )
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

    // Auto-started daemons run with stdout/stderr connected to /dev/null, so
    // their tracing output would be silently discarded. When this process is a
    // spawned daemon (NORMALIZE_DAEMON_LOG is set), route logs to a file sink so
    // WARN/ERROR — including spin-loop detection — survive. Foreground
    // `daemon run` and all other invocations keep logging to stderr.
    let env_filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };
    #[cfg(unix)]
    let daemon_log = normalize::daemon::open_daemon_log_writer();
    #[cfg(not(unix))]
    let daemon_log: Option<std::fs::File> = None;
    if let Some(log_file) = daemon_log {
        // File sink: keep timestamps + level + target (this is a long-lived
        // process whose log is read after the fact, unlike interactive output).
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(log_file))
            .with_env_filter(env_filter())
            .init();
    } else {
        tracing_subscriber::fmt()
            .without_time()
            .with_target(false)
            .with_level(false)
            .with_env_filter(env_filter())
            .init();
    }

    let mut argv: Vec<std::ffi::OsString> = std::env::args_os().collect();

    // Normalize argv[0] to its file stem so clap usage strings always show
    // "normalize" regardless of the on-disk binary name (e.g. normalize.elf
    // when invoked via the musl-loader wrapper in the release install).
    let stem0: String = argv
        .first()
        .map(|p| {
            std::path::Path::new(p)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("normalize")
                .to_owned()
        })
        .unwrap_or_else(|| "normalize".to_owned());
    if let Some(first) = argv.first_mut() {
        *first = stem0.as_str().into();
    }
    // Only read inside the drop-in-CLI dispatch blocks below; with none of those
    // features enabled (bare `cli`) it is legitimately unused.
    #[cfg_attr(
        not(any(feature = "jq-cli", feature = "rg-cli", feature = "ast-grep-cli")),
        allow(unused_variables)
    )]
    let argv0: &str = &stem0;

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

    // Auto-start daemon in background before running any command (if configured).
    // Skipped for daemon subcommands, serve, and informational flags.
    if !should_skip_daemon_autostart(&argv) {
        let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        normalize::daemon::maybe_start_daemon(&root);
    }

    // First-run grammar check: if grammars have never been installed and
    // we're running non-interactively, auto-install. The check is gated on
    // a stamp file so it runs at most once per user. Skipped for the
    // grammars subcommand itself (it manages its own install) and for
    // informational flags.
    if !should_skip_grammar_check(&argv) {
        let _ = normalize::commands::grammars::ensure_grammars_first_use();
    }

    // Expand @-sigil command aliases before any other dispatch.
    // e.g. `normalize @vocabulary --json` → `normalize structure query "SELECT ..." --json`
    let argv = expand_command_alias(argv);

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
