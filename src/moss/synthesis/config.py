"""Synthesis configuration system.

This module provides configuration for the synthesis subsystem,
supporting both programmatic configuration and TOML-based configuration.

Configuration can be specified in moss.toml:

```toml
[synthesis]
preset = "default"  # or "research", "production", "minimal"
max_iterations = 50
max_depth = 10
parallel_subproblems = false

[synthesis.generators]
enabled = ["placeholder", "template"]
template_dirs = ["templates/", "~/.moss/templates/"]

[synthesis.validators]
enabled = ["pytest", "mypy"]
timeout_ms = 30000
max_retries = 3

[synthesis.strategies]
enabled = ["type_driven", "test_driven", "pattern_based"]

[synthesis.learning]
enabled = true
max_history = 1000
```

Usage:
    from moss.synthesis.config import SynthesisConfigLoader, load_synthesis_config

    # Load from TOML
    config = load_synthesis_config(Path("moss.toml"))

    # Or programmatically
    config = SynthesisConfigLoader().with_preset("research").build()
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .framework import SynthesisConfig
from .presets import get_preset, list_presets


@dataclass
class GeneratorConfig:
    """Configuration for code generators."""

    enabled: list[str] = field(default_factory=lambda: ["placeholder", "template"])
    template_dirs: list[Path] = field(default_factory=list)
    prefer_templates: bool = True

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "enabled": self.enabled,
            "template_dirs": [str(p) for p in self.template_dirs],
            "prefer_templates": self.prefer_templates,
        }


@dataclass
class ValidatorConfig:
    """Configuration for synthesis validators."""

    enabled: list[str] = field(default_factory=lambda: ["pytest"])
    timeout_ms: int = 30000
    max_retries: int = 3

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "enabled": self.enabled,
            "timeout_ms": self.timeout_ms,
            "max_retries": self.max_retries,
        }


@dataclass
class StrategyConfig:
    """Configuration for decomposition strategies."""

    enabled: list[str] = field(
        default_factory=lambda: ["type_driven", "test_driven", "pattern_based"]
    )
    auto_discover: bool = True

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "enabled": self.enabled,
            "auto_discover": self.auto_discover,
        }


@dataclass
class LearningConfig:
    """Configuration for strategy learning."""

    enabled: bool = True
    max_history: int = 1000
    learning_rate: float = 0.1

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "enabled": self.enabled,
            "max_history": self.max_history,
            "learning_rate": self.learning_rate,
        }


@dataclass
class BruteForceConfig:
    """Configuration for brute-force mode with fast/small models.

    Uses higher sample counts and voting to compensate for lower model quality.
    Optimized for local inference with models like Phi-3, Qwen2.5-Coder, etc.

    Example in moss.toml:
        [synthesis.brute_force]
        enabled = true
        n_samples = 5
        temperature = 0.7
        voting_strategy = "majority"  # or "first_valid", "consensus"
        parallel = true
    """

    enabled: bool = False
    n_samples: int = 5  # Number of samples per generation
    temperature: float = 0.7  # Higher temp for diversity
    voting_strategy: str = "majority"  # How to pick winner
    parallel: bool = True  # Generate samples in parallel
    require_consensus: float = 0.6  # For "consensus" strategy: min agreement ratio
    fallback_to_best: bool = True  # If no majority, use highest-scored

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "enabled": self.enabled,
            "n_samples": self.n_samples,
            "temperature": self.temperature,
            "voting_strategy": self.voting_strategy,
            "parallel": self.parallel,
            "require_consensus": self.require_consensus,
            "fallback_to_best": self.fallback_to_best,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> BruteForceConfig:
        """Create from dictionary."""
        return cls(
            enabled=data.get("enabled", False),
            n_samples=data.get("n_samples", 5),
            temperature=data.get("temperature", 0.7),
            voting_strategy=data.get("voting_strategy", "majority"),
            parallel=data.get("parallel", True),
            require_consensus=data.get("require_consensus", 0.6),
            fallback_to_best=data.get("fallback_to_best", True),
        )


@dataclass
class SynthesisConfigWrapper:
    """Complete synthesis configuration.

    Combines framework config with plugin configurations.
    """

    # Core synthesis settings
    preset: str | None = "default"
    max_iterations: int = 50
    max_depth: int = 10
    parallel_subproblems: bool = False
    stop_on_first_valid: bool = True

    # Plugin configurations
    generators: GeneratorConfig = field(default_factory=GeneratorConfig)
    validators: ValidatorConfig = field(default_factory=ValidatorConfig)
    strategies: StrategyConfig = field(default_factory=StrategyConfig)
    learning: LearningConfig = field(default_factory=LearningConfig)
    brute_force: BruteForceConfig = field(default_factory=BruteForceConfig)

    def build_framework_config(self) -> SynthesisConfig:
        """Build a SynthesisConfig for the framework."""
        return SynthesisConfig(
            max_iterations=self.max_iterations,
            max_depth=self.max_depth,
            parallel_subproblems=self.parallel_subproblems,
            stop_on_first_valid=self.stop_on_first_valid,
            max_validation_retries=self.validators.max_retries,
            validation_timeout_ms=self.validators.timeout_ms,
            prefer_templates=self.generators.prefer_templates,
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary (for TOML serialization)."""
        return {
            "preset": self.preset,
            "max_iterations": self.max_iterations,
            "max_depth": self.max_depth,
            "parallel_subproblems": self.parallel_subproblems,
            "stop_on_first_valid": self.stop_on_first_valid,
            "generators": self.generators.to_dict(),
            "validators": self.validators.to_dict(),
            "strategies": self.strategies.to_dict(),
            "learning": self.learning.to_dict(),
            "brute_force": self.brute_force.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> SynthesisConfigWrapper:
        """Create from dictionary (from TOML)."""
        config = cls()

        # Core settings
        if "preset" in data:
            config.preset = data["preset"]
        if "max_iterations" in data:
            config.max_iterations = data["max_iterations"]
        if "max_depth" in data:
            config.max_depth = data["max_depth"]
        if "parallel_subproblems" in data:
            config.parallel_subproblems = data["parallel_subproblems"]
        if "stop_on_first_valid" in data:
            config.stop_on_first_valid = data["stop_on_first_valid"]

        # Generator config
        if "generators" in data:
            gen_data = data["generators"]
            if "enabled" in gen_data:
                config.generators.enabled = gen_data["enabled"]
            if "template_dirs" in gen_data:
                config.generators.template_dirs = [
                    Path(p).expanduser() for p in gen_data["template_dirs"]
                ]
            if "prefer_templates" in gen_data:
                config.generators.prefer_templates = gen_data["prefer_templates"]

        # Validator config
        if "validators" in data:
            val_data = data["validators"]
            if "enabled" in val_data:
                config.validators.enabled = val_data["enabled"]
            if "timeout_ms" in val_data:
                config.validators.timeout_ms = val_data["timeout_ms"]
            if "max_retries" in val_data:
                config.validators.max_retries = val_data["max_retries"]

        # Strategy config
        if "strategies" in data:
            strat_data = data["strategies"]
            if "enabled" in strat_data:
                config.strategies.enabled = strat_data["enabled"]
            if "auto_discover" in strat_data:
                config.strategies.auto_discover = strat_data["auto_discover"]

        # Learning config
        if "learning" in data:
            learn_data = data["learning"]
            if "enabled" in learn_data:
                config.learning.enabled = learn_data["enabled"]
            if "max_history" in learn_data:
                config.learning.max_history = learn_data["max_history"]
            if "learning_rate" in learn_data:
                config.learning.learning_rate = learn_data["learning_rate"]

        # Brute force config
        if "brute_force" in data:
            config.brute_force = BruteForceConfig.from_dict(data["brute_force"])

        return config

    @classmethod
    def from_preset(cls, preset_name: str) -> SynthesisConfigWrapper:
        """Create from a preset."""
        preset = get_preset(preset_name)

        config = cls()
        config.preset = preset_name
        config.max_iterations = preset.config.max_iterations
        config.max_depth = preset.config.max_depth
        config.parallel_subproblems = preset.config.parallel_subproblems
        config.stop_on_first_valid = preset.config.stop_on_first_valid
        config.validators.max_retries = preset.config.max_validation_retries
        config.validators.timeout_ms = preset.config.validation_timeout_ms
        config.generators.prefer_templates = preset.config.prefer_templates
        config.strategies.enabled = preset.strategies
        config.validators.enabled = preset.validators

        return config


class SynthesisConfigLoader:
    """Fluent builder for synthesis configuration."""

    def __init__(self) -> None:
        self._config = SynthesisConfigWrapper()

    def with_preset(self, preset_name: str) -> SynthesisConfigLoader:
        """Start from a preset."""
        self._config = SynthesisConfigWrapper.from_preset(preset_name)
        return self

    def with_iterations(self, max_iterations: int) -> SynthesisConfigLoader:
        """Set max iterations."""
        self._config.max_iterations = max_iterations
        return self

    def with_depth(self, max_depth: int) -> SynthesisConfigLoader:
        """Set max decomposition depth."""
        self._config.max_depth = max_depth
        return self

    def with_parallel(self, enabled: bool = True) -> SynthesisConfigLoader:
        """Enable/disable parallel subproblem solving."""
        self._config.parallel_subproblems = enabled
        return self

    def with_generators(self, *names: str) -> SynthesisConfigLoader:
        """Enable specific generators."""
        self._config.generators.enabled = list(names)
        return self

    def with_template_dirs(self, *dirs: Path | str) -> SynthesisConfigLoader:
        """Add template directories."""
        self._config.generators.template_dirs = [
            Path(d).expanduser() if isinstance(d, str) else d for d in dirs
        ]
        return self

    def with_validators(self, *names: str) -> SynthesisConfigLoader:
        """Enable specific validators."""
        self._config.validators.enabled = list(names)
        return self

    def with_validation_timeout(self, timeout_ms: int) -> SynthesisConfigLoader:
        """Set validation timeout."""
        self._config.validators.timeout_ms = timeout_ms
        return self

    def with_validation_retries(self, max_retries: int) -> SynthesisConfigLoader:
        """Set max validation retries."""
        self._config.validators.max_retries = max_retries
        return self

    def with_strategies(self, *names: str) -> SynthesisConfigLoader:
        """Enable specific strategies."""
        self._config.strategies.enabled = list(names)
        return self

    def with_learning(
        self,
        enabled: bool = True,
        max_history: int | None = None,
    ) -> SynthesisConfigLoader:
        """Configure learning."""
        self._config.learning.enabled = enabled
        if max_history is not None:
            self._config.learning.max_history = max_history
        return self

    def build(self) -> SynthesisConfigWrapper:
        """Build the configuration."""
        return self._config


def load_synthesis_config(path: Path) -> SynthesisConfigWrapper:
    """Load synthesis configuration from a TOML file.

    Looks for [synthesis] section in the TOML file.

    Args:
        path: Path to TOML file (e.g., moss.toml)

    Returns:
        SynthesisConfigWrapper
    """
    if not path.exists():
        return SynthesisConfigWrapper()

    try:
        import tomllib
    except ImportError:
        try:
            import tomli as tomllib  # type: ignore
        except ImportError:
            # No TOML parser available, return defaults
            return SynthesisConfigWrapper()

    with open(path, "rb") as f:
        data = tomllib.load(f)

    if "synthesis" not in data:
        return SynthesisConfigWrapper()

    synth_data = data["synthesis"]

    # Check for preset first
    if "preset" in synth_data:
        config = SynthesisConfigWrapper.from_preset(synth_data["preset"])
        # Then apply overrides
        for key in ["max_iterations", "max_depth", "parallel_subproblems", "stop_on_first_valid"]:
            if key in synth_data:
                setattr(config, key, synth_data[key])
        return config

    return SynthesisConfigWrapper.from_dict(synth_data)


def get_default_config() -> SynthesisConfigWrapper:
    """Get the default synthesis configuration."""
    return SynthesisConfigWrapper.from_preset("default")


def list_available_presets() -> list[str]:
    """List available synthesis presets."""
    return list_presets()


__all__ = [
    "BruteForceConfig",
    "GeneratorConfig",
    "LearningConfig",
    "StrategyConfig",
    "SynthesisConfigLoader",
    "SynthesisConfigWrapper",
    "ValidatorConfig",
    "get_default_config",
    "list_available_presets",
    "load_synthesis_config",
]
