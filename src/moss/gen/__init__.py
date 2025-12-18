"""Interface generators for MossAPI.

This module provides automatic interface generation from the MossAPI:
- CLI: Generate argparse commands from API methods
- HTTP: Generate FastAPI routes from API methods
- MCP: Generate Model Context Protocol tools from API methods
- TUI: Generate Textual terminal UI from API methods
- OpenAPI: Generate OpenAPI specification from API
"""

from moss.gen.cli import CLIGenerator, generate_cli
from moss.gen.http import HTTPGenerator, generate_http, generate_openapi
from moss.gen.introspect import (
    APIMethod,
    APIParameter,
    SubAPI,
    introspect_api,
)
from moss.gen.mcp import MCPGenerator, generate_mcp, generate_mcp_definitions
from moss.gen.tui import TUIGenerator, generate_tui_groups, run_tui

__all__ = [
    "APIMethod",
    "APIParameter",
    "CLIGenerator",
    "HTTPGenerator",
    "MCPGenerator",
    "SubAPI",
    "TUIGenerator",
    "generate_cli",
    "generate_http",
    "generate_mcp",
    "generate_mcp_definitions",
    "generate_openapi",
    "generate_tui_groups",
    "introspect_api",
    "run_tui",
]
