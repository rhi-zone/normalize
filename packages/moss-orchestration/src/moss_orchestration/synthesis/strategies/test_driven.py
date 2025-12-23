"""Test-driven decomposition strategy.

Decomposes problems based on test analysis, using test structure
to identify subproblems and their relationships.
"""

from __future__ import annotations

import re
from collections import Counter
from dataclasses import dataclass, field

from moss_orchestration.synthesis.strategy import DecompositionStrategy, StrategyMetadata
from moss_orchestration.synthesis.types import Context, Specification, Subproblem


@dataclass
class ExtractedTestCase:
    """Information extracted from a test case.

    Named without 'Test' prefix to avoid pytest collection warnings.
    """

    name: str
    description: str = ""
    operations: list[str] = field(default_factory=list)
    inputs: list[str] = field(default_factory=list)
    expected_outputs: list[str] = field(default_factory=list)
    category: str = ""


def extract_test_info(test: str | dict) -> ExtractedTestCase:
    """Extract information from a test case.

    Handles both string (test code) and dict (structured test) formats.
    """
    if isinstance(test, dict):
        return ExtractedTestCase(
            name=test.get("name", "unknown"),
            description=test.get("description", ""),
            operations=test.get("operations", []),
            inputs=[str(i) for i in test.get("inputs", [])],
            expected_outputs=[str(o) for o in test.get("expected", [])],
            category=test.get("category", ""),
        )

    # Parse test code string
    test_str = str(test)

    # Extract test name
    name_match = re.search(r"def (test_\w+)", test_str)
    name = name_match.group(1) if name_match else "unknown"

    # Extract operations (function calls)
    operations = re.findall(r"(\w+)\s*\(", test_str)
    operations = [op for op in operations if op not in ("def", "assert", "test")]

    # Extract assert patterns for expected outputs
    expected = re.findall(r"assert\s+.*?==\s*([^\n]+)", test_str)

    # Categorize test
    category = categorize_test(name, test_str)

    return ExtractedTestCase(
        name=name,
        description=f"Test: {name}",
        operations=operations,
        expected_outputs=expected,
        category=category,
    )


def categorize_test(name: str, content: str) -> str:
    """Categorize a test based on its name and content."""
    name_lower = name.lower()
    content_lower = content.lower()

    if "error" in name_lower or "exception" in name_lower or "raise" in content_lower:
        return "error_handling"
    if "valid" in name_lower or "invalid" in name_lower:
        return "validation"
    if "empty" in name_lower or "none" in name_lower:
        return "edge_case"
    if "success" in name_lower or "happy" in name_lower:
        return "happy_path"
    return "general"


def cluster_tests(tests: list[ExtractedTestCase]) -> dict[str, list[ExtractedTestCase]]:
    """Group tests by what they exercise."""
    clusters: dict[str, list[ExtractedTestCase]] = {}

    for test in tests:
        # Cluster by category
        if test.category not in clusters:
            clusters[test.category] = []
        clusters[test.category].append(test)

        # Also cluster by primary operation
        if test.operations:
            primary_op = test.operations[0]
            op_key = f"op:{primary_op}"
            if op_key not in clusters:
                clusters[op_key] = []
            clusters[op_key].append(test)

    return clusters


class TestDrivenDecomposition(DecompositionStrategy):
    """Decompose based on test analysis and coverage.

    This strategy works best when:
    - Comprehensive tests are available
    - Tests exercise different aspects/components
    - Tests are well-structured and isolated

    Decomposition approaches:
    1. Cluster tests by category (error handling, validation, etc.)
    2. Cluster tests by operations they exercise
    3. Extract subproblems from test patterns
    """

    @property
    def metadata(self) -> StrategyMetadata:
        return StrategyMetadata(
            name="test_driven",
            description="Decompose based on test analysis and coverage",
            keywords=(
                "test",
                "testing",
                "tdd",
                "test-driven",
                "coverage",
                "pytest",
                "unittest",
            ),
        )

    def can_handle(self, spec: Specification, context: Context) -> bool:
        """Check if we have tests to analyze."""
        return len(spec.tests) > 0

    def decompose(
        self,
        spec: Specification,
        context: Context,
    ) -> list[Subproblem]:
        """Decompose based on test analysis."""
        if not spec.tests:
            return []

        # Extract test info
        test_infos = [extract_test_info(t) for t in spec.tests]

        # Cluster tests
        clusters = cluster_tests(test_infos)

        # Generate subproblems from clusters
        subproblems: list[Subproblem] = []

        # Priority 1: Happy path tests
        if "happy_path" in clusters:
            sub_spec = self._spec_from_tests(
                "Implement core functionality",
                clusters["happy_path"],
                spec,
            )
            subproblems.append(Subproblem(specification=sub_spec, priority=0))

        # Priority 2: Validation tests
        if "validation" in clusters:
            sub_spec = self._spec_from_tests(
                "Implement input validation",
                clusters["validation"],
                spec,
            )
            subproblems.append(
                Subproblem(
                    specification=sub_spec,
                    dependencies=(0,) if subproblems else (),
                    priority=1,
                )
            )

        # Priority 3: Error handling tests
        if "error_handling" in clusters:
            sub_spec = self._spec_from_tests(
                "Implement error handling",
                clusters["error_handling"],
                spec,
            )
            subproblems.append(
                Subproblem(
                    specification=sub_spec,
                    dependencies=tuple(range(len(subproblems))),
                    priority=2,
                )
            )

        # Priority 4: Edge cases
        if "edge_case" in clusters:
            sub_spec = self._spec_from_tests(
                "Handle edge cases",
                clusters["edge_case"],
                spec,
            )
            subproblems.append(
                Subproblem(
                    specification=sub_spec,
                    dependencies=tuple(range(len(subproblems))),
                    priority=3,
                )
            )

        # Add operation-based subproblems
        for key, tests in clusters.items():
            if key.startswith("op:") and len(tests) >= 2:
                op_name = key[3:]
                sub_spec = self._spec_from_tests(
                    f"Implement {op_name} operation",
                    tests,
                    spec,
                )
                subproblems.append(
                    Subproblem(
                        specification=sub_spec,
                        priority=len(subproblems),
                    )
                )

        return subproblems

    def estimate_success(self, spec: Specification, context: Context) -> float:
        """Estimate based on test quality."""
        if not spec.tests:
            return 0.0

        score = 0.3  # Base score for having tests

        num_tests = len(spec.tests)

        # More tests = higher confidence
        if num_tests > 10:
            score += 0.2
        elif num_tests > 5:
            score += 0.15
        elif num_tests > 2:
            score += 0.1

        # Analyze test variety
        test_infos = [extract_test_info(t) for t in spec.tests]
        categories = {t.category for t in test_infos}

        # More categories = better coverage
        if len(categories) >= 3:
            score += 0.2
        elif len(categories) >= 2:
            score += 0.1

        # Check for operation variety
        all_ops: list[str] = []
        for t in test_infos:
            all_ops.extend(t.operations)
        unique_ops = len(set(all_ops))

        if unique_ops >= 5:
            score += 0.2
        elif unique_ops >= 3:
            score += 0.1

        return min(1.0, score)

    def _spec_from_tests(
        self,
        description: str,
        tests: list[ExtractedTestCase],
        parent_spec: Specification,
    ) -> Specification:
        """Create a specification from a group of tests."""
        # Collect operations from tests
        all_ops = Counter[str]()
        for test in tests:
            all_ops.update(test.operations)

        # Build description with test info
        test_names = [t.name for t in tests[:5]]  # Limit to 5
        desc = f"{description} (tests: {', '.join(test_names)})"

        return Specification(
            description=desc,
            type_signature=parent_spec.type_signature,
            tests=tuple(tests),
            constraints=parent_spec.constraints,
        )
