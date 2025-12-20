"""Tests for DWIM custom configuration loading."""

import tempfile
from pathlib import Path

from moss.dwim_config import (
    CustomTool,
    DWIMConfig,
    IntentPattern,
    KeywordBoost,
    load_dwim_config,
    match_intent_patterns,
)


class TestIntentPattern:
    """Tests for IntentPattern matching."""

    def test_simple_pattern(self):
        """Test simple regex pattern matching."""
        pattern = IntentPattern(pattern="show.*code", tool="cli_expand")
        assert pattern.matches("show me the code")
        assert pattern.matches("show code")
        assert not pattern.matches("display source")

    def test_case_insensitive(self):
        """Pattern matching is case insensitive."""
        pattern = IntentPattern(pattern="EXPAND", tool="cli_expand")
        assert pattern.matches("expand")
        assert pattern.matches("EXPAND")
        assert pattern.matches("Expand")

    def test_word_boundary(self):
        """Test patterns with word boundaries."""
        pattern = IntentPattern(pattern=r"\bwho\s+calls\b", tool="callers")
        assert pattern.matches("who calls this function")
        assert not pattern.matches("whocalls")


class TestKeywordBoost:
    """Tests for KeywordBoost."""

    def test_default_boost(self):
        """Default boost is zero."""
        boost = KeywordBoost(keywords=["test"])
        assert boost.boost == 0.0

    def test_custom_boost(self):
        """Custom boost value is preserved."""
        boost = KeywordBoost(keywords=["test"], boost=0.2)
        assert boost.boost == 0.2


class TestDWIMConfig:
    """Tests for DWIMConfig dataclass."""

    def test_empty_config(self):
        """Empty config has empty collections."""
        config = DWIMConfig()
        assert config.aliases == {}
        assert config.keyword_boosts == {}
        assert config.tools == []
        assert config.intents == []

    def test_merge_aliases(self):
        """Merge combines aliases, later takes precedence."""
        config1 = DWIMConfig(aliases={"a": "tool1", "b": "tool2"})
        config2 = DWIMConfig(aliases={"b": "tool3", "c": "tool4"})
        merged = config1.merge(config2)
        assert merged.aliases == {"a": "tool1", "b": "tool3", "c": "tool4"}

    def test_merge_tools(self):
        """Merge concatenates tool lists."""
        config1 = DWIMConfig(tools=[CustomTool(name="t1", description="d1")])
        config2 = DWIMConfig(tools=[CustomTool(name="t2", description="d2")])
        merged = config1.merge(config2)
        assert len(merged.tools) == 2

    def test_merge_intents_sorted_by_priority(self):
        """Merge sorts intents by priority."""
        config1 = DWIMConfig(intents=[IntentPattern(pattern="a", tool="t1", priority=5)])
        config2 = DWIMConfig(intents=[IntentPattern(pattern="b", tool="t2", priority=10)])
        merged = config1.merge(config2)
        assert merged.intents[0].priority == 10
        assert merged.intents[1].priority == 5


class TestLoadDWIMConfig:
    """Tests for TOML config loading."""

    def test_load_nonexistent(self):
        """Loading nonexistent file returns empty config."""
        config = load_dwim_config(Path("/nonexistent/path.toml"))
        assert config.aliases == {}
        assert config.tools == []

    def test_load_aliases(self):
        """Load aliases from TOML."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write("""
[aliases]
ll = "skeleton"
cat = "cli_expand"
""")
            f.flush()
            config = load_dwim_config(Path(f.name))

        assert config.aliases["ll"] == "skeleton"
        assert config.aliases["cat"] == "cli_expand"

    def test_load_keyword_boosts(self):
        """Load keyword boosts from TOML."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write("""
[keywords.skeleton]
extra = ["structure", "layout"]
boost = 0.15
""")
            f.flush()
            config = load_dwim_config(Path(f.name))

        assert "skeleton" in config.keyword_boosts
        boost = config.keyword_boosts["skeleton"]
        assert "structure" in boost.keywords
        assert boost.boost == 0.15

    def test_load_custom_tools(self):
        """Load custom tool definitions from TOML."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write("""
[[tools]]
name = "my_linter"
description = "Run custom project linter"
keywords = ["lint", "check"]
parameters = ["path"]
""")
            f.flush()
            config = load_dwim_config(Path(f.name))

        assert len(config.tools) == 1
        tool = config.tools[0]
        assert tool.name == "my_linter"
        assert tool.description == "Run custom project linter"
        assert "lint" in tool.keywords
        assert "path" in tool.parameters

    def test_load_intent_patterns(self):
        """Load intent patterns from TOML."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write("""
[[intents]]
pattern = "show.*code"
tool = "cli_expand"
priority = 10

[[intents]]
pattern = "who calls"
tool = "callers"
priority = 5
""")
            f.flush()
            config = load_dwim_config(Path(f.name))

        assert len(config.intents) == 2
        # Should be sorted by priority
        assert config.intents[0].priority == 10
        assert config.intents[0].tool == "cli_expand"

    def test_load_complete_config(self):
        """Load a complete config with all sections."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write("""
[aliases]
ll = "skeleton"

[keywords.skeleton]
extra = ["overview"]
boost = 0.1

[[tools]]
name = "custom"
description = "Custom tool"
keywords = ["custom"]

[[intents]]
pattern = "test"
tool = "skeleton"
""")
            f.flush()
            config = load_dwim_config(Path(f.name))

        assert "ll" in config.aliases
        assert "skeleton" in config.keyword_boosts
        assert len(config.tools) == 1
        assert len(config.intents) == 1


class TestMatchIntentPatterns:
    """Tests for intent pattern matching."""

    def test_match_first_pattern(self):
        """First matching pattern (by priority) wins."""
        patterns = [
            IntentPattern(pattern="show", tool="tool1", priority=10),
            IntentPattern(pattern="show.*code", tool="tool2", priority=5),
        ]
        # Patterns should already be sorted by priority
        patterns.sort(key=lambda p: p.priority, reverse=True)

        assert match_intent_patterns("show code", patterns) == "tool1"

    def test_no_match(self):
        """Return None when no pattern matches."""
        patterns = [IntentPattern(pattern="xyz", tool="tool1")]
        assert match_intent_patterns("abc", patterns) is None

    def test_empty_patterns(self):
        """Empty pattern list returns None."""
        assert match_intent_patterns("anything", []) is None
