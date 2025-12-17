"""Tests for synthesis configuration presets."""

from __future__ import annotations

import pytest

from moss.synthesis import (
    PresetName,
    SynthesisPreset,
    get_preset,
    get_preset_descriptions,
    list_presets,
    register_preset,
)
from moss.synthesis.framework import SynthesisConfig


class TestPresets:
    """Tests for preset retrieval."""

    def test_list_presets(self):
        """Test listing available presets."""
        presets = list_presets()
        assert "default" in presets
        assert "research" in presets
        assert "production" in presets
        assert "minimal" in presets

    def test_get_default_preset(self):
        """Test getting default preset."""
        preset = get_preset("default")
        assert preset.name == "default"
        assert isinstance(preset.config, SynthesisConfig)
        assert "type_driven" in preset.strategies
        assert "test_driven" in preset.strategies

    def test_get_research_preset(self):
        """Test getting research preset."""
        preset = get_preset("research")
        assert preset.name == "research"
        assert preset.config.max_iterations == 200
        assert preset.config.parallel_subproblems is True
        assert len(preset.strategies) == 3  # All strategies

    def test_get_production_preset(self):
        """Test getting production preset."""
        preset = get_preset("production")
        assert preset.name == "production"
        assert preset.config.max_iterations == 20
        assert preset.config.parallel_subproblems is False
        assert "velocity" in preset.policies

    def test_get_minimal_preset(self):
        """Test getting minimal preset."""
        preset = get_preset("minimal")
        assert preset.name == "minimal"
        assert preset.config.max_iterations == 10
        assert len(preset.validators) == 0

    def test_get_unknown_preset(self):
        """Test getting unknown preset raises error."""
        with pytest.raises(ValueError) as exc_info:
            get_preset("nonexistent")
        assert "Unknown preset" in str(exc_info.value)

    def test_preset_descriptions(self):
        """Test getting preset descriptions."""
        descriptions = get_preset_descriptions()
        assert "default" in descriptions
        assert isinstance(descriptions["default"], str)
        assert len(descriptions["default"]) > 0


class TestPresetConfig:
    """Tests for preset configuration values."""

    def test_default_config_values(self):
        """Test default preset has sensible config."""
        preset = get_preset("default")
        config = preset.config

        assert config.max_iterations == 50
        assert config.max_depth == 10
        assert config.parallel_subproblems is False
        assert config.stop_on_first_valid is True
        assert config.max_validation_retries == 3

    def test_research_config_values(self):
        """Test research preset has aggressive config."""
        preset = get_preset("research")
        config = preset.config

        assert config.max_iterations == 200
        assert config.max_depth == 20
        assert config.parallel_subproblems is True
        assert config.stop_on_first_valid is False
        assert config.max_validation_retries == 5

    def test_production_config_values(self):
        """Test production preset has conservative config."""
        preset = get_preset("production")
        config = preset.config

        assert config.max_iterations == 20
        assert config.max_depth == 5
        assert config.parallel_subproblems is False
        assert config.max_validation_retries == 2


class TestPresetStrategies:
    """Tests for preset strategy selection."""

    def test_default_strategies(self):
        """Test default preset strategies."""
        preset = get_preset("default")
        strategies = preset.get_strategies()

        assert len(strategies) == 2
        strategy_names = [s.metadata.name for s in strategies]
        assert "type_driven" in strategy_names
        assert "test_driven" in strategy_names

    def test_research_strategies(self):
        """Test research preset has all strategies."""
        preset = get_preset("research")
        strategies = preset.get_strategies()

        assert len(strategies) == 3
        strategy_names = [s.metadata.name for s in strategies]
        assert "type_driven" in strategy_names
        assert "test_driven" in strategy_names
        assert "pattern_based" in strategy_names

    def test_production_strategies(self):
        """Test production preset has safe strategies."""
        preset = get_preset("production")
        strategies = preset.get_strategies()

        assert len(strategies) == 2
        strategy_names = [s.metadata.name for s in strategies]
        # Production should have pattern_based (most predictable) and test_driven
        assert "pattern_based" in strategy_names
        assert "test_driven" in strategy_names


class TestCustomPresets:
    """Tests for custom preset registration."""

    def test_register_custom_preset(self):
        """Test registering a custom preset."""
        custom = SynthesisPreset(
            name="custom_test",
            description="Custom test preset",
            config=SynthesisConfig(max_iterations=100),
            strategies=["type_driven"],
            validators=["pytest"],
        )

        register_preset(custom)

        # Should be retrievable
        retrieved = get_preset("custom_test")
        assert retrieved.name == "custom_test"
        assert retrieved.config.max_iterations == 100

    def test_custom_preset_in_list(self):
        """Test custom preset appears in list."""
        custom = SynthesisPreset(
            name="another_custom",
            description="Another custom preset",
            config=SynthesisConfig(),
        )

        register_preset(custom)

        presets = list_presets()
        assert "another_custom" in presets


class TestPresetName:
    """Tests for PresetName enum."""

    def test_preset_name_values(self):
        """Test PresetName enum values."""
        assert PresetName.DEFAULT.value == "default"
        assert PresetName.RESEARCH.value == "research"
        assert PresetName.PRODUCTION.value == "production"
        assert PresetName.MINIMAL.value == "minimal"

    def test_preset_name_with_get_preset(self):
        """Test using PresetName enum with get_preset."""
        preset = get_preset(PresetName.DEFAULT.value)
        assert preset.name == "default"
