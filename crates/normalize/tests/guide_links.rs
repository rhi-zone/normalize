//! Guide-link regression test.
//!
//! Parses every `normalize <...>` example line in each guide topic and resolves
//! the subcommand path against the live clap Command tree (built programmatically
//! via `NormalizeService::cli_command()` — no external binary is spawned).
//!
//! If a guide references a command that has been moved or renamed, this test
//! catches the stale reference and names the offending guide topic + line.
//!
//! # Resolution algorithm
//!
//! Tokens are extracted greedily from each `normalize …` line: all leading
//! lowercase-alphanumeric-or-hyphen words (stopping at flags `--`/`-`, paths
//! containing `/`/`.`, positional arguments starting with uppercase, quoted
//! strings, etc.).
//!
//! Each extracted token list is walked against the clap Command tree:
//! - Token matches a subcommand → descend and continue.
//! - Token does NOT match any subcommand on a LEAF command (no subcommands) →
//!   OK — the remaining tokens are positional arguments.
//! - Token does NOT match any subcommand on a NON-LEAF command → FAIL — the
//!   guide references a subcommand that doesn't exist.

use normalize::service::NormalizeService;
use normalize::service::guide::GuideService;
use server_less::CliSubcommand;

/// Returns true if `token` looks like a subcommand name: all lowercase ASCII
/// letters, digits, or hyphens, starting with a lowercase letter.
///
/// Stops extraction at flags (`-`), paths (`.`, `/`), quoted strings (`"`/`'`),
/// angle-bracket placeholders (`<`), uppercase words (positional args by
/// convention in guide text), and shell metacharacters.
fn looks_like_subcommand(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && token.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// Extract the leading subcommand-candidate tokens from a `normalize …` line.
///
/// Returns `None` if the line (after trimming) doesn't start with `normalize `.
/// Strips inline `# comments` before tokenising.
fn extract_guide_ref(line: &str) -> Option<Vec<String>> {
    let rest = line.trim().strip_prefix("normalize ")?;
    // Strip inline comment: ` # …` suffix.
    let rest = rest.split(" #").next().unwrap_or(rest).trim_end();
    let tokens: Vec<String> = rest
        .split_whitespace()
        .take_while(|t| looks_like_subcommand(t))
        .map(String::from)
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

/// Walk `tokens` greedily against the clap Command tree rooted at `cmd`.
///
/// Returns `Ok(())` if the path is resolvable:
/// - All tokens resolved as subcommands, OR
/// - Resolution stopped at a LEAF command (no sub-subcommands); remaining
///   tokens are treated as positional arguments.
///
/// Returns `Err(msg)` if a token fails to match on a NON-LEAF command — that
/// indicates the guide references a subcommand that no longer exists.
fn resolve_path(cmd: &server_less::clap::Command, tokens: &[String]) -> Result<(), String> {
    match tokens.split_first() {
        None => Ok(()),
        Some((head, tail)) => {
            if let Some(sub) = cmd.find_subcommand(head.as_str()) {
                // Token matched — descend into the subcommand.
                resolve_path(sub, tail)
            } else {
                let has_subcommands = cmd.get_subcommands().next().is_some();
                if has_subcommands {
                    // Non-leaf command that doesn't recognise this token →
                    // the guide references a missing or renamed subcommand.
                    Err(format!(
                        "'{}' is not a subcommand of '{}'",
                        head,
                        cmd.get_name()
                    ))
                } else {
                    // Leaf command — all remaining tokens are positional args.
                    Ok(())
                }
            }
        }
    }
}

/// All guide topics: (human-readable name, guide content).
fn all_guides() -> Vec<(&'static str, String)> {
    let g = GuideService;
    vec![
        ("rules", g.rules().unwrap().content),
        ("explore", g.explore().unwrap().content),
        ("setup", g.setup().unwrap().content),
        ("analyze", g.analyze().unwrap().content),
        ("tree-sitter", g.tree_sitter().unwrap().content),
    ]
}

/// Every `normalize <…>` example in every guide must resolve against the live
/// clap Command tree.  Fails with a list of stale references if any don't.
#[test]
fn guide_commands_are_valid() {
    let root_cmd = <NormalizeService as CliSubcommand>::cli_command();
    let mut failures: Vec<String> = Vec::new();

    for (topic, content) in all_guides() {
        for line in content.lines() {
            if let Some(tokens) = extract_guide_ref(line)
                && let Err(e) = resolve_path(&root_cmd, &tokens)
            {
                failures.push(format!(
                    "guide/{}: `normalize {}` — {}",
                    topic,
                    tokens.join(" "),
                    e,
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Stale normalize command references in guide bodies:\n{}",
        failures.join("\n")
    );
}

/// Unit-level proof that the resolution logic catches bogus references.
///
/// This is the red half of the red-green verification: it asserts that a
/// made-up first-level subcommand (`frobnicate`) is detected as stale, and
/// that a known-good command (`view`) passes.  Run `cargo test guide` to
/// confirm both behaviours without modifying any guide text.
#[test]
fn detect_stale_guide_reference() {
    let root_cmd = <NormalizeService as CliSubcommand>::cli_command();

    // Bogus top-level subcommand: must be an error.
    let bad = vec!["frobnicate".to_string()];
    assert!(
        resolve_path(&root_cmd, &bad).is_err(),
        "`normalize frobnicate` should have been detected as stale"
    );

    // Known-good top-level subcommand: must pass.
    let good = vec!["view".to_string()];
    assert!(
        resolve_path(&root_cmd, &good).is_ok(),
        "`normalize view` should resolve fine"
    );

    // Known-good two-level path: must pass.
    let good2 = vec!["analyze".to_string(), "security".to_string()];
    assert!(
        resolve_path(&root_cmd, &good2).is_ok(),
        "`normalize analyze security` should resolve fine"
    );

    // Bogus second-level subcommand on a non-leaf parent: must be an error.
    let bad2 = vec!["analyze".to_string(), "frobnicate".to_string()];
    assert!(
        resolve_path(&root_cmd, &bad2).is_err(),
        "`normalize analyze frobnicate` should have been detected as stale"
    );

    // Valid command followed by a positional arg (leaf command): must pass.
    let leaf_with_arg = vec![
        "graph".to_string(),
        "dependents".to_string(),
        "path".to_string(),
    ];
    assert!(
        resolve_path(&root_cmd, &leaf_with_arg).is_ok(),
        "`normalize graph dependents path` — `path` is a positional arg on a leaf command, must pass"
    );
}
