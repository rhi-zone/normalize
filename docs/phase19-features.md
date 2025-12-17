# Phase 19: Advanced Features

This document describes the advanced features implemented in Phase 19.

## Overview

Phase 19 focused on developer experience improvements: real-time feedback, IDE integration, refactoring tools, and output customization.

## Features

### 19a: Non-Code Content Plugins
Extends analysis beyond Python to structured content files.

- **Markdown**: Heading structure, link extraction, code block detection
- **JSON/YAML/TOML**: Schema extraction, key paths, validation

### 19b: Embedding-based Search
Semantic code search using embeddings.

- Hybrid routing: TF-IDF for exact matches, embeddings for semantic
- Code indexer with incremental updates
- CLI: `moss search "find authentication logic"`

### 19c: Auto-fix System
Automated code fixes with safety guarantees.

- Safe/unsafe fix classification
- Preview with diff before applying
- Conflict resolution for overlapping fixes
- Shadow Git integration for rollback

### 19e: Visual CFG Output
Control flow graph visualization.

```bash
moss cfg path/to/file.py --mermaid    # Mermaid diagram
moss cfg path/to/file.py --dot        # Graphviz DOT
moss cfg path/to/file.py --html -o graph.html  # Interactive HTML
```

### 19f: LSP Integration
Language Server Protocol support for IDE integration.

```bash
moss lsp  # Start LSP server (stdio)
```

Features:
- Real-time diagnostics
- Hover information (function signatures, docstrings)
- Document symbols (outline view)
- Go-to-definition

### 19g: Live CFG Rendering
Real-time CFG updates as you edit.

```bash
moss cfg path/to/file.py --live --port 8080
```

- HTTP server with auto-refresh
- File watcher integration
- Modern dark-themed UI

### 19h: Progress Indicators
Visual feedback for long-running operations.

```python
from moss.progress import ProgressTracker, MultiStageProgress

# Single operation
async with ProgressTracker(total=100, description="Analyzing") as tracker:
    for i in range(100):
        await tracker.update(1)

# Multi-stage pipeline
stages = [
    ProgressStage("parse", "Parsing files", 50),
    ProgressStage("analyze", "Analyzing", 30),
    ProgressStage("report", "Generating report", 20),
]
async with MultiStageProgress(stages) as progress:
    await progress.start_stage("parse")
    # ... work ...
    await progress.complete_stage("parse")
```

### 19i: Multi-file Refactoring
AST-based refactoring across the codebase.

```python
from moss.refactoring import rename_symbol, move_symbol, extract_function

# Rename across workspace
result = await rename_symbol(
    workspace=Path("."),
    old_name="old_func",
    new_name="new_func",
    dry_run=True,  # Preview changes
)

# Move symbol to different file
result = await move_symbol(
    workspace=Path("."),
    symbol_name="MyClass",
    source_file=Path("old_module.py"),
    target_file=Path("new_module.py"),
)

# Extract code to function
result = await extract_function(
    path=Path("file.py"),
    start_line=10,
    end_line=15,
    new_name="extracted_helper",
)
```

### 19j: Configurable Output Verbosity
Unified output system for CLI tools.

```python
from moss.output import Output, Verbosity, configure_output

# Configure globally
configure_output(verbosity=Verbosity.VERBOSE, no_color=True)

# Or create custom instance
output = Output(verbosity=Verbosity.DEBUG)
output.info("Processing...")
output.debug("Detailed info")
output.success("Done!")
output.error("Something failed")

# JSON output for scripting
output.use_json()
output.data({"files": 42, "errors": 0})
```

Verbosity levels:
- `QUIET`: Errors only
- `NORMAL`: Standard output (default)
- `VERBOSE`: Additional details
- `DEBUG`: Everything

Formatters:
- `TextFormatter`: Human-readable with colors/emoji
- `JSONFormatter`: Machine-readable JSON
- `CompactFormatter`: Minimal single-line format

## Module Reference

| Feature | Module | Tests |
|---------|--------|-------|
| Visual CFG | `moss.visualization` | `tests/test_visualization.py` |
| LSP Server | `moss.lsp_server` | `tests/test_lsp_server.py` |
| Live CFG | `moss.live_cfg` | `tests/test_live_cfg.py` |
| Progress | `moss.progress` | `tests/test_progress.py` |
| Refactoring | `moss.refactoring` | `tests/test_refactoring.py` |
| Output | `moss.output` | `tests/test_output.py` |
