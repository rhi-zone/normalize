"""Moss Python Style Guide.

This file demonstrates the coding conventions used in moss.
Use it as a reference for in-context learning when working on this codebase.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

# Use TYPE_CHECKING for import-only types to avoid circular imports
if TYPE_CHECKING:
    from collections.abc import Callable  # noqa: F401 - used in type alias below


# =============================================================================
# Enums: Use for finite sets of values
# =============================================================================


class Severity(Enum):
    """Severity levels for issues."""

    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


# =============================================================================
# Dataclasses: Use for data containers
# =============================================================================


@dataclass
class Issue:
    """An issue found during analysis.

    Dataclasses are preferred for simple data containers.
    Use field() for mutable defaults.
    """

    message: str
    severity: Severity
    file: Path | None = None
    line: int | None = None
    tags: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "message": self.message,
            "severity": self.severity.value,
            "file": str(self.file) if self.file else None,
            "line": self.line,
            "tags": self.tags,
        }


# =============================================================================
# Protocols: Use for duck typing interfaces
# =============================================================================


@runtime_checkable
class Validator(Protocol):
    """Protocol for validators.

    Use Protocol for interfaces that don't require inheritance.
    @runtime_checkable allows isinstance() checks.
    """

    @property
    def name(self) -> str:
        """Validator name."""
        ...

    def validate(self, code: str) -> list[Issue]:
        """Validate code and return issues."""
        ...


# =============================================================================
# Classes: Use for stateful components
# =============================================================================


class Analyzer:
    """Code analyzer.

    Classes are used for stateful components with behavior.
    """

    def __init__(self, root: Path, *, strict: bool = False) -> None:
        """Initialize analyzer.

        Use keyword-only args (after *) for optional configuration.
        """
        self.root = root.resolve()
        self.strict = strict
        self._cache: dict[Path, list[Issue]] = {}

    def analyze(self, path: Path) -> list[Issue]:
        """Analyze a file.

        Public methods have docstrings.
        """
        if path in self._cache:
            return self._cache[path]

        issues = self._do_analysis(path)
        self._cache[path] = issues
        return issues

    def _do_analysis(self, path: Path) -> list[Issue]:
        """Internal analysis implementation.

        Private methods start with underscore.
        """
        # Implementation here
        return []


# =============================================================================
# Functions: Use for stateless operations
# =============================================================================


def find_issues(
    code: str,
    *,
    include_warnings: bool = True,
    max_issues: int | None = None,
) -> list[Issue]:
    """Find issues in code.

    Functions are used for stateless operations.

    Args:
        code: Source code to analyze.
        include_warnings: Whether to include warnings (default: True).
        max_issues: Maximum issues to return (default: None = unlimited).

    Returns:
        List of issues found.

    Example:
        >>> issues = find_issues("def foo(): pass")
        >>> len(issues)
        0
    """
    issues: list[Issue] = []
    # Implementation here
    if max_issues is not None:
        issues = issues[:max_issues]
    return issues


# =============================================================================
# Async: Use for I/O-bound operations
# =============================================================================


async def analyze_async(path: Path) -> list[Issue]:
    """Analyze a file asynchronously.

    Use async for I/O-bound operations (file reading, network, etc.).
    """
    # Use asyncio for concurrent operations
    content = path.read_text()  # In real code, use aiofiles
    return find_issues(content)


# =============================================================================
# Type Aliases: Use for complex types
# =============================================================================

# Simple alias
IssueList = list[Issue]

# Callable type
ValidatorFunc = "Callable[[str], list[Issue]]"


# =============================================================================
# Constants: Use UPPER_SNAKE_CASE
# =============================================================================

DEFAULT_MAX_LINE_LENGTH = 100
SUPPORTED_EXTENSIONS = {".py", ".pyi"}


# =============================================================================
# Error Handling
# =============================================================================


class AnalysisError(Exception):
    """Raised when analysis fails.

    Custom exceptions inherit from Exception.
    """

    def __init__(self, message: str, path: Path | None = None) -> None:
        super().__init__(message)
        self.path = path


def safe_analyze(path: Path) -> list[Issue] | None:
    """Safely analyze a file, returning None on error.

    Use | None return type for operations that can fail gracefully.
    """
    try:
        return Analyzer(path.parent).analyze(path)
    except AnalysisError:
        return None
