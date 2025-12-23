"""moss-mcp: MCP server for moss code intelligence.

Provides Model Context Protocol server integration for LLM tools.

The MCP server exposes moss's code intelligence capabilities as tools
that LLMs can use. Two modes are available:

1. Single-tool mode (default): One 'moss' tool that accepts CLI-style commands
   - Token efficient (~50 tokens vs ~8K for full tool definitions)
   - Best for Claude and other LLMs

2. Multi-tool mode (--full): Separate tools for each capability
   - Better discoverability in IDEs
   - More structured tool calls

Example:
    # Run single-tool server
    moss-mcp

    # Run multi-tool server
    moss-mcp --full

    # In Claude Code's claude_desktop_config.json:
    {
        "mcpServers": {
            "moss": {
                "command": "moss-mcp"
            }
        }
    }
"""

from __future__ import annotations

import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# Lazy import to avoid loading heavy dependencies at import time
_server = None
_server_full = None


def get_server():
    """Get the single-tool MCP server."""
    global _server
    if _server is None:
        try:
            from moss.mcp_server import create_server

            _server = create_server()
        except ImportError as e:
            raise ImportError(
                f"Failed to import MCP server. Ensure moss is installed: {e}"
            ) from e
    return _server


def get_server_full():
    """Get the multi-tool MCP server."""
    global _server_full
    if _server_full is None:
        try:
            from moss.mcp_server_full import create_server

            _server_full = create_server()
        except ImportError as e:
            raise ImportError(
                f"Failed to import MCP server. Ensure moss is installed: {e}"
            ) from e
    return _server_full


def run_server():
    """Entry point for moss-mcp command."""
    import argparse

    parser = argparse.ArgumentParser(description="Run moss MCP server")
    parser.add_argument(
        "--full",
        action="store_true",
        help="Run multi-tool server instead of single-tool",
    )
    args = parser.parse_args()

    if args.full:
        from moss.mcp_server_full import main

        main()
    else:
        from moss.mcp_server import main

        main()


__all__ = ["get_server", "get_server_full", "run_server"]
