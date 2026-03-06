# VS Code Extension

A VS Code extension that surfaces normalize diagnostics and skeleton views inside the editor. Implemented in TypeScript, it registers commands (lint, view skeleton) and integrates with VS Code's diagnostic collection API to display normalize rule violations as editor annotations. The extension shells out to the normalize binary via `NormalizeRunner` and parses its JSON output. Build output goes to `out/`; source lives in `src/`.
