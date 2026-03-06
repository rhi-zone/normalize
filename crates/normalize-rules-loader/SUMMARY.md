# normalize-rules-loader

Rule pack discovery, loading, and execution for compiled (dylib) rule packs.

Key types: `LoadedRulePack` (wraps `RulePackRef` from the ABI-stable API), `RulePackError` (Load/NotFound/Invalid). Key functions: `load_from_path(path)` (loads a dylib via `RulePackRef::load_from_file`), `discover(root)` (searches `.normalize/rules`, `~/.normalize/rules`, and the XDG data dir for platform-appropriate dylibs), `load_all(root)`, `format_diagnostic`. `LoadedRulePack` exposes `info()`, `run(relations)`, and `run_rule(rule_id, relations)`.
