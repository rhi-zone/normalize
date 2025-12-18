"""Preset configurations for moss commands.

Presets allow users to define named combinations of checks and output settings
for the `moss overview` command.

Example moss.toml configuration:

    [presets.ci]
    checks = ["health", "deps"]
    output = "compact"
    strict = true

    [presets.full]
    checks = ["health", "deps", "docs", "todos", "refs"]
    output = "json"

Usage:
    moss overview --preset ci
    moss --compact overview --preset full  # CLI flags override preset
"""

from __future__ import annotations

import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# Available checks that can be included in presets
AVAILABLE_CHECKS = {"health", "deps", "docs", "todos", "refs"}

# Built-in presets
BUILTIN_PRESETS: dict[str, dict[str, Any]] = {
    "ci": {
        "checks": ["health", "deps"],
        "output": "compact",
        "strict": True,
    },
    "quick": {
        "checks": ["health"],
        "output": "compact",
        "strict": False,
    },
    "full": {
        "checks": ["health", "deps", "docs", "todos", "refs"],
        "output": "markdown",
        "strict": False,
    },
}


@dataclass
class Preset:
    """A named preset configuration."""

    name: str
    checks: list[str] = field(default_factory=lambda: list(AVAILABLE_CHECKS))
    output: str = "markdown"  # "compact", "json", "markdown"
    strict: bool = False  # Exit non-zero on warnings

    def __post_init__(self):
        # Validate checks
        invalid = set(self.checks) - AVAILABLE_CHECKS
        if invalid:
            raise ValueError(f"Invalid checks in preset '{self.name}': {invalid}")
        # Validate output format
        if self.output not in ("compact", "json", "markdown"):
            raise ValueError(f"Invalid output format in preset '{self.name}': {self.output}")

    @classmethod
    def from_dict(cls, name: str, data: dict[str, Any]) -> Preset:
        """Create a Preset from a dictionary."""
        return cls(
            name=name,
            checks=data.get("checks", list(AVAILABLE_CHECKS)),
            output=data.get("output", "markdown"),
            strict=data.get("strict", False),
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "name": self.name,
            "checks": self.checks,
            "output": self.output,
            "strict": self.strict,
        }


def load_presets(root: Path) -> dict[str, Preset]:
    """Load presets from config files.

    Searches for moss.toml or pyproject.toml in the given directory
    and loads any [presets.*] sections.

    Args:
        root: Directory to search for config files

    Returns:
        Dictionary of preset name -> Preset
    """
    presets = {}

    # Start with built-in presets
    for name, data in BUILTIN_PRESETS.items():
        presets[name] = Preset.from_dict(name, data)

    # Try to load from moss.toml
    moss_toml = root / "moss.toml"
    if moss_toml.exists():
        try:
            with open(moss_toml, "rb") as f:
                data = tomllib.load(f)
            if "presets" in data:
                for name, preset_data in data["presets"].items():
                    presets[name] = Preset.from_dict(name, preset_data)
        except Exception:
            pass  # Ignore config errors, use defaults

    # Try to load from pyproject.toml
    pyproject = root / "pyproject.toml"
    if pyproject.exists():
        try:
            with open(pyproject, "rb") as f:
                data = tomllib.load(f)
            if "tool" in data and "moss" in data["tool"]:
                moss_config = data["tool"]["moss"]
                if "presets" in moss_config:
                    for name, preset_data in moss_config["presets"].items():
                        presets[name] = Preset.from_dict(name, preset_data)
        except Exception:
            pass  # Ignore config errors, use defaults

    return presets


def get_preset(name: str, root: Path) -> Preset | None:
    """Get a preset by name.

    Args:
        name: Preset name
        root: Project root for loading config

    Returns:
        Preset if found, None otherwise
    """
    presets = load_presets(root)
    return presets.get(name)


def list_presets(root: Path) -> list[Preset]:
    """List all available presets.

    Args:
        root: Project root for loading config

    Returns:
        List of all presets (built-in and custom)
    """
    presets = load_presets(root)
    return sorted(presets.values(), key=lambda p: p.name)
