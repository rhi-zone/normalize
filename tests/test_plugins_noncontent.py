"""Tests for non-code content plugins (Markdown, JSON, YAML, TOML)."""

from pathlib import Path

import pytest

from moss.views import ViewTarget

# =============================================================================
# Markdown Plugin Tests
# =============================================================================


class TestMarkdownStructureExtraction:
    """Tests for Markdown structure extraction functions."""

    def test_extract_headings(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = """# Title
## Section 1
### Subsection
## Section 2
"""
        structure = extract_markdown_structure(source)

        assert len(structure.headings) == 4
        assert structure.headings[0].level == 1
        assert structure.headings[0].text == "Title"
        assert structure.headings[1].level == 2
        assert structure.headings[1].text == "Section 1"
        assert structure.headings[2].level == 3

    def test_extract_code_blocks(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = """# Code Example

```python
def hello():
    print("world")
```

```javascript
console.log("test");
```
"""
        structure = extract_markdown_structure(source)

        assert len(structure.code_blocks) == 2
        assert structure.code_blocks[0].language == "python"
        assert "def hello" in structure.code_blocks[0].content
        assert structure.code_blocks[1].language == "javascript"

    def test_extract_code_block_no_language(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = """```
plain text
```
"""
        structure = extract_markdown_structure(source)

        assert len(structure.code_blocks) == 1
        assert structure.code_blocks[0].language is None

    def test_extract_links(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = """Check [the docs](https://example.com/docs) for info.
Also see [local file](./README.md) and [anchor](#section).
"""
        structure = extract_markdown_structure(source)

        assert len(structure.links) == 3

        external = [lnk for lnk in structure.links if not lnk.is_internal]
        internal = [lnk for lnk in structure.links if lnk.is_internal]

        assert len(external) == 1
        assert external[0].url == "https://example.com/docs"

        assert len(internal) == 2

    def test_extract_front_matter(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = """---
title: Test Doc
author: Test Author
---

# Content
"""
        structure = extract_markdown_structure(source)

        # Front matter parsing depends on PyYAML availability
        assert structure.front_matter is not None
        if "_raw" not in structure.front_matter:
            assert structure.front_matter.get("title") == "Test Doc"

    def test_no_front_matter(self):
        from moss.plugins.markdown import extract_markdown_structure

        source = "# Just a heading\n\nSome content."
        structure = extract_markdown_structure(source)

        assert structure.front_matter is None

    def test_format_structure(self):
        from moss.plugins.markdown import extract_markdown_structure, format_markdown_structure

        source = """# Title

## Section

Some text with a [link](https://example.com).

```python
code
```
"""
        structure = extract_markdown_structure(source)
        formatted = format_markdown_structure(structure)

        assert "Structure:" in formatted
        assert "Title" in formatted
        assert "Section" in formatted
        assert "Code Blocks:" in formatted
        assert "python" in formatted
        assert "External Links:" in formatted


class TestMarkdownPlugin:
    """Tests for MarkdownStructurePlugin."""

    @pytest.fixture
    def plugin(self):
        from moss.plugins.markdown import MarkdownStructurePlugin

        return MarkdownStructurePlugin()

    @pytest.fixture
    def md_file(self, tmp_path: Path) -> Path:
        f = tmp_path / "test.md"
        f.write_text("""# Test Document

## Introduction

This is a test.

## Code

```python
print("hello")
```
""")
        return f

    def test_metadata(self, plugin):
        meta = plugin.metadata
        assert meta.name == "markdown-structure"
        assert meta.view_type == "skeleton"
        assert "markdown" in meta.languages

    def test_supports_markdown(self, plugin, md_file: Path):
        target = ViewTarget(path=md_file)
        assert plugin.supports(target) is True

    def test_not_supports_python(self, plugin, tmp_path: Path):
        py_file = tmp_path / "test.py"
        py_file.write_text("print('hello')")
        target = ViewTarget(path=py_file)
        assert plugin.supports(target) is False

    async def test_render(self, plugin, md_file: Path):
        target = ViewTarget(path=md_file)
        view = await plugin.render(target)

        assert view.metadata.get("heading_count") == 3
        assert view.metadata.get("code_block_count") == 1
        assert "Test Document" in view.content


# =============================================================================
# JSON Plugin Tests
# =============================================================================


class TestJSONSchemaPlugin:
    """Tests for JSONSchemaPlugin."""

    @pytest.fixture
    def plugin(self):
        from moss.plugins.data_files import JSONSchemaPlugin

        return JSONSchemaPlugin()

    @pytest.fixture
    def json_file(self, tmp_path: Path) -> Path:
        f = tmp_path / "test.json"
        f.write_text("""{
    "name": "test",
    "version": "1.0.0",
    "dependencies": {
        "foo": "^1.0"
    },
    "keywords": ["test", "example"]
}""")
        return f

    def test_metadata(self, plugin):
        meta = plugin.metadata
        assert meta.name == "json-schema"
        assert meta.view_type == "skeleton"
        assert "json" in meta.languages

    def test_supports_json(self, plugin, json_file: Path):
        target = ViewTarget(path=json_file)
        assert plugin.supports(target) is True

    async def test_render(self, plugin, json_file: Path):
        target = ViewTarget(path=json_file)
        view = await plugin.render(target)

        assert view.metadata.get("root_type") == "object"
        assert "name: string" in view.content
        assert "version: string" in view.content
        assert "dependencies: object" in view.content
        assert "keywords: array" in view.content

    async def test_render_invalid_json(self, plugin, tmp_path: Path):
        bad_file = tmp_path / "bad.json"
        bad_file.write_text("{invalid json")
        target = ViewTarget(path=bad_file)
        view = await plugin.render(target)

        assert "error" in view.metadata


class TestSchemaInference:
    """Tests for schema inference."""

    def test_infer_string(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema("hello")
        assert schema.value_type == "string"

    def test_infer_number(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema(42)
        assert schema.value_type == "number"

        schema = infer_schema(3.14)
        assert schema.value_type == "number"

    def test_infer_boolean(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema(True)
        assert schema.value_type == "boolean"

    def test_infer_null(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema(None)
        assert schema.value_type == "null"

    def test_infer_array(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema([1, 2, 3])
        assert schema.value_type == "array"
        assert schema.array_item_type == "number"

    def test_infer_object(self):
        from moss.plugins.data_files import infer_schema

        schema = infer_schema({"a": 1, "b": "two"})
        assert schema.value_type == "object"
        assert len(schema.children) == 2

    def test_infer_nested(self):
        from moss.plugins.data_files import infer_schema

        data = {
            "config": {
                "debug": True,
                "options": ["a", "b"],
            }
        }
        schema = infer_schema(data)

        assert schema.value_type == "object"
        config_node = schema.children[0]
        assert config_node.name == "config"
        assert config_node.value_type == "object"


# =============================================================================
# YAML Plugin Tests
# =============================================================================


class TestYAMLSchemaPlugin:
    """Tests for YAMLSchemaPlugin."""

    @pytest.fixture
    def plugin(self):
        from moss.plugins.data_files import YAMLSchemaPlugin

        return YAMLSchemaPlugin()

    @pytest.fixture
    def yaml_file(self, tmp_path: Path) -> Path:
        f = tmp_path / "test.yaml"
        f.write_text("""
name: test
version: 1.0.0
config:
  debug: true
  level: 3
""")
        return f

    def test_metadata(self, plugin):
        meta = plugin.metadata
        assert meta.name == "yaml-schema"
        assert meta.view_type == "skeleton"
        assert "yaml" in meta.languages

    def test_supports_yaml(self, plugin, yaml_file: Path):
        target = ViewTarget(path=yaml_file)
        assert plugin.supports(target) is True

    def test_supports_yml(self, plugin, tmp_path: Path):
        yml_file = tmp_path / "test.yml"
        yml_file.write_text("key: value")
        target = ViewTarget(path=yml_file)
        assert plugin.supports(target) is True

    async def test_render(self, plugin, yaml_file: Path):
        pytest.importorskip("yaml")

        target = ViewTarget(path=yaml_file)
        view = await plugin.render(target)

        assert view.metadata.get("root_type") == "object"
        assert "name: string" in view.content
        assert "config: object" in view.content


# =============================================================================
# TOML Plugin Tests
# =============================================================================


class TestTOMLSchemaPlugin:
    """Tests for TOMLSchemaPlugin."""

    @pytest.fixture
    def plugin(self):
        from moss.plugins.data_files import TOMLSchemaPlugin

        return TOMLSchemaPlugin()

    @pytest.fixture
    def toml_file(self, tmp_path: Path) -> Path:
        f = tmp_path / "test.toml"
        f.write_text("""
[project]
name = "test"
version = "1.0.0"

[project.dependencies]
foo = "^1.0"
""")
        return f

    def test_metadata(self, plugin):
        meta = plugin.metadata
        assert meta.name == "toml-schema"
        assert meta.view_type == "skeleton"
        assert "toml" in meta.languages

    def test_supports_toml(self, plugin, toml_file: Path):
        target = ViewTarget(path=toml_file)
        assert plugin.supports(target) is True

    async def test_render(self, plugin, toml_file: Path):
        # TOML parsing requires Python 3.11+ or tomli
        target = ViewTarget(path=toml_file)
        view = await plugin.render(target)

        # Either we get a schema or an error about missing parser
        if "error" not in view.metadata:
            assert view.metadata.get("root_type") == "object"
            assert "project: object" in view.content


# =============================================================================
# Integration Tests
# =============================================================================


class TestPluginRegistration:
    """Test that non-code plugins are properly registered."""

    def test_plugins_registered(self):
        from moss.plugins import get_registry, reset_registry

        reset_registry()
        registry = get_registry()

        # Check that all non-code plugins are registered
        assert registry.get_plugin("markdown-structure") is not None
        assert registry.get_plugin("json-schema") is not None
        assert registry.get_plugin("yaml-schema") is not None
        assert registry.get_plugin("toml-schema") is not None

        reset_registry()

    def test_find_plugin_for_markdown(self, tmp_path: Path):
        from moss.plugins import get_registry, reset_registry

        reset_registry()
        registry = get_registry()

        md_file = tmp_path / "README.md"
        md_file.write_text("# Hello")

        target = ViewTarget(path=md_file)
        plugin = registry.find_plugin(target, "skeleton")

        assert plugin is not None
        assert plugin.metadata.name == "markdown-structure"

        reset_registry()

    def test_find_plugin_for_json(self, tmp_path: Path):
        from moss.plugins import get_registry, reset_registry

        reset_registry()
        registry = get_registry()

        json_file = tmp_path / "data.json"
        json_file.write_text('{"key": "value"}')

        target = ViewTarget(path=json_file)
        plugin = registry.find_plugin(target, "skeleton")

        assert plugin is not None
        assert plugin.metadata.name == "json-schema"

        reset_registry()
