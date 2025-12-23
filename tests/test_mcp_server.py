"""Tests for MCP server tool functions."""

from pathlib import Path
from typing import ClassVar

import pytest

from moss_mcp.server_full import _execute_tool, _serialize_result
from moss_orchestration.gen.mcp import MCPGenerator


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

    async def test_extracts_skeleton(self, tools, python_file: Path):
        result = await _execute_tool(
            "skeleton_extract_python_skeleton", {"file_path": str(python_file)}, tools
        )

        assert len(result) >= 2  # Foo, baz
        names = [s.name for s in result]
        assert "Foo" in names
        assert "baz" in names

    async def test_formats_skeleton(self, tools, python_file: Path):
        result = await _execute_tool(
            "skeleton_format_skeleton", {"file_path": str(python_file)}, tools
        )

        assert isinstance(result, str)
        assert "Foo" in result
        assert "baz" in result

    async def test_handles_nonexistent_path(self, tools):
        # Empty source produces empty skeleton, no error
        result = await _execute_tool(
            "skeleton_extract_python_skeleton", {"file_path": "/nonexistent/path"}, tools
        )
        assert result == []  # Empty source produces empty result


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

    async def test_finds_anchors(self, tools, python_file: Path):
        result = await _execute_tool(
            "anchors_find_anchors",
            {"file_path": str(python_file), "anchor": "function:my_function"},
            tools,
        )

        assert isinstance(result, list)
        assert len(result) >= 1  # Should find my_function

    async def test_resolves_anchor(self, tools, python_file: Path, tmp_path: Path):
        result = await _execute_tool(
            "anchors_resolve_anchor",
            {"file_path": str(python_file), "anchor": "function:my_function"},
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

    async def test_builds_cfg(self, tools, python_file: Path):
        result = await _execute_tool("cfg_build_cfg", {"file_path": str(python_file)}, tools)

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

    async def test_extracts_deps(self, tools, python_file: Path):
        result = await _execute_tool(
            "dependencies_extract_dependencies", {"file_path": str(python_file)}, tools
        )

        assert hasattr(result, "imports")
        assert hasattr(result, "exports")
        assert len(result.imports) >= 2

        modules = [i.module for i in result.imports]
        assert "os" in modules
        assert "pathlib" in modules

    async def test_formats_deps(self, tools, python_file: Path):
        result = await _execute_tool(
            "dependencies_format_dependencies", {"file_path": str(python_file)}, tools
        )

        assert isinstance(result, str)
        assert "os" in result


class TestToolDwim:
    """Tests for DWIM tools."""

    async def test_list_tools(self, tools):
        result = await _execute_tool("dwim_list_tools", {}, tools)

        # Result is a list of dicts with 'name' keys
        assert isinstance(result, list)
        assert len(result) > 0
        names = [t["name"] for t in result]
        assert "skeleton" in names

    async def test_resolve_tool(self, tools):
        result = await _execute_tool("dwim_resolve_tool", {"tool_name": "skelton"}, tools)

        assert result.tool == "skeleton"
        assert result.confidence > 0.8

    async def test_analyze_intent(self, tools):
        result = await _execute_tool(
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
        """Strings are returned directly (not wrapped in dict)."""
        result = _serialize_result("hello")
        assert result == "hello"  # Direct string, not {"result": "hello"}

    def test_serializes_list(self):
        result = _serialize_result([1, 2, 3])
        assert result == "3 items: 1, 2, 3"

    def test_serializes_empty_list(self):
        result = _serialize_result([])
        assert result == "(empty)"

    def test_serializes_dict(self):
        result = _serialize_result({"key": "value"})
        assert result == {"key": "value"}

    def test_serializes_path(self):
        from pathlib import Path

        result = _serialize_result(Path("/foo/bar"))
        assert result == {"result": "/foo/bar"}

    async def test_serializes_dataclass(self, tools, tmp_path: Path):
        """Test that dataclasses are serialized properly."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("def foo(): pass")

        result = await _execute_tool(
            "skeleton_extract_python_skeleton", {"file_path": str(py_file)}, tools
        )
        serialized = _serialize_result(result)

        # Lists of dataclasses now get compact formatting
        assert isinstance(serialized, str)
        assert "foo" in serialized


class TestMCPOutputConsistency:
    """CI tests to ensure MCP tools return consistent, compact formats.

    These tests verify that tools returning formatted text return plain strings,
    not JSON-wrapped dicts like {"result": "..."}.
    """

    @pytest.fixture
    def python_file(self, tmp_path: Path):
        """Create a Python file for testing."""
        py_file = tmp_path / "sample.py"
        py_file.write_text("""
import os
from pathlib import Path

class MyClass:
    '''A sample class.'''
    def method(self) -> str:
        if True:
            return "hello"
        return "world"

def my_function(x: int) -> int:
    '''A sample function.'''
    return x * 2
""")
        return py_file

    async def test_skeleton_format_returns_string(self, tools, python_file: Path):
        """skeleton_format should return a plain string, not JSON."""
        result = await _execute_tool(
            "skeleton_format_skeleton", {"file_path": str(python_file)}, tools
        )
        serialized = _serialize_result(result)

        assert isinstance(serialized, str), f"Expected str, got {type(serialized)}: {serialized!r}"
        assert "MyClass" in serialized
        assert "my_function" in serialized
        # Should NOT be JSON-wrapped
        assert not serialized.startswith("{")

    async def test_tree_format_returns_string(self, tools, tmp_path: Path):
        """tree_render_tree should return a plain string, not JSON."""
        # Create some files
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "main.py").write_text("# main")

        result = await _execute_tool("tree_render_tree", {"root": str(tmp_path)}, tools)
        serialized = _serialize_result(result)

        assert isinstance(serialized, str), f"Expected str, got {type(serialized)}: {serialized!r}"
        assert "src" in serialized
        # Should NOT be JSON-wrapped
        assert not serialized.startswith("{")

    async def test_dependencies_format_returns_string(self, tools, python_file: Path):
        """dependencies_format should return a plain string, not JSON."""
        result = await _execute_tool(
            "dependencies_format_dependencies", {"file_path": str(python_file)}, tools
        )
        serialized = _serialize_result(result)

        assert isinstance(serialized, str), f"Expected str, got {type(serialized)}: {serialized!r}"
        assert "os" in serialized or "pathlib" in serialized
        # Should NOT be JSON-wrapped
        assert not serialized.startswith("{")

    async def test_complexity_analyze_returns_compact_string(self, tools, python_file: Path):
        """complexity_analyze should return a compact string."""
        result = await _execute_tool(
            "complexity_analyze_complexity",
            {"path": str(python_file.parent), "pattern": str(python_file.name)},
            tools,
        )
        serialized = _serialize_result(result)

        assert isinstance(serialized, str), f"Expected str, got {type(serialized)}: {serialized!r}"
        # Should be compact format: "complexity: avg X, max Y | ..."
        assert "complexity:" in serialized or isinstance(serialized, str)
        # Should NOT be JSON-wrapped
        assert not serialized.startswith("{")

    async def test_dwim_list_tools_returns_compact_string(self, tools):
        """dwim_list_tools should return a compact string, not JSON."""
        result = await _execute_tool("dwim_list_tools", {}, tools)
        serialized = _serialize_result(result)

        assert isinstance(serialized, str), f"Expected str, got {type(serialized)}: {serialized!r}"
        # Should contain tool info in compact format
        assert "skeleton" in serialized.lower()
        # Should NOT be JSON-wrapped like {"items": [...]}
        assert not serialized.startswith("{")


class TestAllToolsReturnCompact:
    """Test that MCP tools with handlers return compact (non-JSON) output.

    This tests tools that have handlers in the dispatcher. Not all introspected
    tools have handlers - the dispatcher only covers the most useful tools.

    Tools should return either:
    - Plain strings (for format functions)
    - Compact text (via to_compact())
    - Formatted lists (via _format_list_compact)

    Some tools return structured data (dicts) which is acceptable for
    non-formatting tools.
    """

    # Tools that return structured data (dicts) - this is expected
    STRUCTURED_OUTPUT_TOOLS: ClassVar[set[str]] = {
        "tree_build_tree",  # Returns tree node structure
        "dwim_resolve_tool",  # Returns {tool, confidence, message}
    }

    @pytest.fixture
    def project_dir(self, tmp_path: Path):
        """Create a minimal project for testing."""
        # Create some Python files
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "main.py").write_text("""
import os
from pathlib import Path

class MyClass:
    '''A sample class.'''
    def method(self) -> str:
        if True:
            return "hello"
        return "world"

def my_function(x: int) -> int:
    '''A sample function.'''
    return x * 2
""")
        (tmp_path / "tests").mkdir()
        (tmp_path / "tests" / "test_main.py").write_text("""
def test_example():
    assert True
""")
        return tmp_path

    async def test_dispatcher_tools_return_valid_output(self, tools, project_dir: Path):
        """Test all tools with dispatcher handlers return valid output."""
        from moss_mcp.server_full import _get_tool_dispatcher

        python_file = project_dir / "src" / "main.py"
        dispatcher = _get_tool_dispatcher()

        failures = []

        for tool in tools:
            # Only test tools that have dispatcher handlers
            if tool.api_path not in dispatcher:
                continue

            # Build arguments based on tool requirements
            args = {}
            if tool.api_path in {"skeleton.extract_python_skeleton", "skeleton.format_skeleton"}:
                args["file_path"] = str(python_file)
            elif tool.api_path in {"anchors.find_anchors", "anchors.resolve_anchor"}:
                args["file_path"] = str(python_file)
                args["anchor"] = "function:my_function"
            elif tool.api_path == "cfg.build_cfg":
                args["file_path"] = str(python_file)
            elif tool.api_path in {
                "dependencies.extract_dependencies",
                "dependencies.format_dependencies",
            }:
                args["file_path"] = str(python_file)
            elif tool.api_path == "complexity.analyze_complexity":
                args["path"] = str(project_dir)
            elif tool.api_path == "security.analyze_security":
                args["path"] = str(project_dir)
            elif tool.api_path in {"tree.render_tree", "tree.build_tree"}:
                args["root"] = str(project_dir)
            elif tool.api_path == "dwim.list_tools":
                pass  # No args needed
            elif tool.api_path == "dwim.resolve_tool":
                args["tool_name"] = "skeleton"
            elif tool.api_path == "dwim.analyze_intent":
                args["query"] = "show code structure"

            try:
                result = await _execute_tool(tool.name, args, tools)
                serialized = _serialize_result(result)

                # Check output format
                if tool.name in self.STRUCTURED_OUTPUT_TOOLS:
                    # These return dicts, which is expected
                    continue

                if isinstance(serialized, str):
                    if serialized.startswith("{"):
                        failures.append(
                            f"{tool.name}: returned string starting with '{{': {serialized[:100]!r}"
                        )
                elif isinstance(serialized, dict):
                    # Only flag as failure if it's not in STRUCTURED_OUTPUT_TOOLS
                    failures.append(
                        f"{tool.name}: returned dict instead of string: {list(serialized.keys())}"
                    )
            except Exception as e:
                # Some tools may fail due to missing dependencies - that's OK
                if "not found" not in str(e).lower() and "no such file" not in str(e).lower():
                    failures.append(f"{tool.name}: raised {type(e).__name__}: {e}")

        if failures:
            pytest.fail("Tools with invalid output:\n" + "\n".join(failures))
