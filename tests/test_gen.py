"""Tests for the interface generator module."""

from pathlib import Path

import pytest

from moss.gen import (
    CLIGenerator,
    HTTPGenerator,
    MCPGenerator,
    generate_cli,
    generate_http,
    generate_mcp,
    generate_mcp_definitions,
    generate_openapi,
    introspect_api,
)
from moss.gen.http import (
    HTTPRouter,
    method_to_endpoint,
    subapi_to_router,
)
from moss.gen.introspect import (
    _get_type_string,
    _parse_docstring,
    introspect_method,
    introspect_subapi,
)

# =============================================================================
# Docstring Parsing Tests
# =============================================================================


class TestParseDocstring:
    def test_parse_simple_docstring(self):
        docstring = "A simple description."
        desc, params = _parse_docstring(docstring)
        assert desc == "A simple description."
        assert params == {}

    def test_parse_docstring_with_args(self):
        docstring = """Do something useful.

        Args:
            name: The name to use
            count: How many times
        """
        desc, params = _parse_docstring(docstring)
        assert desc == "Do something useful."
        assert params["name"] == "The name to use"
        assert params["count"] == "How many times"

    def test_parse_docstring_with_returns(self):
        docstring = """Get a value.

        Args:
            key: The key to look up

        Returns:
            The value for the key
        """
        desc, params = _parse_docstring(docstring)
        # Description includes everything before Args section
        assert "Get a value" in desc
        assert params["key"] == "The key to look up"

    def test_parse_empty_docstring(self):
        desc, params = _parse_docstring(None)
        assert desc == ""
        assert params == {}


# =============================================================================
# Type String Tests
# =============================================================================


class TestGetTypeString:
    def test_simple_types(self):
        assert _get_type_string(str) == "str"
        assert _get_type_string(int) == "int"
        assert _get_type_string(bool) == "bool"
        assert _get_type_string(Path) == "Path"

    def test_string_annotations(self):
        assert _get_type_string("str") == "str"
        assert _get_type_string("Path") == "Path"

    def test_none(self):
        assert _get_type_string(None) == "None"


# =============================================================================
# Method Introspection Tests
# =============================================================================


class TestIntrospectMethod:
    def test_simple_method(self):
        def simple(self, name: str) -> str:
            """Get a name."""
            return name

        method = introspect_method(simple, {"name": str, "return": str})
        assert method.name == "simple"
        assert method.description == "Get a name."
        assert len(method.parameters) == 1
        assert method.parameters[0].name == "name"
        assert method.parameters[0].required is True
        assert method.return_type == "str"
        assert method.is_async is False

    def test_method_with_optional(self):
        def with_optional(self, name: str, count: int = 10) -> str:
            """Do something."""
            return name * count

        method = introspect_method(with_optional, {"name": str, "count": int, "return": str})
        assert len(method.parameters) == 2
        assert method.parameters[0].required is True
        assert method.parameters[1].required is False
        assert method.parameters[1].default == 10

    def test_async_method(self):
        async def async_method(self) -> None:
            """An async method."""
            pass

        method = introspect_method(async_method, {"return": None})
        assert method.is_async is True


# =============================================================================
# SubAPI Introspection Tests
# =============================================================================


class TestIntrospectSubapi:
    def test_introspect_subapi(self):
        from moss.moss_api import SkeletonAPI

        subapi = introspect_subapi(SkeletonAPI, "skeleton")
        assert subapi.name == "skeleton"
        assert subapi.class_name == "SkeletonAPI"
        assert len(subapi.methods) > 0
        # Should have extract and format methods
        method_names = [m.name for m in subapi.methods]
        assert "extract" in method_names
        assert "format" in method_names


# =============================================================================
# Full API Introspection Tests
# =============================================================================


class TestIntrospectAPI:
    def test_introspect_all(self):
        sub_apis = introspect_api()
        assert len(sub_apis) > 0

        names = [api.name for api in sub_apis]
        assert "skeleton" in names
        assert "anchor" in names
        assert "health" in names

    def test_skeleton_api_methods(self):
        sub_apis = introspect_api()
        skeleton = next((api for api in sub_apis if api.name == "skeleton"), None)
        assert skeleton is not None

        method_names = [m.name for m in skeleton.methods]
        assert "extract" in method_names
        assert "format" in method_names

    def test_health_api_methods(self):
        sub_apis = introspect_api()
        health = next((api for api in sub_apis if api.name == "health"), None)
        assert health is not None

        method_names = [m.name for m in health.methods]
        assert "check" in method_names
        assert "summarize" in method_names


# =============================================================================
# CLI Generator Tests
# =============================================================================


class TestCLIGenerator:
    @pytest.fixture
    def generator(self):
        return CLIGenerator()

    def test_generate_groups(self, generator: CLIGenerator):
        groups = generator.generate_groups()
        assert len(groups) > 0

        group_names = [g.name for g in groups]
        assert "skeleton" in group_names
        assert "health" in group_names

    def test_skeleton_group_commands(self, generator: CLIGenerator):
        groups = generator.generate_groups()
        skeleton = next((g for g in groups if g.name == "skeleton"), None)
        assert skeleton is not None

        command_names = [c.name for c in skeleton.commands]
        assert "extract" in command_names
        assert "format" in command_names

    def test_generate_parser(self, generator: CLIGenerator):
        parser = generator.generate_parser()
        assert parser is not None
        assert parser.prog == "moss"

    def test_parser_has_root_option(self, generator: CLIGenerator):
        parser = generator.generate_parser()
        # Parse with --help would show root option
        # Test by parsing valid args
        args = parser.parse_args(["--root", "/tmp/test"])
        assert args.root == "/tmp/test"

    def test_parser_has_json_option(self, generator: CLIGenerator):
        parser = generator.generate_parser()
        args = parser.parse_args(["--json"])
        assert args.json is True


class TestGenerateCLI:
    def test_convenience_function(self):
        parser = generate_cli()
        assert parser is not None
        assert parser.prog == "moss"

    def test_custom_prog(self):
        parser = generate_cli(prog="my-cli")
        assert parser.prog == "my-cli"


# =============================================================================
# CLI Executor Tests
# =============================================================================


class TestCLIExecutor:
    @pytest.fixture
    def executor(self):
        generator = CLIGenerator()
        return generator.generate_executor()

    def test_execute_health_check(self, executor, tmp_path: Path):
        # Create a minimal project structure
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "__init__.py").touch()

        import argparse

        args = argparse.Namespace(
            root=str(tmp_path),
            command="health",
            subcommand="check",
            json=False,
        )

        # This should run without error
        result = executor.execute(args)
        # Health check returns ProjectStatus
        assert result is not None
        assert hasattr(result, "health_grade")

    def test_execute_unknown_command(self, executor, tmp_path: Path):
        import argparse

        args = argparse.Namespace(
            root=str(tmp_path),
            command="unknown",
            subcommand="test",
            json=False,
        )

        with pytest.raises(ValueError, match="Unknown command"):
            executor.execute(args)

    def test_execute_no_command(self, executor, tmp_path: Path):
        import argparse

        args = argparse.Namespace(
            root=str(tmp_path),
            command=None,
            subcommand=None,
            json=False,
        )

        result = executor.execute(args)
        assert result is None


# =============================================================================
# Integration Tests
# =============================================================================


class TestCLIIntegration:
    def test_parse_skeleton_extract(self):
        parser = generate_cli()
        args = parser.parse_args(["skeleton", "extract", "test.py"])
        assert args.command == "skeleton"
        assert args.subcommand == "extract"

    def test_parse_health_check(self):
        parser = generate_cli()
        args = parser.parse_args(["health", "check"])
        assert args.command == "health"
        assert args.subcommand == "check"

    def test_parse_with_root(self):
        parser = generate_cli()
        args = parser.parse_args(["--root", "/tmp", "health", "check"])
        assert args.root == "/tmp"
        assert args.command == "health"


# =============================================================================
# HTTP Generator Tests
# =============================================================================


class TestHTTPGenerator:
    @pytest.fixture
    def generator(self):
        return HTTPGenerator()

    def test_generate_routers(self, generator: HTTPGenerator):
        routers = generator.generate_routers()
        assert len(routers) > 0

        prefixes = [r.prefix for r in routers]
        assert "/skeleton" in prefixes
        assert "/health" in prefixes

    def test_skeleton_router_endpoints(self, generator: HTTPGenerator):
        routers = generator.generate_routers()
        skeleton = next((r for r in routers if r.prefix == "/skeleton"), None)
        assert skeleton is not None

        paths = [e.path for e in skeleton.endpoints]
        assert "/skeleton/extract" in paths
        assert "/skeleton/format" in paths

    def test_health_router_endpoints(self, generator: HTTPGenerator):
        routers = generator.generate_routers()
        health = next((r for r in routers if r.prefix == "/health"), None)
        assert health is not None

        paths = [e.path for e in health.endpoints]
        assert "/health/check" in paths
        assert "/health/summarize" in paths


class TestMethodToEndpoint:
    def test_get_method_for_read_operations(self):
        from moss.gen.introspect import APIMethod

        method = APIMethod(name="check", description="Check something")
        endpoint = method_to_endpoint(method, "/health")
        assert endpoint.method == "GET"

    def test_post_method_for_write_operations(self):
        from moss.gen.introspect import APIMethod

        method = APIMethod(name="apply", description="Apply something")
        endpoint = method_to_endpoint(method, "/patch")
        assert endpoint.method == "POST"


class TestSubapiToRouter:
    def test_creates_router(self):
        from moss.gen.introspect import APIMethod, SubAPI

        subapi = SubAPI(
            name="test",
            class_name="TestAPI",
            description="Test API",
            methods=[
                APIMethod(name="get_item", description="Get item"),
                APIMethod(name="create_item", description="Create item"),
            ],
        )

        router = subapi_to_router(subapi)
        assert router.prefix == "/test"
        assert router.tag == "test"
        assert len(router.endpoints) == 2


class TestGenerateOpenAPI:
    def test_generates_spec(self):
        spec = generate_openapi()
        assert spec["openapi"] == "3.0.3"
        assert spec["info"]["title"] == "Moss API"
        assert "paths" in spec
        assert "tags" in spec

    def test_spec_has_paths(self):
        spec = generate_openapi()
        paths = spec["paths"]
        # Should have skeleton and health paths
        assert any("/skeleton" in path for path in paths)
        assert any("/health" in path for path in paths)

    def test_spec_has_tags(self):
        spec = generate_openapi()
        tags = spec["tags"]
        tag_names = [t["name"] for t in tags]
        assert "skeleton" in tag_names
        assert "health" in tag_names


class TestGenerateHTTP:
    def test_convenience_function(self):
        routers = generate_http()
        assert len(routers) > 0
        assert all(isinstance(r, HTTPRouter) for r in routers)


# =============================================================================
# MCP Generator Tests
# =============================================================================


class TestMCPGenerator:
    @pytest.fixture
    def generator(self):
        return MCPGenerator()

    def test_generate_tools(self, generator: MCPGenerator):
        tools = generator.generate_tools()
        assert len(tools) > 0

        # Should have tools for skeleton and health APIs
        tool_names = [t.name for t in tools]
        assert any("skeleton" in name for name in tool_names)
        assert any("health" in name for name in tool_names)

    def test_skeleton_tools(self, generator: MCPGenerator):
        tools = generator.generate_tools()
        skeleton_tools = [t for t in tools if t.name.startswith("skeleton_")]
        assert len(skeleton_tools) > 0

        # Should have extract tool
        extract_tool = next((t for t in skeleton_tools if "extract" in t.name), None)
        assert extract_tool is not None
        assert extract_tool.api_path == "skeleton.extract"

    def test_tool_has_input_schema(self, generator: MCPGenerator):
        tools = generator.generate_tools()
        for tool in tools:
            assert "type" in tool.input_schema
            assert tool.input_schema["type"] == "object"

    def test_generate_tool_definitions(self, generator: MCPGenerator):
        defs = generator.generate_tool_definitions()
        assert len(defs) > 0

        for defn in defs:
            assert "name" in defn
            assert "description" in defn
            assert "inputSchema" in defn


class TestMCPToolExecutor:
    @pytest.fixture
    def executor(self):
        from moss.gen.mcp import MCPToolExecutor

        return MCPToolExecutor()

    def test_list_tools(self, executor):
        tools = executor.list_tools()
        assert len(tools) > 0
        assert any("health" in t for t in tools)

    def test_execute_health_check(self, executor, tmp_path: Path):
        # Create a minimal project structure
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "__init__.py").touch()

        result = executor.execute("health_check", {"root": str(tmp_path)})
        assert result is not None
        assert hasattr(result, "health_grade")

    def test_execute_unknown_tool(self, executor):
        with pytest.raises(ValueError, match="Unknown tool"):
            executor.execute("unknown_tool", {})


class TestGenerateMCP:
    def test_convenience_function(self):
        tools = generate_mcp()
        assert len(tools) > 0

    def test_definitions_function(self):
        defs = generate_mcp_definitions()
        assert len(defs) > 0
        assert all(isinstance(d, dict) for d in defs)
