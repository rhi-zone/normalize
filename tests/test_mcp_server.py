"""Tests for MCP server tool functions."""

from pathlib import Path

import pytest

from moss.gen.mcp import MCPGenerator
from moss.mcp_server import _execute_tool, _serialize_result


@pytest.fixture
def tools():
    """Generate MCP tools for testing."""
    gen = MCPGenerator()
    return gen.generate_tools()


class TestToolSkeleton:
    """Tests for skeleton tools."""

    @pytest.fixture
    def python_file(self, tmp_path: Path):
        """Create a Python file for testing."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("""
class Foo:
    '''A class.'''
    def bar(self) -> str:
        '''A method.'''
        return "hello"

def baz():
    '''A function.'''
    pass
""")
        return py_file

    def test_extracts_skeleton(self, tools, python_file: Path):
        result = _execute_tool("skeleton_extract", {"file_path": str(python_file)}, tools)

        assert len(result) >= 2  # Foo, baz
        names = [s.name for s in result]
        assert "Foo" in names
        assert "baz" in names

    def test_formats_skeleton(self, tools, python_file: Path):
        result = _execute_tool("skeleton_format", {"file_path": str(python_file)}, tools)

        assert isinstance(result, str)
        assert "Foo" in result
        assert "baz" in result

    def test_handles_nonexistent_path(self, tools):
        with pytest.raises(FileNotFoundError):
            _execute_tool("skeleton_extract", {"file_path": "/nonexistent/path"}, tools)


class TestToolAnchor:
    """Tests for anchor tools."""

    @pytest.fixture
    def python_file(self, tmp_path: Path):
        """Create a Python file for testing."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("""
class MyClass:
    def method(self): pass

def my_function():
    pass
""")
        return py_file

    def test_finds_anchors(self, tools, python_file: Path):
        result = _execute_tool(
            "anchor_find",
            {"file_path": str(python_file), "name": "my_function"},
            tools,
        )

        assert isinstance(result, list)
        assert len(result) >= 1

    def test_resolves_anchor(self, tools, python_file: Path):
        result = _execute_tool(
            "anchor_resolve",
            {"file_path": str(python_file), "name": "my_function"},
            tools,
        )

        assert result is not None
        assert hasattr(result, "lineno")


class TestToolCfg:
    """Tests for cfg tool."""

    @pytest.fixture
    def python_file(self, tmp_path: Path):
        """Create a Python file with control flow."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("""
def check(x):
    if x > 0:
        return "positive"
    else:
        return "non-positive"
""")
        return py_file

    def test_builds_cfg(self, tools, python_file: Path):
        result = _execute_tool("cfg_build", {"file_path": str(python_file)}, tools)

        assert isinstance(result, list)
        assert len(result) == 1
        assert result[0].name == "check"


class TestToolDeps:
    """Tests for dependencies tools."""

    @pytest.fixture
    def python_file(self, tmp_path: Path):
        """Create a Python file with imports."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("""
import os
from pathlib import Path

def public_func():
    pass

class PublicClass:
    pass
""")
        return py_file

    def test_extracts_deps(self, tools, python_file: Path):
        result = _execute_tool("dependencies_extract", {"file_path": str(python_file)}, tools)

        assert hasattr(result, "imports")
        assert hasattr(result, "exports")
        assert len(result.imports) >= 2

        modules = [i.module for i in result.imports]
        assert "os" in modules
        assert "pathlib" in modules

    def test_formats_deps(self, tools, python_file: Path):
        result = _execute_tool("dependencies_format", {"file_path": str(python_file)}, tools)

        assert isinstance(result, str)
        assert "os" in result


class TestToolDwim:
    """Tests for DWIM tools."""

    def test_list_tools(self, tools):
        result = _execute_tool("dwim_list_tools", {}, tools)

        assert isinstance(result, list)
        assert len(result) > 0
        names = [t.name for t in result]
        assert "skeleton" in names

    def test_resolve_tool(self, tools):
        result = _execute_tool("dwim_resolve_tool", {"tool_name": "skelton"}, tools)

        assert result.tool == "skeleton"
        assert result.confidence > 0.8

    def test_analyze_intent(self, tools):
        result = _execute_tool(
            "dwim_analyze_intent",
            {"query": "show me the code structure", "top_k": 3},
            tools,
        )

        assert isinstance(result, list)
        assert len(result) <= 3


class TestSerializeResult:
    """Tests for result serialization."""

    def test_serializes_none(self):
        result = _serialize_result(None)
        assert result == {"result": None}

    def test_serializes_string(self):
        result = _serialize_result("hello")
        assert result == {"result": "hello"}

    def test_serializes_list(self):
        result = _serialize_result([1, 2, 3])
        assert result == {"items": [1, 2, 3], "count": 3}

    def test_serializes_dict(self):
        result = _serialize_result({"key": "value"})
        assert result == {"key": "value"}

    def test_serializes_path(self):
        from pathlib import Path

        result = _serialize_result(Path("/foo/bar"))
        assert result == {"result": "/foo/bar"}

    def test_serializes_dataclass(self, tools, tmp_path: Path):
        """Test that dataclasses are serialized properly."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("def foo(): pass")

        result = _execute_tool("skeleton_extract", {"file_path": str(py_file)}, tools)
        serialized = _serialize_result(result)

        assert "items" in serialized
        assert len(serialized["items"]) == 1
        assert serialized["items"][0]["name"] == "foo"
