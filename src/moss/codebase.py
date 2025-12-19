"""Unified codebase tree: filesystem + AST merged.

See docs/codebase-tree.md for design.
"""

from __future__ import annotations

import ast
from collections.abc import Iterator
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path


class NodeKind(Enum):
    """Kind of tree node."""

    ROOT = "root"
    DIRECTORY = "directory"
    FILE = "file"
    CLASS = "class"
    FUNCTION = "function"
    METHOD = "method"
    CONSTANT = "constant"


@dataclass
class Node:
    """A node in the codebase tree."""

    kind: NodeKind
    name: str
    path: Path  # Filesystem path (for file/dir) or logical path (for symbols)
    parent: Node | None = None
    children: list[Node] = field(default_factory=list)

    # Metadata (populated lazily)
    description: str = ""  # First line of docstring or inferred
    signature: str = ""  # For functions/methods
    lineno: int = 0  # Line number in file (for symbols)
    end_lineno: int = 0

    def __repr__(self) -> str:
        return f"Node({self.kind.value}, {self.name!r})"

    @property
    def full_path(self) -> str:
        """Full path from root, e.g., 'src/moss/dwim.py:ToolRouter.analyze_intent'."""
        parts = []
        node: Node | None = self
        while node and node.kind != NodeKind.ROOT:
            parts.append(node.name)
            node = node.parent
        parts.reverse()

        # Join with appropriate separators
        result: list[str] = []
        for i, part in enumerate(parts):
            if i == 0:
                result.append(part)
            elif result and (result[-1].endswith(".py") or ":" in "".join(result)):
                # After a file, use colon; within symbols, use dot
                if result[-1].endswith(".py"):
                    result.append(":")
                else:
                    result.append(".")
                result.append(part)
            else:
                result.append("/")
                result.append(part)
        return "".join(result)

    def add_child(self, child: Node) -> Node:
        """Add a child node."""
        child.parent = self
        self.children.append(child)
        return child

    def find(self, name: str) -> Node | None:
        """Find immediate child by name."""
        for child in self.children:
            if child.name == name:
                return child
        return None

    def walk(self) -> Iterator[Node]:
        """Walk all descendants depth-first."""
        yield self
        for child in self.children:
            yield from child.walk()


class CodebaseTree:
    """Unified view of a codebase: filesystem + AST."""

    def __init__(self, root: Path):
        self.root_path = root.resolve()
        self._root = Node(NodeKind.ROOT, root.name, root)
        self._cache: dict[Path, Node] = {}

    @property
    def root(self) -> Node:
        return self._root

    def get(self, path: str | Path) -> Node | None:
        """Get a node by path (file path or symbol path).

        Examples:
            tree.get("src/moss/dwim.py")
            tree.get("src/moss/dwim.py:ToolRouter")
            tree.get("ToolRouter.analyze_intent")
        """
        if isinstance(path, Path):
            path = str(path)

        # Handle symbol paths (file:symbol or just symbol)
        if ":" in path:
            file_part, symbol_part = path.split(":", 1)
            file_node = self._get_file_node(Path(file_part))
            if not file_node:
                return None
            return self._find_symbol(file_node, symbol_part)

        # Try as file path first
        p = Path(path)
        if p.suffix or (self.root_path / p).exists():
            return self._get_file_node(p)

        # Try as symbol name (search)
        return self._find_symbol_globally(path)

    def _get_file_node(self, path: Path) -> Node | None:
        """Get or create node for a file/directory path."""
        if not path.is_absolute():
            path = self.root_path / path

        if path in self._cache:
            return self._cache[path]

        if not path.exists():
            return None

        # Build path from root
        try:
            rel = path.relative_to(self.root_path)
        except ValueError:
            return None

        current = self._root
        current_path = self.root_path

        for part in rel.parts:
            current_path = current_path / part
            child = current.find(part)
            if not child:
                kind = NodeKind.DIRECTORY if current_path.is_dir() else NodeKind.FILE
                child = current.add_child(Node(kind, part, current_path))
                self._cache[current_path] = child

                # If it's a Python file, parse it
                if kind == NodeKind.FILE and current_path.suffix == ".py":
                    self._parse_python_file(child)

            current = child

        return current

    def _parse_python_file(self, file_node: Node) -> None:
        """Parse a Python file and add symbol children."""
        try:
            source = file_node.path.read_text()
            tree = ast.parse(source)
        except (SyntaxError, OSError):
            return

        # Get module docstring
        file_node.description = ast.get_docstring(tree) or ""
        if file_node.description:
            file_node.description = file_node.description.split("\n")[0]

        for node in tree.body:
            self._add_ast_node(file_node, node)

    def _add_ast_node(self, parent: Node, node: ast.AST) -> None:
        """Add an AST node to the tree."""
        if isinstance(node, ast.ClassDef):
            class_node = parent.add_child(
                Node(
                    kind=NodeKind.CLASS,
                    name=node.name,
                    path=parent.path,
                    description=self._get_docstring_first_line(node),
                    lineno=node.lineno,
                    end_lineno=node.end_lineno or node.lineno,
                )
            )
            # Add methods
            for item in node.body:
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    class_node.add_child(
                        Node(
                            kind=NodeKind.METHOD,
                            name=item.name,
                            path=parent.path,
                            description=self._get_docstring_first_line(item),
                            signature=self._get_signature(item),
                            lineno=item.lineno,
                            end_lineno=item.end_lineno or item.lineno,
                        )
                    )

        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            parent.add_child(
                Node(
                    kind=NodeKind.FUNCTION,
                    name=node.name,
                    path=parent.path,
                    description=self._get_docstring_first_line(node),
                    signature=self._get_signature(node),
                    lineno=node.lineno,
                    end_lineno=node.end_lineno or node.lineno,
                )
            )

        elif isinstance(node, ast.Assign):
            # Top-level constants (UPPER_CASE)
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id.isupper():
                    parent.add_child(
                        Node(
                            kind=NodeKind.CONSTANT,
                            name=target.id,
                            path=parent.path,
                            lineno=node.lineno,
                            end_lineno=node.end_lineno or node.lineno,
                        )
                    )

    def _get_docstring_first_line(self, node: ast.AST) -> str:
        """Get first line of docstring."""
        doc = ast.get_docstring(node)
        if doc:
            return doc.split("\n")[0]
        return ""

    def _get_signature(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> str:
        """Get function signature."""
        args = []
        for arg in node.args.args:
            arg_str = arg.arg
            if arg.annotation:
                arg_str += f": {ast.unparse(arg.annotation)}"
            args.append(arg_str)

        ret = ""
        if node.returns:
            ret = f" -> {ast.unparse(node.returns)}"

        prefix = "async " if isinstance(node, ast.AsyncFunctionDef) else ""
        return f"{prefix}def {node.name}({', '.join(args)}){ret}"

    def _find_symbol(self, file_node: Node, symbol_path: str) -> Node | None:
        """Find a symbol within a file node.

        symbol_path can be "ClassName" or "ClassName.method_name"
        """
        parts = symbol_path.split(".")
        current = file_node

        for part in parts:
            found = current.find(part)
            if not found:
                return None
            current = found

        return current

    def _find_symbol_globally(self, name: str) -> Node | None:
        """Search for a symbol by name across all loaded files."""
        for node in self._root.walk():
            if node.name == name and node.kind in (
                NodeKind.CLASS,
                NodeKind.FUNCTION,
                NodeKind.METHOD,
            ):
                return node
        return None


def build_tree(root: Path) -> CodebaseTree:
    """Build a codebase tree for a directory."""
    return CodebaseTree(root)
