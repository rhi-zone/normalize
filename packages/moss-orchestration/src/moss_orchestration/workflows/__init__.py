"""Workflow package.

TOML-based workflows using composable execution primitives.
See src/moss/execution/__init__.py for the core execution engine.
"""

from .templates import TEMPLATES

__all__ = ["TEMPLATES"]
