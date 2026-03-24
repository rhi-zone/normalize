//! Built-in guides — workflow-oriented help that teaches by example.
//!
//! Each method is a guide topic. `normalize guide` lists all topics via `--help`.

use crate::output::OutputFormatter;
use server_less::cli;

/// A guide page returned by the guide service.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct GuideReport {
    pub topic: String,
    pub content: String,
}

impl OutputFormatter for GuideReport {
    fn format_text(&self) -> String {
        self.content.clone()
    }
}

/// Built-in guides for normalize workflows.
pub struct GuideService;

impl GuideService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(name = "guide", description = "Workflow guides with examples")]
impl GuideService {
    /// Writing and testing syntax rules
    #[cli(display_with = "display_output")]
    pub fn rules(&self) -> Result<GuideReport, String> {
        Ok(GuideReport {
            topic: "rules".into(),
            content: GUIDE_RULES.into(),
        })
    }

    /// Exploring a codebase
    #[cli(display_with = "display_output")]
    pub fn explore(&self) -> Result<GuideReport, String> {
        Ok(GuideReport {
            topic: "explore".into(),
            content: GUIDE_EXPLORE.into(),
        })
    }

    /// Setting up normalize in a project
    #[cli(display_with = "display_output")]
    pub fn setup(&self) -> Result<GuideReport, String> {
        Ok(GuideReport {
            topic: "setup".into(),
            content: GUIDE_SETUP.into(),
        })
    }

    /// Running analysis on a codebase
    #[cli(display_with = "display_output")]
    pub fn analyze(&self) -> Result<GuideReport, String> {
        Ok(GuideReport {
            topic: "analyze".into(),
            content: GUIDE_ANALYZE.into(),
        })
    }

    /// Using tree-sitter introspection commands
    #[cli(display_with = "display_output")]
    pub fn tree_sitter(&self) -> Result<GuideReport, String> {
        Ok(GuideReport {
            topic: "tree-sitter".into(),
            content: GUIDE_TREE_SITTER.into(),
        })
    }
}

const GUIDE_RULES: &str = r#"# Writing Syntax Rules

Syntax rules are tree-sitter queries (.scm) with TOML frontmatter.
The workflow: inspect the grammar → draft a query → test it → ship it.

## 1. Find the right node types

  normalize analyze node-types python --search raise

Shows all named types, anonymous types, and field names matching "raise".

## 2. See how target code parses

  normalize syntax ast buggy_code.py
  normalize syntax ast buggy_code.py --at 5:4      # subtree at line 5, col 4
  normalize syntax ast buggy_code.py --depth 3     # limit depth

## 3. Draft and test a query interactively

  normalize syntax query code.py '(raise_statement !cause) @match'
  normalize syntax query code.py path/to/draft.scm

Each match shows captures with node kind, position, and text.

## 4. Create the rule file

Rules live in .normalize/rules/ (project) or ~/.config/normalize/rules/ (global).

  # ---
  # id = "python/raise-without-from"
  # severity = "warning"
  # tags = ["correctness"]
  # message = "Raise inside except should chain: raise X from e"
  # languages = ["python"]
  # enabled = false
  # ---
  (except_clause
    (block
      (raise_statement (_) !cause) @match))

## 5. Test against your codebase

  normalize rules run --rule python/raise-without-from src/
  normalize rules run --rule python/raise-without-from --pretty

## 6. Create fixture tests

In crates/normalize-syntax-rules/tests/fixtures/<lang>/<rule-name>/:
  match.<ext>      — code that SHOULD trigger the rule
  no_match.<ext>   — code that should NOT trigger it

Run: cargo test -p normalize-syntax-rules

## Key patterns

  Field negation:    (node !field_name) @match
  Text match:        (#match? @name "^pattern$")
  Equality:          (#eq? @capture "literal")
  Anchors:           (list . (only_child) .)   — exactly one child
  Named children:    (_)                       — any named node
"#;

const GUIDE_EXPLORE: &str = r#"# Exploring a Codebase

## Directory overview

  normalize view .                        # top-level structure
  normalize view src/                     # expand a directory
  normalize view src/ --depth 2           # deeper expansion

## Find a symbol

  normalize view ClassName                # search by name
  normalize view src/file.rs/function     # path + symbol
  normalize view file.rs:42              # jump to line

## Search for patterns

  normalize grep "pattern"                # ripgrep-powered search
  normalize grep "TODO|FIXME" --only "*.rs"

## Read code with context

  normalize view src/file.rs --full        # full source
  normalize view src/file.rs --context     # skeleton + imports
  normalize view src/file.rs --deps        # show dependencies

## Structural overview

  normalize analyze summary               # auto-generated codebase overview
  normalize analyze size                   # hierarchical LOC (ncdu-style)
  normalize analyze architecture           # coupling, cycles, hubs

## Import/dependency graph

  normalize analyze imports                # modules ranked by import fan-in
  normalize analyze graph --on modules     # dependency graph properties
  normalize analyze dependents --file path # who depends on this?
"#;

const GUIDE_SETUP: &str = r#"# Setting Up Normalize

## Quick start

  normalize init                          # create .normalize/ config
  normalize init --setup                  # interactive rule wizard

The wizard runs all rules, groups violations by count, and walks you through
enabling or disabling each rule interactively.

## Load grammars (for tree-sitter analysis)

  normalize grammars install              # download grammars for detected languages

Grammars are stored in ~/.config/normalize/grammars/.

## Build the facts index (for import/call analysis)

  normalize structure rebuild             # index symbols, imports, calls
  normalize structure stats               # check index status

The index enables: import analysis, call graphs, dead code detection,
dependents tracking, and fact-based rules.

## Configure rules

  normalize rules list                    # see all available rules
  normalize rules list --pretty           # with descriptions
  normalize rules show python/bare-except # full docs for a rule
  normalize rules enable python/bare-except
  normalize rules disable no-todo-comment

Rule config lives in .normalize/config.toml under [analyze.rules].

## Pre-commit hook

Add to .pre-commit-config.yaml or your hook runner:

  normalize rules run                     # exits 1 on error-severity violations

## Editor integration

  normalize serve --lsp                   # LSP server (diagnostics, completion)
  normalize serve --mcp                   # MCP server (for AI assistants)
"#;

const GUIDE_ANALYZE: &str = r#"# Running Analysis

## Start broad, then drill down

  normalize analyze all                   # everything at once
  normalize analyze health                # quick health check
  normalize analyze summary               # generated overview

## Code quality

  normalize analyze complexity             # cyclomatic complexity ranking
  normalize analyze length                 # long functions
  normalize analyze duplicates             # copy-paste detection
  normalize analyze duplicates --scope blocks  # block-level duplicates
  normalize analyze ceremony              # boilerplate ratio

## Module structure

  normalize analyze size                   # LOC breakdown
  normalize analyze density                # information density per module
  normalize analyze module-health          # composite score (tests + uniqueness + density)
  normalize analyze surface                # public API surface per module
  normalize analyze layering               # are imports flowing downward?

## Dependencies and graphs

  normalize analyze architecture           # coupling + cycles + hubs
  normalize analyze graph --on modules     # graph-theoretic properties
  normalize analyze dependents --file path # reverse dependency closure
  normalize analyze call-graph symbol      # callers and callees
  normalize analyze depth-map              # dependency depth + ripple risk

## Git history

  normalize analyze hotspots               # churn x complexity
  normalize analyze coupling               # files that change together
  normalize analyze ownership              # per-file ownership concentration
  normalize analyze complexity-trend       # complexity over git history

## Security

  normalize analyze security               # security scan
  normalize rules run --engine syntax      # includes hardcoded-secret rule

## Output formats

Every command supports:
  --json      machine-readable JSON
  --jsonl     one JSON object per line
  --jq '.field'  filter with jq expressions
  --pretty    colored terminal output
  --schema    print JSON Schema of the output
"#;

const GUIDE_TREE_SITTER: &str = r#"# Tree-Sitter Introspection

Three commands for working with tree-sitter grammars and queries.

## List node types for a grammar

  normalize analyze node-types rust
  normalize analyze node-types python --search "raise"
  normalize analyze node-types go --search "block"

Shows named types, anonymous types (operators, punctuation), and field names.

## Parse a file into its CST

  normalize syntax ast src/main.rs
  normalize syntax ast src/main.rs --depth 3
  normalize syntax ast src/main.rs --at 10:4

Output shows every node with its kind, field name, position, and leaf text:

  source_file [1:0-99:0]
    function_item [1:0-16:1]
      name: identifier "main" [1:3-1:7]
      parameters: parameters "()" [1:7-1:9]
      body: block [1:10-16:1]

## Run a query against a file

  normalize syntax query file.py '(function_definition name: (identifier) @fn)'
  normalize syntax query file.go path/to/query.scm

Shows each match with captures:

  Match 1 (1 capture):
    @fn: identifier "main" [3:5-3:9]

## Common query patterns

  (node_type) @match                       # match any node of this type
  (parent child: (child_type) @cap)        # match with field name
  (node field: _ @cap)                     # match any child in field
  (#eq? @cap "text")                       # text equality
  (#match? @cap "^regex$")                 # regex match
  (node !field_name)                       # field must NOT exist
  (list . (item) .)                        # exactly one child (anchors)
  [(type_a) (type_b)] @match               # match either type

## Workflow: unknown grammar

  normalize analyze node-types <lang>                    # what types exist?
  normalize syntax ast sample.<ext> --depth 2         # how does it parse?
  normalize syntax query sample.<ext> '(type) @m'       # does my query match?
"#;
