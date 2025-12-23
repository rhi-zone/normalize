"""moss-tui: Terminal UI for moss code intelligence.

Provides an interactive terminal interface for exploring and
analyzing codebases using Textual.

Features:
- Tree navigation: Browse codebase structure
- Skeleton view: See function signatures without implementation
- Analysis modes: Complexity, security, dependencies
- Command palette: Quick access to all features
- Task management: Track analysis tasks

Example:
    # Start the TUI
    moss-tui

    # Or from the main moss CLI
    moss tui

    # Start in a specific directory
    moss-tui /path/to/project
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pathlib import Path

# Lazy import to avoid loading heavy dependencies at import time
_app = None


def get_app(project_root: Path | None = None):
    """Get the TUI app instance.

    Args:
        project_root: Optional project root directory. Defaults to cwd.

    Returns:
        MossApp instance ready to run.
    """
    try:
        from moss.tui import MossApp

        return MossApp(project_root=project_root)
    except ImportError as e:
        raise ImportError(
            f"Failed to import TUI. Ensure moss[tui] is installed: {e}"
        ) from e


def run_app():
    """Entry point for moss-tui command."""
    import argparse
    from pathlib import Path

    parser = argparse.ArgumentParser(description="Run moss TUI")
    parser.add_argument(
        "project",
        nargs="?",
        default=".",
        help="Project directory to analyze (default: current directory)",
    )
    args = parser.parse_args()

    try:
        app = get_app(Path(args.project).resolve())
        app.run()
    except ImportError as e:
        print(f"Error: TUI dependencies not installed. Install with: pip install 'moss[tui]'")
        print(f"Details: {e}")
        raise SystemExit(1)


__all__ = ["get_app", "run_app"]
