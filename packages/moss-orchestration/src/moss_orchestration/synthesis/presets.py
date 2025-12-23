"""Synthesis configuration presets.

Provides pre-configured synthesis settings for different use cases:
- default: Balanced for normal use
- research: Aggressive with all strategies
- production: Conservative with safe strategies only

Usage:
    from moss_orchestration.synthesis.presets import get_preset, SynthesisPreset

    config = get_preset("production")
    framework = SynthesisFramework(config=config)
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import TYPE_CHECKING

from .framework import SynthesisConfig

if TYPE_CHECKING:
    from .strategy import DecompositionStrategy


class PresetName(Enum):
    """Available preset names."""

    DEFAULT = "default"
    RESEARCH = "research"
    PRODUCTION = "production"
    MINIMAL = "minimal"


@dataclass
class SynthesisPreset:
    """A complete synthesis preset configuration.

    Combines framework config with strategy and validator selections.
    """

    name: str
    description: str
    config: SynthesisConfig
    strategies: list[str] = field(default_factory=list)
    validators: list[str] = field(default_factory=list)
    policies: list[str] = field(default_factory=list)

    def get_strategies(self) -> list[DecompositionStrategy]:
        """Instantiate the configured strategies."""
        from .strategies import (
            PatternBasedDecomposition,
            TestDrivenDecomposition,
            TypeDrivenDecomposition,
        )

        strategy_map = {
            "type_driven": TypeDrivenDecomposition,
            "test_driven": TestDrivenDecomposition,
            "pattern_based": PatternBasedDecomposition,
        }

        result = []
        for name in self.strategies:
            if name in strategy_map:
                result.append(strategy_map[name]())

        return result


# =============================================================================
# Preset Definitions
# =============================================================================


def _default_preset() -> SynthesisPreset:
    """Default preset - balanced for normal use."""
    return SynthesisPreset(
        name="default",
        description="Balanced synthesis for normal use cases",
        config=SynthesisConfig(
            max_iterations=50,
            max_depth=10,
            parallel_subproblems=False,  # Sequential for safety
            stop_on_first_valid=True,
            max_validation_retries=3,
            validation_timeout_ms=30000,
            prefer_templates=True,
        ),
        strategies=["type_driven", "test_driven"],
        validators=["pytest"],
        policies=["velocity"],
    )


def _research_preset() -> SynthesisPreset:
    """Research preset - aggressive exploration."""
    return SynthesisPreset(
        name="research",
        description="Aggressive synthesis with all strategies for research",
        config=SynthesisConfig(
            max_iterations=200,
            max_depth=20,
            parallel_subproblems=True,  # Aggressive parallelism
            stop_on_first_valid=False,  # Explore all solutions
            max_validation_retries=5,
            validation_timeout_ms=60000,
            prefer_templates=False,  # Try all generators
        ),
        strategies=["type_driven", "test_driven", "pattern_based"],
        validators=["pytest", "mypy"],
        policies=[],  # No limits for research
    )


def _production_preset() -> SynthesisPreset:
    """Production preset - conservative and safe."""
    return SynthesisPreset(
        name="production",
        description="Conservative synthesis for production use",
        config=SynthesisConfig(
            max_iterations=20,
            max_depth=5,
            parallel_subproblems=False,  # Sequential for safety
            stop_on_first_valid=True,
            max_validation_retries=2,
            validation_timeout_ms=15000,
            prefer_templates=True,  # Prefer tested templates
        ),
        strategies=["pattern_based", "test_driven"],  # Safe strategies only
        validators=["pytest", "mypy"],
        policies=["velocity", "quarantine", "resource_limit"],
    )


def _minimal_preset() -> SynthesisPreset:
    """Minimal preset - fast, limited synthesis."""
    return SynthesisPreset(
        name="minimal",
        description="Fast synthesis with minimal resources",
        config=SynthesisConfig(
            max_iterations=10,
            max_depth=3,
            parallel_subproblems=False,
            stop_on_first_valid=True,
            max_validation_retries=1,
            validation_timeout_ms=5000,
            prefer_templates=True,
        ),
        strategies=["pattern_based"],
        validators=[],  # No validation in minimal mode
        policies=[],
    )


# =============================================================================
# Preset Registry
# =============================================================================


_PRESETS: dict[str, SynthesisPreset] = {}


def _ensure_presets() -> None:
    """Ensure presets are registered."""
    if _PRESETS:
        return

    _PRESETS["default"] = _default_preset()
    _PRESETS["research"] = _research_preset()
    _PRESETS["production"] = _production_preset()
    _PRESETS["minimal"] = _minimal_preset()


def get_preset(name: str) -> SynthesisPreset:
    """Get a synthesis preset by name.

    Args:
        name: Preset name (default, research, production, minimal)

    Returns:
        SynthesisPreset configuration

    Raises:
        ValueError: If preset name is not found
    """
    _ensure_presets()

    if name not in _PRESETS:
        available = ", ".join(_PRESETS.keys())
        raise ValueError(f"Unknown preset '{name}'. Available: {available}")

    return _PRESETS[name]


def list_presets() -> list[str]:
    """List available preset names."""
    _ensure_presets()
    return list(_PRESETS.keys())


def get_preset_descriptions() -> dict[str, str]:
    """Get descriptions for all presets."""
    _ensure_presets()
    return {name: preset.description for name, preset in _PRESETS.items()}


def register_preset(preset: SynthesisPreset) -> None:
    """Register a custom preset.

    Args:
        preset: The preset to register
    """
    _ensure_presets()
    _PRESETS[preset.name] = preset


__all__ = [
    "PresetName",
    "SynthesisPreset",
    "get_preset",
    "get_preset_descriptions",
    "list_presets",
    "register_preset",
]
