"""DWIM custom tool semantics configuration.

Loads user-defined tool mappings from:
- .moss/dwim.toml (project-specific)
- ~/.config/moss/dwim.toml (user-level)

TOML format:

    # Custom aliases (shortcut → tool)
    [aliases]
    ll = "skeleton"
    cat = "cli_expand"
    refs = "callers"

    # Additional keywords for existing tools
    [keywords.skeleton]
    extra = ["structure", "layout"]
    boost = 0.2  # Boost score by 20% when these match

    # Custom tool definitions
    [[tools]]
    name = "my_linter"
    description = "Run custom project linter"
    keywords = ["lint", "check", "style"]
    parameters = ["path"]

    # Intent patterns (regex → tool)
    [[intents]]
    pattern = "show.*code"
    tool = "cli_expand"
    priority = 10

    [[intents]]
    pattern = "who calls"
    tool = "callers"
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence

logger = logging.getLogger(__name__)


@dataclass
class IntentPattern:
    """A regex pattern that maps to a tool."""

    pattern: str
    tool: str
    priority: int = 0
    _compiled: re.Pattern | None = field(default=None, repr=False)

    def matches(self, query: str) -> bool:
        """Check if query matches this pattern."""
        if self._compiled is None:
            self._compiled = re.compile(self.pattern, re.IGNORECASE)
        return bool(self._compiled.search(query))


@dataclass
class KeywordBoost:
    """Additional keywords for an existing tool with optional score boost."""

    keywords: list[str] = field(default_factory=list)
    boost: float = 0.0  # Additive boost when these keywords match


@dataclass
class CustomTool:
    """A user-defined tool."""

    name: str
    description: str
    keywords: list[str] = field(default_factory=list)
    parameters: list[str] = field(default_factory=list)


@dataclass
class DWIMConfig:
    """Complete DWIM configuration."""

    # Custom aliases: shortcut -> canonical tool name
    aliases: dict[str, str] = field(default_factory=dict)

    # Keyword boosts per tool
    keyword_boosts: dict[str, KeywordBoost] = field(default_factory=dict)

    # Custom tool definitions
    tools: list[CustomTool] = field(default_factory=list)

    # Intent patterns (higher priority first)
    intents: list[IntentPattern] = field(default_factory=list)

    def merge(self, other: DWIMConfig) -> DWIMConfig:
        """Merge another config into this one (other takes precedence)."""
        merged_aliases = {**self.aliases, **other.aliases}
        merged_boosts = {**self.keyword_boosts, **other.keyword_boosts}
        merged_tools = self.tools + other.tools
        merged_intents = self.intents + other.intents
        # Sort intents by priority (higher first)
        merged_intents.sort(key=lambda i: i.priority, reverse=True)

        return DWIMConfig(
            aliases=merged_aliases,
            keyword_boosts=merged_boosts,
            tools=merged_tools,
            intents=merged_intents,
        )


def load_dwim_config(path: Path) -> DWIMConfig:
    """Load DWIM configuration from a TOML file.

    Args:
        path: Path to TOML file

    Returns:
        DWIMConfig object
    """
    import tomllib

    if not path.exists():
        return DWIMConfig()

    try:
        data = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as e:
        logger.warning("Failed to load DWIM config from %s: %s", path, e)
        return DWIMConfig()

    config = DWIMConfig()

    # Load aliases
    if "aliases" in data:
        for alias, target in data["aliases"].items():
            if isinstance(target, str):
                config.aliases[alias.lower()] = target

    # Load keyword boosts
    if "keywords" in data:
        for tool_name, boost_data in data["keywords"].items():
            if isinstance(boost_data, dict):
                config.keyword_boosts[tool_name] = KeywordBoost(
                    keywords=boost_data.get("extra", []),
                    boost=float(boost_data.get("boost", 0.0)),
                )

    # Load custom tools
    for tool_data in data.get("tools", []):
        if isinstance(tool_data, dict) and "name" in tool_data:
            config.tools.append(
                CustomTool(
                    name=tool_data["name"],
                    description=tool_data.get("description", ""),
                    keywords=tool_data.get("keywords", []),
                    parameters=tool_data.get("parameters", []),
                )
            )

    # Load intent patterns
    for intent_data in data.get("intents", []):
        if isinstance(intent_data, dict) and "pattern" in intent_data and "tool" in intent_data:
            config.intents.append(
                IntentPattern(
                    pattern=intent_data["pattern"],
                    tool=intent_data["tool"],
                    priority=intent_data.get("priority", 0),
                )
            )

    # Sort intents by priority
    config.intents.sort(key=lambda i: i.priority, reverse=True)

    return config


def load_all_configs(project_dir: Path | None = None) -> DWIMConfig:
    """Load DWIM configs from all sources.

    Order (later takes precedence):
    1. ~/.config/moss/dwim.toml (user-level)
    2. <project>/.moss/dwim.toml (project-level)

    Args:
        project_dir: Project directory (defaults to cwd)

    Returns:
        Merged DWIMConfig
    """
    config = DWIMConfig()

    # User-level config
    user_config_path = Path.home() / ".config" / "moss" / "dwim.toml"
    if user_config_path.exists():
        user_config = load_dwim_config(user_config_path)
        config = config.merge(user_config)

    # Project-level config
    if project_dir is None:
        project_dir = Path.cwd()
    project_config_path = project_dir / ".moss" / "dwim.toml"
    if project_config_path.exists():
        project_config = load_dwim_config(project_config_path)
        config = config.merge(project_config)

    return config


def match_intent_patterns(query: str, patterns: Sequence[IntentPattern]) -> str | None:
    """Match query against intent patterns.

    Args:
        query: User query string
        patterns: List of IntentPattern (should be sorted by priority)

    Returns:
        Tool name if matched, None otherwise
    """
    for pattern in patterns:
        if pattern.matches(query):
            return pattern.tool
    return None


# Global config cache
_cached_config: DWIMConfig | None = None
_cached_project_dir: Path | None = None


def get_config(project_dir: Path | None = None) -> DWIMConfig:
    """Get the cached DWIM config for a project.

    Reloads if project_dir changes.
    """
    global _cached_config, _cached_project_dir

    if project_dir is None:
        project_dir = Path.cwd()

    if _cached_config is None or _cached_project_dir != project_dir:
        _cached_config = load_all_configs(project_dir)
        _cached_project_dir = project_dir

    return _cached_config


def reload_config(project_dir: Path | None = None) -> DWIMConfig:
    """Force reload the DWIM config."""
    global _cached_config, _cached_project_dir
    _cached_config = None
    _cached_project_dir = None
    return get_config(project_dir)


__all__ = [
    "CustomTool",
    "DWIMConfig",
    "IntentPattern",
    "KeywordBoost",
    "get_config",
    "load_all_configs",
    "load_dwim_config",
    "match_intent_patterns",
    "reload_config",
]
