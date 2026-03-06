# normalize-shadow/src

Source for the shadow git tracking system.

- `lib.rs` — the entire implementation: `Shadow` struct with full history management (init, before/after edit, commit, undo, redo, goto, diff, tree, prune, validate, apply_to_real); `HistoryEntry`, `EditInfo`, `UndoResult`, `ValidationResult`, `ShadowConfig`, `ShadowError`
