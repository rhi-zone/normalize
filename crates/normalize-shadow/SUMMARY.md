# normalize-shadow

Shadow git history tracking for `normalize edit` operations — maintains a hidden git repository (`.normalize/shadow/`) that automatically commits a snapshot before and after each edit, providing undo/redo/goto/history for all code modifications made by normalize.

Key types: `Shadow` (manages the shadow repo: `before_edit`, `after_edit`, `history`, `undo`, `redo`, `goto`, `diff`, `tree`, `prune`, `validate`, `apply_to_real`), `HistoryEntry`, `EditInfo`, `UndoResult`, `ValidationResult`, `ShadowConfig`. Supports checkpoint boundaries (refuses to undo past a git commit unless `--cross-checkpoint`), conflict detection (external modifications), and dry-run mode. Git errors in `undo`, `redo`, and `goto` shadow commits now propagate as `ShadowError::Commit` rather than being silently discarded.
