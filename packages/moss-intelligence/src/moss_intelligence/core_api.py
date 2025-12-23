"""Core APIs for TUI integration.

Provides ViewAPI and AnalyzeAPI for unified code viewing and analysis.
These are lightweight wrappers around moss-intelligence primitives.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class ViewResult:
    """Result of a view operation."""

    target: str
    kind: str  # "directory", "file", "symbol", "unknown"
    content: dict[str, Any] = field(default_factory=dict)


class ViewAPI:
    """API for viewing code structure.

    Provides unified access to directory listings, file skeletons,
    and symbol source code.
    """

    def __init__(self, root: Path):
        self.root = root

    def _resolve_path(self, path: str | Path) -> Path:
        """Resolve path relative to root."""
        p = Path(path)
        if not p.is_absolute():
            p = self.root / p
        return p

    def view(self, target: str, depth: int = 1) -> ViewResult:
        """View a target (directory, file, or symbol).

        Args:
            target: Path or symbol to view
            depth: Depth for directory listings

        Returns:
            ViewResult with kind and content
        """
        from .codebase import CodebaseTree
        from .skeleton import extract_python_skeleton

        # Try to resolve as path first
        path = self._resolve_path(target)

        if path.is_dir():
            # Directory listing
            files = []
            for item in sorted(path.iterdir()):
                if item.name.startswith("."):
                    continue
                prefix = "d " if item.is_dir() else "f "
                files.append(prefix + item.name)
            return ViewResult(
                target=str(path.relative_to(self.root)),
                kind="directory",
                content={"files": files},
            )

        if path.is_file():
            # File skeleton
            if path.suffix == ".py":
                try:
                    source = path.read_text()
                    symbols = extract_python_skeleton(source)
                    symbol_dicts = [
                        {
                            "name": s.name,
                            "kind": s.kind,
                            "signature": s.signature,
                            "docstring": s.docstring,
                            "lineno": s.lineno,
                        }
                        for s in symbols
                    ]
                    line_count = len(source.splitlines())
                    return ViewResult(
                        target=str(path.relative_to(self.root)),
                        kind="file",
                        content={"symbols": symbol_dicts, "line_count": line_count},
                    )
                except (OSError, SyntaxError):
                    pass

            # Non-Python or failed to parse
            try:
                line_count = len(path.read_text().splitlines())
            except OSError:
                line_count = 0
            return ViewResult(
                target=str(path.relative_to(self.root)),
                kind="file",
                content={"symbols": [], "line_count": line_count},
            )

        # Try as symbol path (file:symbol or fuzzy)
        tree = CodebaseTree(self.root)
        nodes = tree.resolve(target)
        if nodes:
            node = nodes[0]
            # Get source for the symbol
            if node.lineno > 0:
                try:
                    lines = node.path.read_text().splitlines()
                    source = "\n".join(lines[node.lineno - 1 : node.end_lineno])
                    return ViewResult(
                        target=node.full_path,
                        kind="symbol",
                        content={
                            "source": source,
                            "signature": node.signature,
                            "description": node.description,
                        },
                    )
                except OSError:
                    pass

            return ViewResult(
                target=node.full_path,
                kind="symbol",
                content={"signature": node.signature, "description": node.description},
            )

        return ViewResult(target=target, kind="unknown", content={})


@dataclass
class AnalyzeResult:
    """Result of an analysis operation."""

    target: str
    health: dict[str, Any] | None = None
    complexity: dict[str, Any] | None = None
    security: dict[str, Any] | None = None


class AnalyzeAPI:
    """API for code analysis.

    Provides unified access to health checks, complexity analysis,
    and security scanning.
    """

    def __init__(self, root: Path):
        self.root = root

    def _resolve_path(self, path: str | Path) -> Path:
        """Resolve path relative to root."""
        p = Path(path)
        if not p.is_absolute():
            p = self.root / p
        return p

    def analyze(
        self,
        target: str = ".",
        health: bool = False,
        complexity: bool = False,
        security: bool = False,
    ) -> AnalyzeResult:
        """Analyze a target with selected analyses.

        Args:
            target: Path to analyze
            health: Run health checks
            complexity: Run complexity analysis
            security: Run security analysis

        Returns:
            AnalyzeResult with requested analyses
        """
        path = self._resolve_path(target)
        result = AnalyzeResult(target=str(path.relative_to(self.root)))

        if health:
            result.health = self._analyze_health(path)

        if complexity:
            result.complexity = self._analyze_complexity(path)

        if security:
            result.security = self._analyze_security(path)

        return result

    def _analyze_health(self, path: Path) -> dict[str, Any]:
        """Run health checks on a path."""
        health: dict[str, Any] = {}

        if path.is_dir():
            # Count files by type
            py_files = list(path.rglob("*.py"))
            health["python_files"] = len(py_files)
            health["total_lines"] = sum(
                len(f.read_text().splitlines()) for f in py_files if f.is_file()
            )

            # Check for common issues
            issues = []
            if not (path / "README.md").exists() and not (path / "README.rst").exists():
                issues.append("No README found")
            if not (path / "pyproject.toml").exists() and not (path / "setup.py").exists():
                issues.append("No pyproject.toml or setup.py")

            health["issues"] = issues
        else:
            # Single file
            try:
                content = path.read_text()
                health["lines"] = len(content.splitlines())
                health["size"] = len(content)
            except OSError as e:
                health["error"] = str(e)

        return health

    def _analyze_complexity(self, path: Path) -> dict[str, Any]:
        """Run complexity analysis."""
        from .complexity import analyze_complexity

        try:
            report = analyze_complexity(path)
            functions = [
                {
                    "name": f.name,
                    "file": str(f.file.relative_to(self.root)) if f.file else "",
                    "complexity": f.complexity,
                    "line": f.line,
                }
                for f in report.functions[:50]  # Limit to top 50
            ]
            return {
                "functions": functions,
                "total": report.total_complexity,
                "average": report.average_complexity,
            }
        except Exception as e:
            return {"error": str(e), "functions": []}

    def _analyze_security(self, path: Path) -> dict[str, Any]:
        """Run security analysis."""
        from .security import analyze_security

        try:
            analysis = analyze_security(path)
            return analysis
        except Exception as e:
            return {"error": str(e), "findings": []}


__all__ = ["ViewAPI", "ViewResult", "AnalyzeAPI", "AnalyzeResult"]
