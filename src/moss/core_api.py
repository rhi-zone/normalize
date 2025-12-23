"""Core API - 4 primitive APIs matching CLI/MCP interface.

ViewAPI   - View codebase nodes (directories, files, symbols)
EditAPI   - Structural code modifications
AnalyzeAPI - Health, complexity, and security analysis
SearchAPI - Search for symbols, text patterns, and files

These APIs wrap the Rust CLI for fast, consistent behavior across
CLI, MCP, and programmatic access.
"""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from moss_intelligence import rust_shim


@dataclass
class ViewResult:
    """Result of viewing a codebase node."""

    target: str
    kind: str  # "directory", "file", "symbol"
    content: dict[str, Any]
    raw: dict[str, Any] = field(default_factory=dict)

    def to_compact(self) -> str:
        """Return compact format for display."""
        if self.kind == "directory":
            files = self.content.get("files", [])
            return f"{self.target}: {len(files)} files"
        elif self.kind == "file":
            symbols = self.content.get("symbols", [])
            return f"{self.target}: {len(symbols)} symbols"
        else:
            return f"{self.target}: {self.content.get('signature', '')}"


@dataclass
class EditResult:
    """Result of a structural edit operation."""

    target: str
    operation: str
    success: bool
    message: str
    diff: str = ""
    raw: dict[str, Any] = field(default_factory=dict)

    def to_compact(self) -> str:
        """Return compact format for display."""
        status = "OK" if self.success else "FAILED"
        return f"[{status}] {self.operation} {self.target}: {self.message}"


@dataclass
class AnalyzeResult:
    """Result of codebase analysis."""

    target: str
    health: dict[str, Any] | None = None
    complexity: dict[str, Any] | None = None
    security: dict[str, Any] | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    def to_compact(self) -> str:
        """Return compact format for display."""
        parts = [f"Analysis: {self.target}"]
        if self.health:
            score = self.health.get("avg_complexity", 0)
            parts.append(f"  Health: {score:.1f} avg complexity")
        if self.complexity:
            funcs = len(self.complexity.get("functions", []))
            parts.append(f"  Complexity: {funcs} functions analyzed")
        if self.security:
            findings = len(self.security.get("findings", []))
            parts.append(f"  Security: {findings} findings")
        return "\n".join(parts)


class ViewAPI:
    """API for viewing codebase nodes (directories, files, symbols).

    Wraps the Rust `moss view` command for fast, consistent results.
    """

    def __init__(self, root: Path | None = None):
        self.root = root or Path.cwd()

    def view(
        self,
        target: str | None = None,
        depth: int = 1,
        line_numbers: bool = False,
        deps: bool = False,
        kind: str | None = None,
        calls: bool = False,
        called_by: bool = False,
    ) -> ViewResult:
        """View a node in the codebase tree.

        Args:
            target: Path to view (file, directory, or symbol like src/foo.py/Bar)
            depth: Expansion depth (0=names, 1=signatures, 2=children, -1=all)
            line_numbers: Include line numbers
            deps: Show dependencies (imports/exports)
            kind: Filter by symbol type (class, function, method)
            calls: Show symbols that call the target
            called_by: Show symbols the target calls

        Returns:
            ViewResult with node content
        """
        result = rust_shim.rust_view(
            target=target,
            depth=depth,
            line_numbers=line_numbers,
            deps=deps,
            kind=kind,
            calls=calls,
            called_by=called_by,
            root=str(self.root),
        )

        if result is None:
            return ViewResult(
                target=target or ".",
                kind="unknown",
                content={},
                raw={},
            )

        # Determine kind from result
        node_kind = "file"
        if result.get("type") == "directory":
            node_kind = "directory"
        elif "/" in (target or "") and not Path(target or "").suffix:
            node_kind = "symbol"

        return ViewResult(
            target=target or ".",
            kind=node_kind,
            content=result,
            raw=result,
        )


class EditAPI:
    """API for structural code modifications.

    Wraps the Rust `moss edit` command for precise, AST-aware edits.
    """

    def __init__(self, root: Path | None = None):
        self.root = root or Path.cwd()

    def edit(
        self,
        target: str,
        delete: bool = False,
        replace: str | None = None,
        before: str | None = None,
        after: str | None = None,
        prepend: str | None = None,
        append: str | None = None,
        move_before: str | None = None,
        move_after: str | None = None,
        swap: str | None = None,
        dry_run: bool = False,
    ) -> EditResult:
        """Edit a node in the codebase tree.

        Args:
            target: Path to edit (like src/foo.py/Bar/method)
            delete: Delete the target node
            replace: Replace target with new content
            before: Insert content before target
            after: Insert content after target
            prepend: Insert at beginning of target container
            append: Insert at end of target container
            move_before: Move target before another node
            move_after: Move target after another node
            swap: Swap target with another node
            dry_run: Show what would change without applying

        Returns:
            EditResult with operation status
        """
        result = rust_shim.rust_edit(
            target=target,
            delete=delete,
            replace=replace,
            before=before,
            after=after,
            prepend=prepend,
            append=append,
            move_before=move_before,
            move_after=move_after,
            swap=swap,
            dry_run=dry_run,
            root=str(self.root),
        )

        if result is None:
            return EditResult(
                target=target,
                operation="unknown",
                success=False,
                message="Rust CLI not available",
            )

        # Determine operation type
        op = "edit"
        if delete:
            op = "delete"
        elif replace:
            op = "replace"
        elif before or after:
            op = "insert"
        elif prepend or append:
            op = "insert"
        elif move_before or move_after:
            op = "move"
        elif swap:
            op = "swap"

        return EditResult(
            target=target,
            operation=op,
            success=result.get("success", False),
            message=result.get("message", ""),
            diff=result.get("diff", ""),
            raw=result,
        )

    def delete(self, target: str, dry_run: bool = False) -> EditResult:
        """Delete a symbol or file.

        Convenience wrapper for delete operation.
        """
        return self.edit(target, delete=True, dry_run=dry_run)

    def replace(self, target: str, content: str, dry_run: bool = False) -> EditResult:
        """Replace a symbol with new content.

        Convenience wrapper for replace operation.
        """
        return self.edit(target, replace=content, dry_run=dry_run)

    def insert_before(self, target: str, content: str, dry_run: bool = False) -> EditResult:
        """Insert content before a symbol.

        Convenience wrapper for before operation.
        """
        return self.edit(target, before=content, dry_run=dry_run)

    def insert_after(self, target: str, content: str, dry_run: bool = False) -> EditResult:
        """Insert content after a symbol.

        Convenience wrapper for after operation.
        """
        return self.edit(target, after=content, dry_run=dry_run)


class AnalyzeAPI:
    """API for codebase analysis (health, complexity, security).

    Wraps the Rust `moss analyze` command for comprehensive analysis.
    """

    def __init__(self, root: Path | None = None):
        self.root = root or Path.cwd()

    def analyze(
        self,
        target: str | None = None,
        health: bool = False,
        complexity: bool = False,
        security: bool = False,
        threshold: int | None = None,
    ) -> AnalyzeResult:
        """Analyze codebase health, complexity, and security.

        Args:
            target: Path to analyze (defaults to cwd)
            health: Run health analysis
            complexity: Run complexity analysis
            security: Run security analysis
            threshold: Complexity threshold filter

        If no flags specified, runs all analyses.

        Returns:
            AnalyzeResult with analysis data
        """
        # If no flags, run all
        if not health and not complexity and not security:
            health = complexity = security = True

        result = rust_shim.rust_analyze(
            target=target,
            health=health,
            complexity=complexity,
            security=security,
            threshold=threshold,
            root=str(self.root),
        )

        if result is None:
            return AnalyzeResult(target=target or ".")

        return AnalyzeResult(
            target=result.get("target", target or "."),
            health=result.get("health"),
            complexity=result.get("complexity"),
            security=result.get("security"),
            raw=result,
        )

    def health(self, target: str | None = None) -> AnalyzeResult:
        """Run health analysis only.

        Convenience wrapper for health-only analysis.
        """
        return self.analyze(target=target, health=True)

    def complexity(self, target: str | None = None, threshold: int | None = None) -> AnalyzeResult:
        """Run complexity analysis only.

        Convenience wrapper for complexity-only analysis.
        """
        return self.analyze(target=target, complexity=True, threshold=threshold)

    def security(self, target: str | None = None) -> AnalyzeResult:
        """Run security analysis only.

        Convenience wrapper for security-only analysis.
        """
        return self.analyze(target=target, security=True)


# Re-export SearchAPI from moss_api for completeness
# SearchAPI already exists and wraps Rust CLI
