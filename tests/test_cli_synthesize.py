"""Tests for the synthesize CLI command."""

from __future__ import annotations

import argparse

import pytest

from moss.cli import cmd_synthesize
from moss.output import set_output


@pytest.fixture(autouse=True)
def reset_output():
    """Reset global output before each test."""
    set_output(None)


class TestCmdSynthesize:
    """Tests for cmd_synthesize CLI command."""

    def test_dry_run_pattern_based(self, capsys):
        """Test dry-run with pattern-based strategy."""
        args = argparse.Namespace(
            description="Build a REST API with CRUD for users",
            type_signature=None,
            examples=None,
            constraints=None,
            strategy="pattern_based",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        assert "Build a REST API" in captured.out
        assert "pattern_based" in captured.out
        assert "dry-run" in captured.out

    def test_dry_run_type_driven(self, capsys):
        """Test dry-run with type-driven strategy."""
        args = argparse.Namespace(
            description="Convert list of ints to strings",
            type_signature="List[int] -> List[str]",
            examples=None,
            constraints=None,
            strategy="type_driven",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        assert "type_driven" in captured.out
        assert "Transform element" in captured.out

    def test_show_decomposition(self, capsys):
        """Test showing decomposition tree."""
        args = argparse.Namespace(
            description="Build authentication with login",
            type_signature=None,
            examples=None,
            constraints=None,
            strategy="pattern_based",
            max_depth=5,
            show_decomposition=True,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        assert "Decomposition" in captured.out

    def test_parse_examples(self, capsys):
        """Test parsing input:output examples."""
        args = argparse.Namespace(
            description="Double a number",
            type_signature="int -> int",
            examples=["2:4", "5:10"],
            constraints=None,
            strategy="auto",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        assert "Examples: 2" in captured.out

    def test_parse_constraints(self, capsys):
        """Test parsing constraints."""
        args = argparse.Namespace(
            description="Sort numbers",
            type_signature="List[int] -> List[int]",
            examples=None,
            constraints=["must be stable", "O(n log n) complexity"],
            strategy="auto",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        assert "must be stable" in captured.out

    def test_invalid_example_format(self, capsys):
        """Test warning for invalid example format."""
        args = argparse.Namespace(
            description="Build a REST API with CRUD",  # Pattern that matches
            type_signature=None,
            examples=["invalid_no_colon"],
            constraints=None,
            strategy="auto",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        # Should still succeed (warning only)
        assert result == 0

        captured = capsys.readouterr()
        assert "Invalid example format" in captured.out

    def test_auto_strategy_selection(self, capsys):
        """Test automatic strategy selection."""
        args = argparse.Namespace(
            description="Build a REST API",
            type_signature=None,
            examples=None,
            constraints=None,
            strategy="auto",
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 0

        captured = capsys.readouterr()
        # Should auto-select pattern_based for REST API
        assert "pattern_based" in captured.out

    def test_no_applicable_strategy(self, capsys):
        """Test when no strategy matches (type_driven without type sig)."""
        args = argparse.Namespace(
            description="Do something generic",
            type_signature=None,  # No type signature
            examples=None,
            constraints=None,
            strategy="type_driven",  # Type-driven needs type signature
            max_depth=5,
            show_decomposition=False,
            dry_run=True,
            json=False,
            quiet=False,
            verbose=False,
            debug=False,
            no_color=True,
        )

        result = cmd_synthesize(args)
        assert result == 1  # Should fail

        captured = capsys.readouterr()
        assert "No applicable strategies" in captured.out
