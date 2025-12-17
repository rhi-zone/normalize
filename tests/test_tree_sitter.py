"""Tests for Tree-sitter integration."""

from pathlib import Path

import pytest

from moss.tree_sitter import (
    LanguageType,
    TreeNode,
    TreeSitterParser,
    TreeSitterSkeletonProvider,
    TSSymbol,
    get_language_for_extension,
    is_supported_extension,
)


class TestTreeNode:
    """Tests for TreeNode dataclass."""

    def test_create_node(self):
        node = TreeNode(
            type="function_declaration",
            text="def foo(): pass",
            start_line=0,
            end_line=0,
            start_column=0,
            end_column=15,
        )

        assert node.type == "function_declaration"
        assert node.text == "def foo(): pass"
        assert node.named is True

    def test_range_property(self):
        node = TreeNode(
            type="test",
            text="x",
            start_line=5,
            end_line=10,
            start_column=2,
            end_column=8,
        )

        assert node.range == ((5, 2), (10, 8))

    def test_children_default(self):
        node = TreeNode(
            type="test",
            text="x",
            start_line=0,
            end_line=0,
            start_column=0,
            end_column=1,
        )

        assert node.children == []


class TestTSSymbol:
    """Tests for TSSymbol dataclass."""

    def test_create_symbol(self):
        symbol = TSSymbol(
            name="MyClass",
            kind="class",
            line=10,
            end_line=50,
            signature="class MyClass:",
            docstring="A class",
            visibility="public",
            parent=None,
        )

        assert symbol.name == "MyClass"
        assert symbol.kind == "class"
        assert symbol.line == 10
        assert symbol.end_line == 50

    def test_default_values(self):
        symbol = TSSymbol(name="foo", kind="function", line=1, end_line=5)

        assert symbol.signature is None
        assert symbol.docstring is None
        assert symbol.parent is None
        assert symbol.children == []


class TestLanguageType:
    """Tests for LanguageType enum."""

    def test_language_types(self):
        assert LanguageType.PYTHON.name == "PYTHON"
        assert LanguageType.TYPESCRIPT.name == "TYPESCRIPT"
        assert LanguageType.GO.name == "GO"
        assert LanguageType.RUST.name == "RUST"


class TestTreeSitterParser:
    """Tests for TreeSitterParser."""

    def test_extension_map(self):
        assert TreeSitterParser.EXTENSION_MAP[".py"] == LanguageType.PYTHON
        assert TreeSitterParser.EXTENSION_MAP[".ts"] == LanguageType.TYPESCRIPT
        assert TreeSitterParser.EXTENSION_MAP[".tsx"] == LanguageType.TYPESCRIPT
        assert TreeSitterParser.EXTENSION_MAP[".go"] == LanguageType.GO
        assert TreeSitterParser.EXTENSION_MAP[".rs"] == LanguageType.RUST

    def test_from_file_python(self, tmp_path: Path):
        test_file = tmp_path / "test.py"
        test_file.write_text("x = 1")

        parser = TreeSitterParser.from_file(test_file)
        assert parser._language_type == LanguageType.PYTHON

    def test_from_file_typescript(self, tmp_path: Path):
        test_file = tmp_path / "test.ts"
        test_file.write_text("const x = 1;")

        parser = TreeSitterParser.from_file(test_file)
        assert parser._language_type == LanguageType.TYPESCRIPT

    def test_from_file_unknown_extension(self, tmp_path: Path):
        test_file = tmp_path / "test.xyz"
        test_file.write_text("unknown")

        with pytest.raises(ValueError, match="Unsupported file extension"):
            TreeSitterParser.from_file(test_file)

    def test_init_with_string(self):
        parser = TreeSitterParser("python")
        assert parser._language_type == LanguageType.PYTHON

    def test_init_with_enum(self):
        parser = TreeSitterParser(LanguageType.TYPESCRIPT)
        assert parser._language_type == LanguageType.TYPESCRIPT

    def test_lazy_initialization(self):
        parser = TreeSitterParser("python")
        assert parser._parser is None
        assert parser._language is None

    def test_parse_python(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_python")

        parser = TreeSitterParser("python")
        source = """
def hello():
    print("Hello")

class MyClass:
    def method(self):
        pass
"""
        tree = parser.parse(source)

        assert tree.type == "module"
        assert len(tree.children) > 0

    def test_extract_python_symbols(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_python")

        parser = TreeSitterParser("python")
        source = """
def hello():
    print("Hello")

class MyClass:
    def method(self):
        pass
"""
        tree = parser.parse(source)
        symbols = parser.extract_symbols(tree)

        names = [s.name for s in symbols]
        assert "hello" in names
        assert "MyClass" in names
        assert "method" in names

    def test_parse_typescript(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_typescript")

        parser = TreeSitterParser("typescript")
        source = """
function greet(name: string): void {
    console.log(name);
}

class Person {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    sayHello(): void {
        console.log(this.name);
    }
}

interface Greeting {
    message: string;
}

type ID = string | number;
"""
        tree = parser.parse(source)
        assert tree.type == "program"

    def test_extract_typescript_symbols(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_typescript")

        parser = TreeSitterParser("typescript")
        source = """
function greet(name: string): void {
    console.log(name);
}

class Person {
    sayHello(): void {
        console.log("hello");
    }
}

interface Greeting {
    message: string;
}
"""
        tree = parser.parse(source)
        symbols = parser.extract_symbols(tree)

        kinds = {s.kind for s in symbols}
        assert "function" in kinds
        assert "class" in kinds


class TestTreeSitterSkeletonProvider:
    """Tests for TreeSitterSkeletonProvider."""

    def test_extract_python_skeleton(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_python")

        provider = TreeSitterSkeletonProvider("python")
        source = """
def hello():
    pass

class MyClass:
    def method(self):
        pass
"""
        symbols = provider.extract_skeleton(source)

        assert len(symbols) >= 2
        names = [s.name for s in symbols]
        assert "hello" in names
        assert "MyClass" in names

    def test_format_skeleton(self):
        pytest.importorskip("tree_sitter")
        pytest.importorskip("tree_sitter_python")

        provider = TreeSitterSkeletonProvider("python")
        symbols = [
            TSSymbol(name="hello", kind="function", line=1, end_line=2, signature="def hello()"),
            TSSymbol(name="MyClass", kind="class", line=4, end_line=10),
        ]

        formatted = provider.format_skeleton(symbols)

        assert "function: def hello()" in formatted
        assert "class: MyClass" in formatted

    def test_from_file(self, tmp_path: Path):
        test_file = tmp_path / "test.py"
        test_file.write_text("x = 1")

        provider = TreeSitterSkeletonProvider.from_file(test_file)
        assert provider._parser._language_type == LanguageType.PYTHON


class TestHelperFunctions:
    """Tests for helper functions."""

    def test_get_language_for_extension(self):
        assert get_language_for_extension(".py") == LanguageType.PYTHON
        assert get_language_for_extension(".ts") == LanguageType.TYPESCRIPT
        assert get_language_for_extension(".PY") == LanguageType.PYTHON  # Case insensitive
        assert get_language_for_extension(".unknown") is None

    def test_is_supported_extension(self):
        assert is_supported_extension(".py") is True
        assert is_supported_extension(".ts") is True
        assert is_supported_extension(".go") is True
        assert is_supported_extension(".rs") is True
        assert is_supported_extension(".unknown") is False
