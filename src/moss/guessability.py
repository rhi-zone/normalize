"""Guessability metrics for codebase structure quality.

Evaluates how intuitive and predictable a codebase's structure is.
Can you guess where to find functionality based on its name?

Key metrics:
- Name-content alignment: Do module names reflect their contents?
- Predictability: Are similar things in similar places?
- Discoverability: Can you find what you're looking for?
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class NameAlignmentScore:
    """Score for how well a module name aligns with its contents.

    Attributes:
        module_path: Path to the module
        module_name: Name of the module (without extension)
        top_symbols: Most prominent symbols in the module
        alignment_score: 0.0-1.0, how well name matches contents
        suggestions: Suggested better names if score is low
    """

    module_path: str
    module_name: str
    top_symbols: list[str]
    alignment_score: float
    suggestions: list[str] = field(default_factory=list)

    def to_compact(self) -> str:
        score_pct = f"{self.alignment_score * 100:.0f}%"
        syms = ", ".join(self.top_symbols[:3])
        if self.suggestions:
            return f"{self.module_name} ({score_pct}): {syms} -> suggest: {self.suggestions[0]}"
        return f"{self.module_name} ({score_pct}): {syms}"


@dataclass
class PredictabilityScore:
    """Score for structural predictability.

    Attributes:
        pattern: The pattern being evaluated (e.g., "test files")
        expected_location: Where you'd expect to find it
        actual_locations: Where things actually are
        consistency_score: 0.0-1.0, how consistent the pattern is
        violations: Files that break the pattern
    """

    pattern: str
    expected_location: str
    actual_locations: list[str]
    consistency_score: float
    violations: list[str] = field(default_factory=list)

    def to_compact(self) -> str:
        score_pct = f"{self.consistency_score * 100:.0f}%"
        if self.violations:
            viols = ", ".join(self.violations[:3])
            return f"{self.pattern} ({score_pct}): violations: {viols}"
        return f"{self.pattern} ({score_pct}): consistent"


@dataclass
class GuessabilityScore:
    """Summary score for codebase guessability.

    Attributes:
        score: Overall guessability (0.0-1.0)
        grade: Letter grade (A-F)
    """

    score: float
    grade: str

    def to_compact(self) -> str:
        return f"Guessability: {self.grade} ({self.score * 100:.0f}%)"


@dataclass
class GuessabilityReport:
    """Complete guessability analysis of a codebase.

    Attributes:
        name_scores: Alignment scores for each module
        predictability_scores: Pattern consistency scores
        overall_score: Aggregate guessability (0.0-1.0)
        grade: Letter grade (A-F)
        recommendations: Suggested improvements
    """

    name_scores: list[NameAlignmentScore]
    predictability_scores: list[PredictabilityScore]
    overall_score: float
    grade: str
    recommendations: list[str]

    def to_compact(self) -> str:
        lines = [
            f"Guessability: {self.grade} ({self.overall_score * 100:.0f}%)",
            "",
            "Name Alignment (lowest 5):",
        ]
        worst_names = sorted(self.name_scores, key=lambda x: x.alignment_score)[:5]
        for score in worst_names:
            lines.append(f"  {score.to_compact()}")

        lines.append("")
        lines.append("Pattern Consistency:")
        for score in self.predictability_scores:
            lines.append(f"  {score.to_compact()}")

        if self.recommendations:
            lines.append("")
            lines.append("Recommendations:")
            for rec in self.recommendations[:5]:
                lines.append(f"  - {rec}")

        return "\n".join(lines)


class GuessabilityAnalyzer:
    """Analyzes codebase guessability."""

    def __init__(self, root: Path):
        self.root = root

    def analyze(self) -> GuessabilityReport:
        """Run full guessability analysis."""
        name_scores = self._analyze_name_alignment()
        predictability_scores = self._analyze_predictability()

        # Calculate overall score
        if name_scores:
            avg_name_score = sum(s.alignment_score for s in name_scores) / len(name_scores)
        else:
            avg_name_score = 1.0

        if predictability_scores:
            avg_pred_score = sum(s.consistency_score for s in predictability_scores) / len(
                predictability_scores
            )
        else:
            avg_pred_score = 1.0

        overall = (avg_name_score + avg_pred_score) / 2

        # Grade
        if overall >= 0.9:
            grade = "A"
        elif overall >= 0.8:
            grade = "B"
        elif overall >= 0.7:
            grade = "C"
        elif overall >= 0.6:
            grade = "D"
        else:
            grade = "F"

        recommendations = self._generate_recommendations(name_scores, predictability_scores)

        return GuessabilityReport(
            name_scores=name_scores,
            predictability_scores=predictability_scores,
            overall_score=overall,
            grade=grade,
            recommendations=recommendations,
        )

    def _analyze_name_alignment(self) -> list[NameAlignmentScore]:
        """Analyze how well module names match their contents."""
        from moss_intelligence.skeleton import extract_python_skeleton

        scores = []

        for py_file in self.root.rglob("*.py"):
            # Skip tests, hidden, etc
            parts = py_file.parts
            skip_dirs = ("venv", "node_modules", "__pycache__", ".git", "tests")
            if any(p.startswith(".") or p in skip_dirs for p in parts):
                continue

            if py_file.name.startswith("test_"):
                continue

            try:
                source = py_file.read_text()
                symbols = extract_python_skeleton(source)
            except (OSError, UnicodeDecodeError, SyntaxError):
                continue

            if not symbols:
                continue

            module_name = py_file.stem
            if module_name == "__init__":
                module_name = py_file.parent.name

            # Get top symbols (public classes and functions)
            top_symbols = [
                s.name
                for s in symbols
                if not s.name.startswith("_") and s.kind in ("class", "function")
            ][:5]

            # Calculate alignment score
            alignment = self._calculate_name_alignment(module_name, top_symbols)

            # Generate suggestions if alignment is low
            suggestions = []
            if alignment < 0.5 and top_symbols:
                suggestions = self._suggest_names(top_symbols)

            rel_path = str(py_file.relative_to(self.root))
            scores.append(
                NameAlignmentScore(
                    module_path=rel_path,
                    module_name=module_name,
                    top_symbols=top_symbols,
                    alignment_score=alignment,
                    suggestions=suggestions,
                )
            )

        return scores

    def _calculate_name_alignment(self, module_name: str, symbols: list[str]) -> float:
        """Calculate how well module name aligns with its symbols."""
        if not symbols:
            return 1.0

        # Normalize module name
        module_words = set(self._split_name(module_name.lower()))

        # Check overlap with symbol names
        matches = 0
        for symbol in symbols:
            symbol_words = set(self._split_name(symbol.lower()))
            if module_words & symbol_words:
                matches += 1

        # Also check if module name appears in any symbol
        for symbol in symbols:
            if module_name.lower() in symbol.lower():
                matches += 0.5

        # Score based on matches
        return min(1.0, matches / max(1, len(symbols)))

    def _split_name(self, name: str) -> list[str]:
        """Split a name into words (handles snake_case and CamelCase)."""
        # Split on underscores
        parts = name.split("_")
        # Split CamelCase
        words = []
        for part in parts:
            # Split on capital letters
            words.extend(re.findall(r"[a-z]+|[A-Z][a-z]*", part))
        return [w.lower() for w in words if w]

    def _suggest_names(self, symbols: list[str]) -> list[str]:
        """Suggest module names based on symbols."""
        suggestions = []

        # Find common prefix
        if len(symbols) >= 2:
            words_lists = [self._split_name(s) for s in symbols]
            if words_lists and all(words_lists):
                common = words_lists[0][0] if words_lists[0] else None
                if common and all(words[0] == common for words in words_lists if words):
                    suggestions.append(common)

        # Use most prominent symbol
        if symbols:
            main_symbol = symbols[0]
            words = self._split_name(main_symbol)
            if words:
                suggestions.append("_".join(words[:2]))

        return suggestions[:2]

    def _analyze_predictability(self) -> list[PredictabilityScore]:
        """Analyze structural pattern consistency."""
        scores = []

        # Pattern: Tests in tests/ directory
        test_files = list(self.root.rglob("test_*.py"))
        # Filter out venv, node_modules, etc
        skip_dirs = (".venv", "venv", "node_modules", ".git", "__pycache__")
        test_files = [
            f for f in test_files if not any(p in skip_dirs or p.startswith(".") for p in f.parts)
        ]
        # Only consider files that actually contain test functions
        actual_test_files = []
        for f in test_files:
            try:
                content = f.read_text()
                if "def test_" in content or "@pytest" in content:
                    actual_test_files.append(f)
            except (OSError, UnicodeDecodeError):
                pass
        test_files = actual_test_files

        tests_in_tests_dir = [f for f in test_files if "tests" in f.parts]
        tests_outside = [f for f in test_files if "tests" not in f.parts]

        if test_files:
            consistency = len(tests_in_tests_dir) / len(test_files)
            scores.append(
                PredictabilityScore(
                    pattern="test files in tests/",
                    expected_location="tests/",
                    actual_locations=[str(f.relative_to(self.root)) for f in test_files[:5]],
                    consistency_score=consistency,
                    violations=[str(f.relative_to(self.root)) for f in tests_outside[:3]],
                )
            )

        # Pattern: __init__.py in packages
        # Only check directories that are likely meant to be packages:
        # - Under src/ directory
        # - Have multiple .py files (not just scripts)
        # - Not documentation/config directories
        non_package_names = {"docs", "scripts", "examples", "site", "build", "dist"}
        py_dirs = [
            d
            for d in self.root.rglob("*")
            if d.is_dir()
            and any(d.glob("*.py"))
            and not d.name.startswith(".")
            and d.name not in non_package_names
        ]
        skip_parts = ("venv", ".venv", "__pycache__", "node_modules", ".git")
        py_dirs = [d for d in py_dirs if not any(p in skip_parts for p in d.parts)]
        # Only flag dirs under src/ or with multiple .py files (likely packages)
        package_dirs = [d for d in py_dirs if "src" in d.parts or len(list(d.glob("*.py"))) > 1]

        if package_dirs:
            has_init = [d for d in package_dirs if (d / "__init__.py").exists()]
            missing_init = [d for d in package_dirs if not (d / "__init__.py").exists()]
            consistency = len(has_init) / len(package_dirs) if package_dirs else 1.0
            scores.append(
                PredictabilityScore(
                    pattern="__init__.py in packages",
                    expected_location="every package",
                    actual_locations=[str(d.relative_to(self.root)) for d in has_init[:5]],
                    consistency_score=consistency,
                    violations=[str(d.relative_to(self.root)) for d in missing_init[:3]],
                )
            )

        # Pattern: Consistent naming (snake_case for modules)
        py_files = list(self.root.rglob("*.py"))
        py_files = [
            f
            for f in py_files
            if not any(p.startswith(".") or p in ("venv", "__pycache__") for p in f.parts)
        ]

        snake_case = [f for f in py_files if re.match(r"^[a-z][a-z0-9_]*\.py$", f.name)]
        non_snake = [f for f in py_files if f not in snake_case and f.name != "__init__.py"]

        if py_files:
            init_count = len([f for f in py_files if f.name == "__init__.py"])
            consistency = (len(snake_case) + init_count) / len(py_files)
            scores.append(
                PredictabilityScore(
                    pattern="snake_case module names",
                    expected_location="all modules",
                    actual_locations=[],
                    consistency_score=consistency,
                    violations=[str(f.relative_to(self.root)) for f in non_snake[:3]],
                )
            )

        return scores

    def _generate_recommendations(
        self,
        name_scores: list[NameAlignmentScore],
        predictability_scores: list[PredictabilityScore],
    ) -> list[str]:
        """Generate actionable recommendations."""
        recommendations = []

        # Low name alignment
        low_alignment = [s for s in name_scores if s.alignment_score < 0.5]
        if low_alignment:
            worst = min(low_alignment, key=lambda x: x.alignment_score)
            if worst.suggestions:
                recommendations.append(
                    f"Rename '{worst.module_name}' to '{worst.suggestions[0]}' for clarity"
                )
            else:
                syms = worst.top_symbols[:3]
                recommendations.append(
                    f"Review '{worst.module_name}' - name doesn't match contents: {syms}"
                )

        # Low predictability
        for score in predictability_scores:
            if score.consistency_score < 0.8 and score.violations:
                recommendations.append(
                    f"Move {score.violations[0]} to follow '{score.pattern}' pattern"
                )

        # General recommendations
        avg_alignment = (
            sum(s.alignment_score for s in name_scores) / len(name_scores) if name_scores else 1.0
        )
        if avg_alignment < 0.6:
            recommendations.append(
                "Consider reorganizing modules - many names don't match contents"
            )

        return recommendations[:5]


def analyze_guessability(root: Path | str) -> GuessabilityReport:
    """Analyze codebase guessability.

    Args:
        root: Path to the project root

    Returns:
        GuessabilityReport with scores and recommendations
    """
    analyzer = GuessabilityAnalyzer(Path(root))
    return analyzer.analyze()
