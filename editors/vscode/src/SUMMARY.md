# VS Code Extension Source

TypeScript source for the normalize VS Code extension. `extension.ts` is the entry point — it activates the extension, registers commands, and wires together the three subsystems. `runner.ts` handles spawning the normalize binary and parsing its diagnostic JSON output. `diagnostics.ts` translates normalize issues into VS Code `Diagnostic` objects and manages the diagnostic collection. `skeleton.ts` implements the skeleton tree view panel for displaying file structure.
