"""Multi-file refactoring support.

This module provides tools for coordinated refactoring across multiple files,
including symbol renaming, code moves, and import updates.

Usage:
    from moss.refactoring import Refactorer, RenameRefactoring

    refactorer = Refactorer(workspace)
    refactoring = RenameRefactoring(
        old_name="old_func",
        new_name="new_func",
        scope=RefactoringScope.WORKSPACE,
    )
    result = await refactorer.apply(refactoring)
"""

from __future__ import annotations

import ast
import re
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass


# =============================================================================
# Configuration & Types
# =============================================================================


class RefactoringScope(Enum):
    """Scope of refactoring operation."""

    FILE = "file"  # Single file
    DIRECTORY = "directory"  # Directory and subdirectories
    WORKSPACE = "workspace"  # Entire workspace


class RefactoringKind(Enum):
    """Kind of refactoring operation."""

    RENAME = "rename"
    MOVE = "move"
    EXTRACT = "extract"
    INLINE = "inline"


@dataclass
class FileChange:
    """A change to apply to a single file."""

    path: Path
    original_content: str
    new_content: str
    description: str = ""

    @property
    def has_changes(self) -> bool:
        """Check if there are actual changes."""
        return self.original_content != self.new_content

    def to_diff(self) -> str:
        """Generate unified diff of changes."""
        import difflib

        original_lines = self.original_content.splitlines(keepends=True)
        new_lines = self.new_content.splitlines(keepends=True)

        diff = difflib.unified_diff(
            original_lines,
            new_lines,
            fromfile=f"a/{self.path}",
            tofile=f"b/{self.path}",
        )
        return "".join(diff)


@dataclass
class RefactoringResult:
    """Result of a refactoring operation."""

    success: bool
    changes: list[FileChange] = field(default_factory=list)
    affected_files: list[Path] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def total_changes(self) -> int:
        """Total number of files with changes."""
        return sum(1 for c in self.changes if c.has_changes)


# =============================================================================
# Refactoring Operations
# =============================================================================


@dataclass
class Refactoring(ABC):
    """Base class for refactoring operations."""

    scope: RefactoringScope = RefactoringScope.FILE
    file_patterns: list[str] = field(default_factory=lambda: ["**/*.py"])

    @property
    @abstractmethod
    def kind(self) -> RefactoringKind:
        """The kind of refactoring."""
        ...

    @abstractmethod
    def apply_to_file(self, path: Path, content: str) -> str | None:
        """Apply refactoring to a single file.

        Args:
            path: Path to the file
            content: File content

        Returns:
            New content if changed, None otherwise
        """
        ...


@dataclass
class RenameRefactoring(Refactoring):
    """Rename a symbol across files."""

    old_name: str = ""
    new_name: str = ""
    symbol_type: str | None = None  # class, function, variable, module

    @property
    def kind(self) -> RefactoringKind:
        return RefactoringKind.RENAME

    def apply_to_file(self, path: Path, content: str) -> str | None:
        """Rename occurrences in file."""
        if not self.old_name or not self.new_name:
            return None

        # Use AST to find and rename symbols
        try:
            tree = ast.parse(content)
            transformer = _RenameTransformer(self.old_name, self.new_name, self.symbol_type)
            new_tree = transformer.visit(tree)

            if transformer.changed:
                return ast.unparse(new_tree)
        except SyntaxError:
            # Fall back to text-based replacement for non-Python files
            pass

        # Text-based fallback
        pattern = rf"\b{re.escape(self.old_name)}\b"
        new_content = re.sub(pattern, self.new_name, content)

        return new_content if new_content != content else None


class _RenameTransformer(ast.NodeTransformer):
    """AST transformer for renaming symbols."""

    def __init__(self, old_name: str, new_name: str, symbol_type: str | None = None):
        self.old_name = old_name
        self.new_name = new_name
        self.symbol_type = symbol_type
        self.changed = False

    def visit_Name(self, node: ast.Name) -> ast.Name:
        if node.id == self.old_name:
            self.changed = True
            return ast.Name(id=self.new_name, ctx=node.ctx)
        return node

    def visit_FunctionDef(self, node: ast.FunctionDef) -> ast.FunctionDef:
        self.generic_visit(node)
        if node.name == self.old_name and self.symbol_type in (None, "function"):
            self.changed = True
            node.name = self.new_name
        return node

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> ast.AsyncFunctionDef:
        self.generic_visit(node)
        if node.name == self.old_name and self.symbol_type in (None, "function"):
            self.changed = True
            node.name = self.new_name
        return node

    def visit_ClassDef(self, node: ast.ClassDef) -> ast.ClassDef:
        self.generic_visit(node)
        if node.name == self.old_name and self.symbol_type in (None, "class"):
            self.changed = True
            node.name = self.new_name
        return node

    def visit_alias(self, node: ast.alias) -> ast.alias:
        if node.name == self.old_name:
            self.changed = True
            node.name = self.new_name
        if node.asname == self.old_name:
            self.changed = True
            node.asname = self.new_name
        return node


@dataclass
class MoveRefactoring(Refactoring):
    """Move a symbol to a different file."""

    source_file: Path | None = None
    target_file: Path | None = None
    symbol_name: str = ""
    update_imports: bool = True

    @property
    def kind(self) -> RefactoringKind:
        return RefactoringKind.MOVE

    def apply_to_file(self, path: Path, content: str) -> str | None:
        """Update imports in files when a symbol moves."""
        if not self.source_file or not self.target_file or not self.symbol_name:
            return None

        # Get module names from paths
        source_module = _path_to_module(self.source_file)
        target_module = _path_to_module(self.target_file)

        if not source_module or not target_module:
            return None

        # Update imports
        try:
            tree = ast.parse(content)
            transformer = _ImportUpdater(self.symbol_name, source_module, target_module)
            new_tree = transformer.visit(tree)

            if transformer.changed:
                return ast.unparse(new_tree)
        except SyntaxError:
            pass

        return None


class _ImportUpdater(ast.NodeTransformer):
    """AST transformer for updating imports."""

    def __init__(self, symbol: str, old_module: str, new_module: str):
        self.symbol = symbol
        self.old_module = old_module
        self.new_module = new_module
        self.changed = False

    def visit_ImportFrom(self, node: ast.ImportFrom) -> ast.ImportFrom:
        if node.module == self.old_module:
            for alias in node.names:
                if alias.name == self.symbol:
                    self.changed = True
                    node.module = self.new_module
        return node


@dataclass
class ExtractRefactoring(Refactoring):
    """Extract code to a new function or method."""

    start_line: int = 0
    end_line: int = 0
    new_name: str = ""
    extract_to: str = "function"  # function, method, class

    @property
    def kind(self) -> RefactoringKind:
        return RefactoringKind.EXTRACT

    def apply_to_file(self, path: Path, content: str) -> str | None:
        """Extract selected code to a new function."""
        if not self.start_line or not self.end_line or not self.new_name:
            return None

        lines = content.splitlines(keepends=True)
        if self.start_line < 1 or self.end_line > len(lines):
            return None

        # Extract selected lines
        selected = lines[self.start_line - 1 : self.end_line]
        extracted_code = "".join(selected)

        # Analyze selected code for variables
        try:
            used_vars = _analyze_used_variables(extracted_code)
            returned_vars = _analyze_assigned_variables(extracted_code)
        except SyntaxError:
            return None

        # Build new function
        params = ", ".join(used_vars)
        returns = ", ".join(returned_vars) if returned_vars else "None"

        indent = _get_indent(selected[0]) if selected else ""
        new_func = f"\n{indent}def {self.new_name}({params}):\n"
        for line in selected:
            new_func += f"{indent}    {line.lstrip()}"
        new_func += f"\n{indent}    return {returns}\n"

        # Replace selected code with call
        call_args = ", ".join(used_vars)
        if returned_vars:
            assignment = ", ".join(returned_vars) + " = "
        else:
            assignment = ""
        replacement = f"{indent}{assignment}{self.new_name}({call_args})\n"

        # Build new content
        new_lines = [
            *lines[: self.start_line - 1],
            replacement,
            *lines[self.end_line :],
        ]
        new_content = "".join(new_lines) + new_func

        return new_content


def _analyze_used_variables(code: str) -> list[str]:
    """Analyze code to find used but not defined variables."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []

    assigned = set()
    used = set()

    for node in ast.walk(tree):
        if isinstance(node, ast.Name):
            if isinstance(node.ctx, ast.Store):
                assigned.add(node.id)
            elif isinstance(node.ctx, ast.Load):
                used.add(node.id)

    # Variables used but not assigned within the code
    return sorted(used - assigned)


def _analyze_assigned_variables(code: str) -> list[str]:
    """Analyze code to find assigned variables."""
    try:
        tree = ast.parse(code)
    except SyntaxError:
        return []

    assigned = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Store):
            assigned.add(node.id)

    return sorted(assigned)


def _get_indent(line: str) -> str:
    """Get leading whitespace from a line."""
    return line[: len(line) - len(line.lstrip())]


def _path_to_module(path: Path) -> str | None:
    """Convert a file path to a module name."""
    if not path.suffix == ".py":
        return None

    # Remove .py extension and convert / to .
    module = str(path.with_suffix("")).replace("/", ".").replace("\\", ".")

    # Remove leading . if present
    return module.lstrip(".")


# =============================================================================
# Refactorer
# =============================================================================


class Refactorer:
    """Applies refactoring operations across multiple files."""

    def __init__(self, workspace: Path, exclude_patterns: list[str] | None = None):
        self.workspace = Path(workspace).resolve()
        self.exclude_patterns = exclude_patterns or [
            "**/node_modules/**",
            "**/.git/**",
            "**/__pycache__/**",
            "**/venv/**",
            "**/.venv/**",
        ]

    async def apply(self, refactoring: Refactoring, dry_run: bool = False) -> RefactoringResult:
        """Apply a refactoring operation.

        Args:
            refactoring: The refactoring to apply
            dry_run: If True, don't write changes to disk

        Returns:
            RefactoringResult with changes and status
        """
        result = RefactoringResult(success=True)

        # Find files to process
        files = self._find_files(refactoring.scope, refactoring.file_patterns)

        for path in files:
            try:
                content = path.read_text()
                new_content = refactoring.apply_to_file(path, content)

                if new_content is not None and new_content != content:
                    change = FileChange(
                        path=path,
                        original_content=content,
                        new_content=new_content,
                        description=f"{refactoring.kind.value} in {path.name}",
                    )
                    result.changes.append(change)
                    result.affected_files.append(path)

                    if not dry_run:
                        path.write_text(new_content)

            except Exception as e:
                result.errors.append(f"Error processing {path}: {e}")

        if result.errors:
            result.success = False

        return result

    def _find_files(self, scope: RefactoringScope, patterns: list[str]) -> list[Path]:
        """Find files matching scope and patterns."""
        if scope == RefactoringScope.FILE:
            return []

        files = []
        for pattern in patterns:
            for path in self.workspace.glob(pattern):
                if path.is_file() and not self._is_excluded(path):
                    files.append(path)

        return files

    def _is_excluded(self, path: Path) -> bool:
        """Check if path matches exclusion patterns."""
        import fnmatch

        path_str = str(path)
        for pattern in self.exclude_patterns:
            if fnmatch.fnmatch(path_str, pattern):
                return True
        return False

    def preview(self, refactoring: Refactoring) -> RefactoringResult:
        """Preview changes without applying them."""
        import asyncio

        return asyncio.run(self.apply(refactoring, dry_run=True))

    def generate_diff(self, result: RefactoringResult) -> str:
        """Generate combined diff for all changes."""
        diffs = []
        for change in result.changes:
            if change.has_changes:
                diffs.append(change.to_diff())
        return "\n".join(diffs)


# =============================================================================
# Convenience Functions
# =============================================================================


async def rename_symbol(
    workspace: Path,
    old_name: str,
    new_name: str,
    symbol_type: str | None = None,
    scope: RefactoringScope = RefactoringScope.WORKSPACE,
    dry_run: bool = False,
) -> RefactoringResult:
    """Rename a symbol across the workspace.

    Args:
        workspace: Workspace root directory
        old_name: Current symbol name
        new_name: New symbol name
        symbol_type: Type of symbol (class, function, variable)
        scope: Refactoring scope
        dry_run: If True, don't write changes

    Returns:
        RefactoringResult
    """
    refactorer = Refactorer(workspace)
    refactoring = RenameRefactoring(
        old_name=old_name,
        new_name=new_name,
        symbol_type=symbol_type,
        scope=scope,
    )
    return await refactorer.apply(refactoring, dry_run=dry_run)


async def move_symbol(
    workspace: Path,
    symbol_name: str,
    source_file: Path,
    target_file: Path,
    dry_run: bool = False,
) -> RefactoringResult:
    """Move a symbol to a different file and update imports.

    Args:
        workspace: Workspace root directory
        symbol_name: Name of the symbol to move
        source_file: Current file containing the symbol
        target_file: Target file to move to
        dry_run: If True, don't write changes

    Returns:
        RefactoringResult
    """
    refactorer = Refactorer(workspace)
    refactoring = MoveRefactoring(
        source_file=source_file,
        target_file=target_file,
        symbol_name=symbol_name,
        scope=RefactoringScope.WORKSPACE,
    )
    return await refactorer.apply(refactoring, dry_run=dry_run)


async def extract_function(
    path: Path,
    start_line: int,
    end_line: int,
    new_name: str,
    dry_run: bool = False,
) -> RefactoringResult:
    """Extract code to a new function.

    Args:
        path: File path
        start_line: Start line of code to extract
        end_line: End line of code to extract
        new_name: Name for the new function
        dry_run: If True, don't write changes

    Returns:
        RefactoringResult
    """
    workspace = path.parent
    refactorer = Refactorer(workspace)
    refactoring = ExtractRefactoring(
        start_line=start_line,
        end_line=end_line,
        new_name=new_name,
        scope=RefactoringScope.FILE,
        file_patterns=[path.name],
    )
    return await refactorer.apply(refactoring, dry_run=dry_run)
