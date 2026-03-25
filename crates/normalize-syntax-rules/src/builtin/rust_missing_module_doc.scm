# ---
# id = "rust/missing-module-doc"
# severity = "warning"
# tags = ["documentation", "rust"]
# message = "Module file has no `//!` inner doc comment — add a module-level doc block at the top"
# languages = ["rust"]
# files = ["**/lib.rs", "**/mod.rs"]
# enabled = false
# ---
#
# Rust convention: `lib.rs` and `mod.rs` should begin with a `//!` inner doc
# comment describing what the module provides. This is the idiomatic equivalent
# of a SUMMARY.md for code — it is the first thing a reader (or `cargo doc`)
# sees when they open the module.
#
# ## How to fix
#
# Add one or more `//!` lines at the top of the file, before any `use`
# statements or item definitions:
#
# ```rust
# //! Token management and refresh logic.
# //!
# //! This module wraps the OAuth token store and handles automatic
# //! refresh before expiry.
# ```
#
# ## When to disable
#
# This rule is disabled by default. Enable it selectively when you want to
# enforce documentation coverage on module entry points:
#
# ```toml
# # .normalize/config.toml
# ["rust/missing-module-doc"]
# enabled = true
# ```
#
# Files that are genuinely internal implementation details (e.g. re-export
# shims, generated code) can be opted out individually using an inline allow
# comment or by adding them to the rule's allow list.

; Detects: source_file with no //! inner doc comment at the top
((source_file) @match
 (#not-match? @match "^//!"))
