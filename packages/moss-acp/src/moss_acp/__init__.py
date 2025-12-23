"""moss-acp: Agent Client Protocol server for moss.

Provides ACP integration for IDEs like Zed and JetBrains, enabling
moss to work as an AI coding agent inside editors.

ACP is the inverse of MCP:
- MCP: LLM connects to moss as a tool provider
- ACP: Editor connects to moss as an AI coding agent

Protocol: JSON-RPC 2.0 over stdio (stdin/stdout)

Features:
- Multi-file editing with shadow git safety
- Streaming responses for real-time feedback
- Tool calls for code analysis
- Terminal integration for running commands

Example:
    # Run the ACP server (editor will spawn this)
    moss-acp

    # In Zed's settings.json:
    {
        "agent_servers": {
            "moss": {
                "command": "moss-acp"
            }
        }
    }

Spec: https://agentclientprotocol.com
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# Lazy import to avoid loading heavy dependencies at import time
_server = None


def get_server():
    """Get the ACP server instance."""
    global _server
    if _server is None:
        try:
            from moss.acp_server import ACPServer

            _server = ACPServer()
        except ImportError as e:
            raise ImportError(
                f"Failed to import ACP server. Ensure moss is installed: {e}"
            ) from e
    return _server


def run_server():
    """Entry point for moss-acp command."""
    try:
        from moss.acp_server import main

        main()
    except ImportError as e:
        print(f"Error: Failed to import ACP server: {e}")
        raise SystemExit(1)


__all__ = ["get_server", "run_server"]
