"""Tests for TOML navigation with jq-like queries."""

import tempfile
from pathlib import Path

import pytest

from moss.toml_nav import (
    format_result,
    list_keys,
    parse_toml,
    query,
    summarize_toml,
    to_json,
)


@pytest.fixture
def sample_data():
    """Sample TOML-like data for testing."""
    return {
        "project": {
            "name": "my-project",
            "version": "1.0.0",
            "authors": ["Alice", "Bob"],
        },
        "dependencies": {
            "requests": "2.28.0",
            "click": "8.1.3",
        },
        "tool": {
            "ruff": {
                "line-length": 100,
                "select": ["E", "W", "F"],
            }
        },
    }


@pytest.fixture
def toml_file(sample_data):
    """Create a temporary TOML file."""
    content = """
[project]
name = "my-project"
version = "1.0.0"
authors = ["Alice", "Bob"]

[dependencies]
requests = "2.28.0"
click = "8.1.3"

[tool.ruff]
line-length = 100
select = ["E", "W", "F"]
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
        f.write(content)
        return Path(f.name)


class TestParseToml:
    """Tests for parse_toml."""

    def test_parse_valid_toml(self, toml_file):
        """Parse a valid TOML file."""
        data = parse_toml(toml_file)
        assert data["project"]["name"] == "my-project"
        assert data["dependencies"]["requests"] == "2.28.0"

    def test_parse_nonexistent(self):
        """Raise FileNotFoundError for nonexistent file."""
        with pytest.raises(FileNotFoundError):
            parse_toml(Path("/nonexistent/file.toml"))


class TestQuery:
    """Tests for jq-like query."""

    def test_identity(self, sample_data):
        """Identity query returns the whole object."""
        assert query(sample_data, ".") == sample_data

    def test_simple_key(self, sample_data):
        """Access a simple key."""
        assert query(sample_data, ".project") == sample_data["project"]

    def test_nested_key(self, sample_data):
        """Access nested keys."""
        assert query(sample_data, ".project.name") == "my-project"
        assert query(sample_data, ".tool.ruff.line-length") == 100

    def test_hyphenated_key(self, sample_data):
        """Keys with hyphens work."""
        assert query(sample_data, ".tool.ruff.line-length") == 100

    def test_array_index(self, sample_data):
        """Access array by index."""
        assert query(sample_data, ".project.authors[0]") == "Alice"
        assert query(sample_data, ".project.authors[1]") == "Bob"

    def test_negative_index(self, sample_data):
        """Negative index accesses from end."""
        assert query(sample_data, ".project.authors[-1]") == "Bob"

    def test_wildcard(self, sample_data):
        """Wildcard returns all values."""
        result = query(sample_data, ".dependencies.*")
        assert "2.28.0" in result
        assert "8.1.3" in result

    def test_pipe_keys(self, sample_data):
        """Pipe to keys function."""
        result = query(sample_data, ".dependencies|keys")
        assert "requests" in result
        assert "click" in result

    def test_pipe_length(self, sample_data):
        """Pipe to length function."""
        assert query(sample_data, ".project.authors|length") == 2
        assert query(sample_data, ".dependencies|length") == 2

    def test_pipe_type(self, sample_data):
        """Pipe to type function."""
        assert query(sample_data, ".project.name|type") == "str"
        assert query(sample_data, ".project.authors|type") == "list"
        assert query(sample_data, ".dependencies|type") == "dict"

    def test_pipe_first_last(self, sample_data):
        """Pipe to first/last functions."""
        assert query(sample_data, ".project.authors|first") == "Alice"
        assert query(sample_data, ".project.authors|last") == "Bob"

    def test_optional_key(self, sample_data):
        """Optional key access returns None if missing."""
        assert query(sample_data, ".nonexistent?") is None
        assert query(sample_data, ".project.missing?") is None

    def test_missing_key_raises(self, sample_data):
        """Non-optional missing key raises KeyError."""
        with pytest.raises(KeyError):
            query(sample_data, ".nonexistent")

    def test_index_out_of_range(self, sample_data):
        """Out of range index raises IndexError."""
        with pytest.raises(IndexError):
            query(sample_data, ".project.authors[10]")


class TestListKeys:
    """Tests for list_keys."""

    def test_list_all_keys(self, sample_data):
        """List all keys recursively."""
        keys = list_keys(sample_data)
        assert "project" in keys
        assert "project.name" in keys
        assert "project.version" in keys
        assert "dependencies" in keys
        assert "dependencies.requests" in keys
        assert "tool.ruff.line-length" in keys


class TestSummarizeToml:
    """Tests for summarize_toml."""

    def test_summary(self, sample_data):
        """Summary includes sections and stats."""
        summary = summarize_toml(sample_data)
        assert "project" in summary["sections"]
        assert "dependencies" in summary["sections"]
        assert "tool" in summary["sections"]
        assert summary["key_count"] > 0
        assert summary["nested_depth"] >= 3


class TestFormatResult:
    """Tests for format_result."""

    def test_format_string(self):
        """String is returned as-is."""
        assert format_result("hello") == "hello"

    def test_format_bool(self):
        """Boolean is lowercase."""
        assert format_result(True) == "true"
        assert format_result(False) == "false"

    def test_format_none(self):
        """None is null."""
        assert format_result(None) == "null"

    def test_format_dict_as_json(self):
        """Dict is formatted as JSON."""
        result = format_result({"a": 1})
        assert '"a"' in result
        assert "1" in result


class TestToJson:
    """Tests for to_json."""

    def test_to_json(self, sample_data):
        """Data is converted to JSON string."""
        result = to_json(sample_data)
        assert '"project"' in result
        assert '"name"' in result
        assert '"my-project"' in result
