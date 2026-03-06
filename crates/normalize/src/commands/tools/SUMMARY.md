# src/commands/tools

`normalize tools` subcommand implementations for running external ecosystem tools (linters, formatters, test runners) via the normalize-tools integration layer. Modules: `lint` (run ecosystem linters, collect diagnostics), `test` (run project test suites). These commands act as thin CLI wrappers over `normalize-tools`, surfacing external tool output as normalized `DiagnosticsReport` values.
