# normalize-module-resolve

Module resolution infrastructure for normalize cross-file analysis (Phase 0).

Re-exports the `ModuleResolver` trait and supporting types (`ImportSpec`, `ModuleId`,
`Resolution`, `ResolverConfig`) defined in `normalize-languages::traits`. Per-language
resolver implementations live in `normalize-languages/src/<lang>.rs`.
