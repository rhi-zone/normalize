"""moss-lsp: Language Server Protocol server for moss.

Provides LSP integration for IDEs and editors, exposing moss's
code analysis capabilities through standard LSP features.

Features:
- Diagnostics: Report complexity warnings, code smells
- Hover: Show function metrics (CFG complexity, node count)
- Document Symbols: Show code structure from skeleton view
- Code Actions: Suggest fixes from autofix system
- Go to Definition: Navigate via anchor resolution

Example:
    # Start the LSP server
    moss-lsp

    # In VS Code settings.json (with generic LSP client):
    {
        "languageServerExample.serverPath": "moss-lsp"
    }

    # In Neovim with nvim-lspconfig:
    require('lspconfig').moss.setup{
        cmd = { "moss-lsp" }
    }
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# Lazy import to avoid loading heavy dependencies at import time
_server = None


def get_server():
    """Get the LSP server instance."""
    global _server
    if _server is None:
        try:
            from moss.lsp_server import create_server

            _server = create_server()
        except ImportError as e:
            raise ImportError(
                f"Failed to import LSP server. Ensure moss[lsp] is installed: {e}"
            ) from e
    return _server


def run_server():
    """Entry point for moss-lsp command."""
    try:
        from moss.lsp_server import main

        main()
    except ImportError as e:
        print(f"Error: LSP dependencies not installed. Install with: pip install 'moss[lsp]'")
        print(f"Details: {e}")
        raise SystemExit(1)


__all__ = ["get_server", "run_server"]
