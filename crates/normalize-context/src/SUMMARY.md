# normalize-context/src

Domain logic for frontmatter-filtered context resolution.

- `lib.rs` — all public types and functions: `CallerContext` (type alias for `HashMap<String, String>`), `ParsedBlock`, `ContextBlock`, `ContextReport`, `ContextListReport`; `parse_blocks` (Markdown + YAML frontmatter parser), `block_matches`, `eval_conditions`, `eval_strategy`, `compare_yaml_to_caller` (matching logic); `collect_new_context_files` (bottom-up `.normalize/{dir}/` walk including `~/.normalize/`), `resolve_context` (orchestration: walk + parse + match), `yaml_to_json` (conversion helper); unit tests for all domain logic
