"""Diff analysis for code changes.

This module provides analysis of git diffs to understand what changed
between commits, including:
- Modified functions and classes
- Added/removed symbols
- Complexity changes
- Impact assessment

Usage:
    from moss.diff_analysis import analyze_diff, get_commit_diff

    # Analyze diff between commits
    diff = get_commit_diff(Path("."), "HEAD~1", "HEAD")
    analysis = analyze_diff(diff)

    # Or analyze staged changes
    diff = get_staged_diff(Path("."))
    analysis = analyze_diff(diff)
"""

from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass
class FileDiff:
    """Represents a diff for a single file."""

    path: Path
    old_path: Path | None = None  # For renames
    status: str = "modified"  # added, modified, deleted, renamed
    additions: int = 0
    deletions: int = 0
    hunks: list[str] = field(default_factory=list)


@dataclass
class SymbolChange:
    """A change to a code symbol (function, class, etc.)."""

    name: str
    kind: str  # function, class, method
    change_type: str  # added, modified, deleted
    file_path: Path
    line_start: int = 0
    line_end: int = 0
    old_signature: str | None = None
    new_signature: str | None = None


@dataclass
class DiffAnalysis:
    """Analysis of a git diff."""

    files_changed: int = 0
    files_added: int = 0
    files_deleted: int = 0
    files_renamed: int = 0
    total_additions: int = 0
    total_deletions: int = 0
    file_diffs: list[FileDiff] = field(default_factory=list)
    symbol_changes: list[SymbolChange] = field(default_factory=list)
    summary: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "files_changed": self.files_changed,
            "files_added": self.files_added,
            "files_deleted": self.files_deleted,
            "files_renamed": self.files_renamed,
            "total_additions": self.total_additions,
            "total_deletions": self.total_deletions,
            "files": [
                {
                    "path": str(f.path),
                    "status": f.status,
                    "additions": f.additions,
                    "deletions": f.deletions,
                }
                for f in self.file_diffs
            ],
            "symbol_changes": [
                {
                    "name": s.name,
                    "kind": s.kind,
                    "change_type": s.change_type,
                    "file": str(s.file_path),
                }
                for s in self.symbol_changes
            ],
            "summary": self.summary,
        }


def get_commit_diff(repo_path: Path, from_ref: str, to_ref: str = "HEAD") -> str:
    """Get diff between two commits.

    Args:
        repo_path: Path to git repository
        from_ref: Starting commit reference
        to_ref: Ending commit reference

    Returns:
        Unified diff output
    """
    result = subprocess.run(
        ["git", "diff", from_ref, to_ref, "-U3"],
        capture_output=True,
        text=True,
        cwd=repo_path,
        check=True,
    )
    return result.stdout


def get_staged_diff(repo_path: Path) -> str:
    """Get diff of staged changes.

    Args:
        repo_path: Path to git repository

    Returns:
        Unified diff output
    """
    result = subprocess.run(
        ["git", "diff", "--cached", "-U3"],
        capture_output=True,
        text=True,
        cwd=repo_path,
        check=True,
    )
    return result.stdout


def get_working_diff(repo_path: Path) -> str:
    """Get diff of unstaged working directory changes.

    Args:
        repo_path: Path to git repository

    Returns:
        Unified diff output
    """
    result = subprocess.run(
        ["git", "diff", "-U3"],
        capture_output=True,
        text=True,
        cwd=repo_path,
        check=True,
    )
    return result.stdout


def get_diff_stat(repo_path: Path, from_ref: str, to_ref: str = "HEAD") -> str:
    """Get diff statistics (compact summary).

    Args:
        repo_path: Path to git repository
        from_ref: Starting commit reference
        to_ref: Ending commit reference

    Returns:
        Diff stat output
    """
    result = subprocess.run(
        ["git", "diff", "--stat", from_ref, to_ref],
        capture_output=True,
        text=True,
        cwd=repo_path,
        check=True,
    )
    return result.stdout


def parse_diff(diff_output: str) -> list[FileDiff]:
    """Parse unified diff output into FileDiff objects.

    Args:
        diff_output: Unified diff output

    Returns:
        List of FileDiff objects
    """
    file_diffs: list[FileDiff] = []
    current_file: FileDiff | None = None
    current_hunk: list[str] = []

    for line in diff_output.split("\n"):
        # New file diff
        if line.startswith("diff --git"):
            if current_file:
                if current_hunk:
                    current_file.hunks.append("\n".join(current_hunk))
                file_diffs.append(current_file)

            # Parse file paths
            match = re.match(r"diff --git a/(.*) b/(.*)", line)
            if match:
                old_path, new_path = match.groups()
                current_file = FileDiff(path=Path(new_path))
                if old_path != new_path:
                    current_file.old_path = Path(old_path)
                    current_file.status = "renamed"
            current_hunk = []

        # File status
        elif line.startswith("new file"):
            if current_file:
                current_file.status = "added"
        elif line.startswith("deleted file"):
            if current_file:
                current_file.status = "deleted"

        # Hunk header
        elif line.startswith("@@"):
            if current_file and current_hunk:
                current_file.hunks.append("\n".join(current_hunk))
            current_hunk = [line]

        # Diff content
        elif current_file and current_hunk:
            current_hunk.append(line)
            if line.startswith("+") and not line.startswith("+++"):
                current_file.additions += 1
            elif line.startswith("-") and not line.startswith("---"):
                current_file.deletions += 1

    # Don't forget the last file
    if current_file:
        if current_hunk:
            current_file.hunks.append("\n".join(current_hunk))
        file_diffs.append(current_file)

    return file_diffs


def analyze_symbol_changes(file_diffs: list[FileDiff]) -> list[SymbolChange]:
    """Analyze symbol changes in diffs.

    Identifies functions, classes, and methods that were added, modified,
    or deleted based on diff hunks.

    Args:
        file_diffs: List of file diffs

    Returns:
        List of symbol changes
    """
    changes: list[SymbolChange] = []

    # Python function/class patterns
    # Method pattern: 4+ spaces of indentation (inside a class)
    method_pattern = re.compile(r"^[+-]\s{4,}def\s+(\w+)\s*\(")
    # Function pattern: 0-3 spaces (top-level)
    func_pattern = re.compile(r"^[+-]\s{0,3}def\s+(\w+)\s*\(")
    class_pattern = re.compile(r"^[+-]\s*class\s+(\w+)")

    for file_diff in file_diffs:
        if not str(file_diff.path).endswith(".py"):
            continue

        for hunk in file_diff.hunks:
            for line in hunk.split("\n"):
                # Check for method definitions first (more specific)
                method_match = method_pattern.match(line)
                if method_match:
                    name = method_match.group(1)
                    change_type = "added" if line.startswith("+") else "deleted"
                    changes.append(
                        SymbolChange(
                            name=name,
                            kind="method",
                            change_type=change_type,
                            file_path=file_diff.path,
                        )
                    )
                    continue

                # Check for function definitions (top-level)
                func_match = func_pattern.match(line)
                if func_match:
                    name = func_match.group(1)
                    change_type = "added" if line.startswith("+") else "deleted"
                    changes.append(
                        SymbolChange(
                            name=name,
                            kind="function",
                            change_type=change_type,
                            file_path=file_diff.path,
                        )
                    )
                    continue

                # Check for class definitions
                class_match = class_pattern.match(line)
                if class_match:
                    name = class_match.group(1)
                    change_type = "added" if line.startswith("+") else "deleted"
                    changes.append(
                        SymbolChange(
                            name=name,
                            kind="class",
                            change_type=change_type,
                            file_path=file_diff.path,
                        )
                    )

    # Identify modifications (same symbol deleted and added)
    by_name: dict[tuple[str, Path], list[SymbolChange]] = {}
    for change in changes:
        key = (change.name, change.file_path)
        if key not in by_name:
            by_name[key] = []
        by_name[key].append(change)

    # Convert add+delete pairs to modifications
    final_changes: list[SymbolChange] = []
    processed: set[int] = set()

    for key, group in by_name.items():
        if len(group) >= 2:
            # Has both add and delete - it's a modification
            name, file_path = key
            added = [c for c in group if c.change_type == "added"]
            deleted = [c for c in group if c.change_type == "deleted"]

            if added and deleted:
                final_changes.append(
                    SymbolChange(
                        name=name,
                        kind=group[0].kind,
                        change_type="modified",
                        file_path=file_path,
                    )
                )
                for c in group:
                    processed.add(id(c))

    # Add remaining non-paired changes
    for change in changes:
        if id(change) not in processed:
            final_changes.append(change)

    return final_changes


def generate_summary(analysis: DiffAnalysis) -> str:
    """Generate a human-readable summary of the diff analysis.

    Args:
        analysis: Diff analysis object

    Returns:
        Summary string
    """
    lines = []

    # File statistics
    lines.append(f"Files: {analysis.files_changed} changed")
    if analysis.files_added:
        lines.append(f"  {analysis.files_added} added")
    if analysis.files_deleted:
        lines.append(f"  {analysis.files_deleted} deleted")
    if analysis.files_renamed:
        lines.append(f"  {analysis.files_renamed} renamed")

    lines.append(f"Lines: +{analysis.total_additions} -{analysis.total_deletions}")

    # Symbol changes
    if analysis.symbol_changes:
        lines.append("")
        lines.append("Symbol changes:")

        added = [s for s in analysis.symbol_changes if s.change_type == "added"]
        modified = [s for s in analysis.symbol_changes if s.change_type == "modified"]
        deleted = [s for s in analysis.symbol_changes if s.change_type == "deleted"]

        if added:
            lines.append(f"  Added: {len(added)}")
            for s in added[:5]:  # Show first 5
                lines.append(f"    + {s.kind} {s.name}")
            if len(added) > 5:
                lines.append(f"    ... and {len(added) - 5} more")

        if modified:
            lines.append(f"  Modified: {len(modified)}")
            for s in modified[:5]:
                lines.append(f"    ~ {s.kind} {s.name}")
            if len(modified) > 5:
                lines.append(f"    ... and {len(modified) - 5} more")

        if deleted:
            lines.append(f"  Deleted: {len(deleted)}")
            for s in deleted[:5]:
                lines.append(f"    - {s.kind} {s.name}")
            if len(deleted) > 5:
                lines.append(f"    ... and {len(deleted) - 5} more")

    return "\n".join(lines)


def analyze_diff(diff_output: str) -> DiffAnalysis:
    """Analyze a git diff.

    Args:
        diff_output: Unified diff output

    Returns:
        DiffAnalysis object with full analysis
    """
    file_diffs = parse_diff(diff_output)
    symbol_changes = analyze_symbol_changes(file_diffs)

    analysis = DiffAnalysis(
        files_changed=len(file_diffs),
        files_added=sum(1 for f in file_diffs if f.status == "added"),
        files_deleted=sum(1 for f in file_diffs if f.status == "deleted"),
        files_renamed=sum(1 for f in file_diffs if f.status == "renamed"),
        total_additions=sum(f.additions for f in file_diffs),
        total_deletions=sum(f.deletions for f in file_diffs),
        file_diffs=file_diffs,
        symbol_changes=symbol_changes,
    )

    analysis.summary = generate_summary(analysis)

    return analysis


def analyze_commits(
    repo_path: Path,
    from_ref: str,
    to_ref: str = "HEAD",
) -> DiffAnalysis:
    """Analyze changes between two commits.

    Args:
        repo_path: Path to git repository
        from_ref: Starting commit reference
        to_ref: Ending commit reference

    Returns:
        DiffAnalysis object
    """
    diff = get_commit_diff(repo_path, from_ref, to_ref)
    return analyze_diff(diff)
