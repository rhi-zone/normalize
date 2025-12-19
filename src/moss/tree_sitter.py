"""Tree-sitter integration for multi-language AST parsing.

This module provides:
- TreeSitterParser: Generic parser for any tree-sitter supported language
- Language-specific skeleton extractors (TypeScript, JavaScript, Go, Rust)
- Query-based code navigation

Requires: pip install tree-sitter tree-sitter-python tree-sitter-typescript etc.

Usage:
    parser = TreeSitterParser("typescript")
    tree = parser.parse(source_code)

    # Extract symbols
    symbols = parser.extract_symbols(tree)

    # Find specific nodes
    functions = parser.query(tree, "(function_declaration) @fn")
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto
from pathlib import Path
from typing import TYPE_CHECKING, Any, ClassVar

if TYPE_CHECKING:
    pass


class LanguageType(Enum):
    """Supported programming languages."""

    PYTHON = auto()
    TYPESCRIPT = auto()
    JAVASCRIPT = auto()
    GO = auto()
    RUST = auto()
    JAVA = auto()
    C = auto()
    CPP = auto()


@dataclass
class TreeNode:
    """Represents a node in the syntax tree."""

    type: str
    text: str
    start_line: int
    end_line: int
    start_column: int
    end_column: int
    children: list[TreeNode] = field(default_factory=list)
    named: bool = True

    @property
    def range(self) -> tuple[tuple[int, int], tuple[int, int]]:
        """Get the (start, end) positions as ((line, col), (line, col))."""
        return ((self.start_line, self.start_column), (self.end_line, self.end_column))


@dataclass
class TSSymbol:
    """Symbol extracted from tree-sitter parse."""

    name: str
    kind: str  # function, class, method, variable, etc.
    line: int
    end_line: int
    signature: str | None = None
    docstring: str | None = None
    visibility: str | None = None  # public, private, protected
    parent: str | None = None
    children: list[TSSymbol] = field(default_factory=list)


@dataclass
class QueryMatch:
    """Result of a tree-sitter query."""

    pattern_index: int
    captures: dict[str, TreeNode]


class TreeSitterParser:
    """Parser using tree-sitter for multi-language AST.

    Supports parsing source code and extracting structural information
    for many programming languages.
    """

    # Map of language to tree-sitter module name
    LANGUAGE_MODULES: ClassVar[dict[LanguageType, str]] = {
        LanguageType.PYTHON: "tree_sitter_python",
        LanguageType.TYPESCRIPT: "tree_sitter_typescript",
        LanguageType.JAVASCRIPT: "tree_sitter_javascript",
        LanguageType.GO: "tree_sitter_go",
        LanguageType.RUST: "tree_sitter_rust",
        LanguageType.JAVA: "tree_sitter_java",
        LanguageType.C: "tree_sitter_c",
        LanguageType.CPP: "tree_sitter_cpp",
    }

    # File extension to language mapping
    EXTENSION_MAP: ClassVar[dict[str, LanguageType]] = {
        ".py": LanguageType.PYTHON,
        ".ts": LanguageType.TYPESCRIPT,
        ".tsx": LanguageType.TYPESCRIPT,
        ".js": LanguageType.JAVASCRIPT,
        ".jsx": LanguageType.JAVASCRIPT,
        ".go": LanguageType.GO,
        ".rs": LanguageType.RUST,
        ".java": LanguageType.JAVA,
        ".c": LanguageType.C,
        ".h": LanguageType.C,
        ".cpp": LanguageType.CPP,
        ".cc": LanguageType.CPP,
        ".hpp": LanguageType.CPP,
    }

    def __init__(self, language: LanguageType | str) -> None:
        """Initialize parser for a specific language.

        Args:
            language: LanguageType enum or string (e.g., "typescript")
        """
        if isinstance(language, str):
            language = LanguageType[language.upper()]
        self._language_type = language
        self._parser: Any = None
        self._language: Any = None

    @classmethod
    def from_file(cls, path: Path) -> TreeSitterParser:
        """Create parser based on file extension.

        Args:
            path: Path to source file

        Returns:
            Configured TreeSitterParser

        Raises:
            ValueError: If extension not recognized
        """
        ext = path.suffix.lower()
        if ext not in cls.EXTENSION_MAP:
            raise ValueError(f"Unsupported file extension: {ext}")
        return cls(cls.EXTENSION_MAP[ext])

    def _ensure_initialized(self) -> None:
        """Lazily initialize tree-sitter parser."""
        if self._parser is not None:
            return

        try:
            import tree_sitter
        except ImportError as e:
            raise ImportError(
                "tree-sitter not installed. Install with: pip install tree-sitter"
            ) from e

        # Get the language module
        module_name = self.LANGUAGE_MODULES[self._language_type]
        try:
            lang_module = __import__(module_name)
            # Handle different naming conventions for language functions
            # Most use .language(), but some (like typescript) use language_<name>()
            if hasattr(lang_module, "language"):
                lang_fn = lang_module.language
            elif hasattr(lang_module, f"language_{self._language_type.value}"):
                lang_fn = getattr(lang_module, f"language_{self._language_type.value}")
            else:
                # Fallback: find any language_* function
                lang_fns = [n for n in dir(lang_module) if n.startswith("language_")]
                if lang_fns:
                    lang_fn = getattr(lang_module, lang_fns[0])
                else:
                    raise AttributeError(f"No language function found in {module_name}")
            self._language = tree_sitter.Language(lang_fn())
        except ImportError as e:
            raise ImportError(
                f"Language module {module_name} not installed. "
                f"Install with: pip install {module_name.replace('_', '-')}"
            ) from e

        self._parser = tree_sitter.Parser(self._language)

    def parse(self, source: str | bytes) -> TreeNode:
        """Parse source code into a tree.

        Args:
            source: Source code as string or bytes

        Returns:
            Root TreeNode of the parse tree
        """
        self._ensure_initialized()

        if isinstance(source, str):
            source = source.encode("utf-8")

        tree = self._parser.parse(source)
        return self._convert_node(tree.root_node, source)

    def parse_file(self, path: Path) -> TreeNode:
        """Parse a source file.

        Args:
            path: Path to source file

        Returns:
            Root TreeNode of the parse tree
        """
        source = path.read_bytes()
        return self.parse(source)

    def _convert_node(self, node: Any, source: bytes) -> TreeNode:
        """Convert tree-sitter node to TreeNode."""
        children = [self._convert_node(child, source) for child in node.children]

        return TreeNode(
            type=node.type,
            text=source[node.start_byte : node.end_byte].decode("utf-8", errors="replace"),
            start_line=node.start_point[0],
            end_line=node.end_point[0],
            start_column=node.start_point[1],
            end_column=node.end_point[1],
            children=children,
            named=node.is_named,
        )

    def query(self, tree: TreeNode, query_str: str) -> list[QueryMatch]:
        """Run a tree-sitter query on the parsed tree.

        Args:
            tree: Parsed tree from parse()
            query_str: Tree-sitter query string

        Returns:
            List of QueryMatch results
        """
        self._ensure_initialized()

        # Re-parse to get the raw tree-sitter tree
        # (we need this for querying)

        raw_tree = self._parser.parse(tree.text.encode("utf-8"))
        query = self._language.query(query_str)
        captures = query.captures(raw_tree.root_node)

        # Group captures by pattern
        results: list[QueryMatch] = []
        current_match: dict[str, TreeNode] = {}

        for node, name in captures:
            tree_node = TreeNode(
                type=node.type,
                text=tree.text[node.start_byte : node.end_byte]
                if hasattr(node, "start_byte")
                else "",
                start_line=node.start_point[0],
                end_line=node.end_point[0],
                start_column=node.start_point[1],
                end_column=node.end_point[1],
            )
            current_match[name] = tree_node

        if current_match:
            results.append(QueryMatch(pattern_index=0, captures=current_match))

        return results

    def extract_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from parsed tree.

        Args:
            tree: Parsed tree from parse()

        Returns:
            List of symbols found in the tree
        """
        if self._language_type == LanguageType.TYPESCRIPT:
            return self._extract_typescript_symbols(tree)
        elif self._language_type == LanguageType.JAVASCRIPT:
            return self._extract_javascript_symbols(tree)
        elif self._language_type == LanguageType.PYTHON:
            return self._extract_python_symbols(tree)
        elif self._language_type == LanguageType.GO:
            return self._extract_go_symbols(tree)
        elif self._language_type == LanguageType.RUST:
            return self._extract_rust_symbols(tree)
        else:
            # Generic extraction
            return self._extract_generic_symbols(tree)

    def _extract_typescript_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from TypeScript/JavaScript AST."""
        symbols = []

        def visit(node: TreeNode, parent_name: str | None = None) -> None:
            if node.type == "function_declaration":
                name = self._find_child_text(node, "identifier")
                params = self._find_child_text(node, "formal_parameters")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="function",
                        line=node.start_line,
                        end_line=node.end_line,
                        signature=f"{name}{params}" if params else name,
                        parent=parent_name,
                    )
                )

            elif node.type == "class_declaration":
                name = self._find_child_text(node, "type_identifier") or self._find_child_text(
                    node, "identifier"
                )
                symbol = TSSymbol(
                    name=name or "<anonymous>",
                    kind="class",
                    line=node.start_line,
                    end_line=node.end_line,
                    parent=parent_name,
                )
                symbols.append(symbol)
                # Visit class body for methods
                for child in node.children:
                    if child.type == "class_body":
                        for member in child.children:
                            visit(member, name)

            elif node.type == "method_definition":
                name = self._find_child_text(node, "property_identifier")
                params = self._find_child_text(node, "formal_parameters")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="method",
                        line=node.start_line,
                        end_line=node.end_line,
                        signature=f"{name}{params}" if params else name,
                        parent=parent_name,
                    )
                )

            elif node.type == "interface_declaration":
                name = self._find_child_text(node, "type_identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="interface",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            elif node.type == "type_alias_declaration":
                name = self._find_child_text(node, "type_identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="type",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            # Recurse
            for child in node.children:
                if child.type not in ("class_body", "statement_block"):
                    visit(child, parent_name)

        visit(tree)
        return symbols

    def _extract_javascript_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from JavaScript AST (same as TypeScript minus types)."""
        return self._extract_typescript_symbols(tree)

    def _extract_python_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from Python AST."""
        symbols = []

        def visit(node: TreeNode, parent_name: str | None = None) -> None:
            if node.type == "function_definition":
                name = self._find_child_text(node, "identifier")
                params = self._find_child_text(node, "parameters")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="function",
                        line=node.start_line,
                        end_line=node.end_line,
                        signature=f"def {name}{params}" if params else f"def {name}()",
                        parent=parent_name,
                    )
                )

            elif node.type == "class_definition":
                name = self._find_child_text(node, "identifier")
                symbol = TSSymbol(
                    name=name or "<anonymous>",
                    kind="class",
                    line=node.start_line,
                    end_line=node.end_line,
                    parent=parent_name,
                )
                symbols.append(symbol)
                # Visit body for methods
                for child in node.children:
                    if child.type == "block":
                        for member in child.children:
                            visit(member, name)

            # Recurse
            for child in node.children:
                if child.type != "block":
                    visit(child, parent_name)

        visit(tree)
        return symbols

    def _extract_go_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from Go AST."""
        symbols = []

        def visit(node: TreeNode, parent_name: str | None = None) -> None:
            if node.type == "function_declaration":
                name = self._find_child_text(node, "identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="function",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            elif node.type == "method_declaration":
                name = self._find_child_text(node, "field_identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="method",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            elif node.type == "type_declaration":
                for child in node.children:
                    if child.type == "type_spec":
                        name = self._find_child_text(child, "type_identifier")
                        child_types = [c.type for c in child.children]
                        kind = "struct" if "struct_type" in child_types else "type"
                        symbols.append(
                            TSSymbol(
                                name=name or "<anonymous>",
                                kind=kind,
                                line=node.start_line,
                                end_line=node.end_line,
                                parent=parent_name,
                            )
                        )

            for child in node.children:
                visit(child, parent_name)

        visit(tree)
        return symbols

    def _extract_rust_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Extract symbols from Rust AST."""
        symbols = []

        def visit(node: TreeNode, parent_name: str | None = None) -> None:
            if node.type == "function_item":
                name = self._find_child_text(node, "identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="function",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            elif node.type == "struct_item":
                name = self._find_child_text(node, "type_identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="struct",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            elif node.type == "impl_item":
                type_name = self._find_child_text(node, "type_identifier")
                for child in node.children:
                    if child.type == "declaration_list":
                        for member in child.children:
                            visit(member, type_name)

            elif node.type == "trait_item":
                name = self._find_child_text(node, "type_identifier")
                symbols.append(
                    TSSymbol(
                        name=name or "<anonymous>",
                        kind="trait",
                        line=node.start_line,
                        end_line=node.end_line,
                        parent=parent_name,
                    )
                )

            for child in node.children:
                if child.type != "declaration_list":
                    visit(child, parent_name)

        visit(tree)
        return symbols

    def _extract_generic_symbols(self, tree: TreeNode) -> list[TSSymbol]:
        """Generic symbol extraction for unsupported languages."""
        symbols = []

        def visit(node: TreeNode) -> None:
            # Look for common patterns
            if "function" in node.type or "method" in node.type:
                name = self._find_child_text(node, "identifier") or self._find_child_text(
                    node, "name"
                )
                if name:
                    symbols.append(
                        TSSymbol(
                            name=name,
                            kind="function",
                            line=node.start_line,
                            end_line=node.end_line,
                        )
                    )

            elif "class" in node.type or "struct" in node.type:
                name = self._find_child_text(node, "identifier") or self._find_child_text(
                    node, "name"
                )
                if name:
                    symbols.append(
                        TSSymbol(
                            name=name,
                            kind="class",
                            line=node.start_line,
                            end_line=node.end_line,
                        )
                    )

            for child in node.children:
                visit(child)

        visit(tree)
        return symbols

    def _find_child_text(self, node: TreeNode, child_type: str) -> str | None:
        """Find first child of given type and return its text."""
        for child in node.children:
            if child.type == child_type:
                return child.text
        return None


class TreeSitterSkeletonProvider:
    """Skeleton provider using tree-sitter for multi-language support."""

    def __init__(self, language: LanguageType | str) -> None:
        """Initialize skeleton provider.

        Args:
            language: Target language
        """
        self._parser = TreeSitterParser(language)

    @classmethod
    def from_file(cls, path: Path) -> TreeSitterSkeletonProvider:
        """Create provider based on file extension."""
        parser = TreeSitterParser.from_file(path)
        provider = cls.__new__(cls)
        provider._parser = parser
        return provider

    def extract_skeleton(self, source: str) -> list[TSSymbol]:
        """Extract skeleton from source code.

        Args:
            source: Source code

        Returns:
            List of symbols representing the code skeleton
        """
        tree = self._parser.parse(source)
        return self._parser.extract_symbols(tree)

    def format_skeleton(self, symbols: list[TSSymbol], indent: int = 0) -> str:
        """Format symbols as readable skeleton.

        Args:
            symbols: List of TSSymbol
            indent: Base indentation level

        Returns:
            Formatted skeleton string
        """
        lines = []
        prefix = "  " * indent

        for sym in symbols:
            if sym.signature:
                lines.append(f"{prefix}{sym.kind}: {sym.signature} (L{sym.line}-{sym.end_line})")
            else:
                lines.append(f"{prefix}{sym.kind}: {sym.name} (L{sym.line}-{sym.end_line})")

            if sym.children:
                lines.append(self.format_skeleton(sym.children, indent + 1))

        return "\n".join(lines)


def get_language_for_extension(ext: str) -> LanguageType | None:
    """Get language type for a file extension.

    Args:
        ext: File extension (e.g., ".ts")

    Returns:
        LanguageType or None if not recognized
    """
    return TreeSitterParser.EXTENSION_MAP.get(ext.lower())


def is_supported_extension(ext: str) -> bool:
    """Check if a file extension is supported.

    Args:
        ext: File extension (e.g., ".ts")

    Returns:
        True if extension is supported
    """
    return ext.lower() in TreeSitterParser.EXTENSION_MAP
