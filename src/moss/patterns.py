"""Architectural pattern detection in Python codebases.

Detects common patterns:
- Plugin systems (Protocol + Registry + implementations)
- Factory patterns (functions returning different types)
- Strategy patterns (interface + swappable implementations)
- Singleton patterns
- Coupling analysis (module dependencies)

Usage:
    from moss.patterns import PatternAnalyzer

    analyzer = PatternAnalyzer(project_root)
    results = analyzer.analyze()

    # Via CLI:
    # moss patterns [directory] [--pattern plugin,factory]
"""

from __future__ import annotations

import ast
import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


# =============================================================================
# Data Types
# =============================================================================


@dataclass
class PatternInstance:
    """A detected pattern instance in the codebase."""

    pattern_type: str  # plugin, factory, strategy, singleton, etc.
    name: str  # Pattern name or primary class/function
    file_path: str
    line_start: int
    line_end: int | None = None
    confidence: float = 1.0  # 0.0-1.0
    components: list[str] = field(default_factory=list)  # Related classes/functions
    description: str = ""
    suggestion: str | None = None  # Improvement suggestion


@dataclass
class CouplingInfo:
    """Coupling information for a module."""

    module: str
    imports_from: list[str] = field(default_factory=list)  # Modules this one imports
    imported_by: list[str] = field(default_factory=list)  # Modules that import this


@dataclass
class PatternAnalysis:
    """Results from pattern analysis."""

    root: Path
    patterns: list[PatternInstance] = field(default_factory=list)
    coupling: dict[str, CouplingInfo] = field(default_factory=dict)
    suggestions: list[str] = field(default_factory=list)

    @property
    def plugin_systems(self) -> list[PatternInstance]:
        return [p for p in self.patterns if p.pattern_type == "plugin"]

    @property
    def factories(self) -> list[PatternInstance]:
        return [p for p in self.patterns if p.pattern_type == "factory"]

    @property
    def strategies(self) -> list[PatternInstance]:
        return [p for p in self.patterns if p.pattern_type == "strategy"]

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "root": str(self.root),
            "summary": {
                "total_patterns": len(self.patterns),
                "plugin_systems": len(self.plugin_systems),
                "factories": len(self.factories),
                "strategies": len(self.strategies),
            },
            "patterns": [
                {
                    "type": p.pattern_type,
                    "name": p.name,
                    "file": p.file_path,
                    "line": p.line_start,
                    "confidence": p.confidence,
                    "components": p.components,
                    "description": p.description,
                    "suggestion": p.suggestion,
                }
                for p in self.patterns
            ],
            "suggestions": self.suggestions,
        }


# =============================================================================
# Pattern Detectors
# =============================================================================


class ProtocolDetector(ast.NodeVisitor):
    """Detect Protocol definitions and their implementations."""

    def __init__(self, source: str, file_path: str) -> None:
        self.source = source
        self.file_path = file_path
        self.protocols: list[dict[str, Any]] = []
        self.protocol_impls: list[dict[str, Any]] = []
        self.registries: list[dict[str, Any]] = []

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        # Check if this is a Protocol definition
        for base in node.bases:
            base_name = self._get_base_name(base)
            if base_name == "Protocol":
                self.protocols.append(
                    {
                        "name": node.name,
                        "line": node.lineno,
                        "end_line": node.end_lineno,
                        "methods": self._get_methods(node),
                    }
                )
            elif base_name in [p["name"] for p in self.protocols]:
                # This class implements a Protocol we found
                self.protocol_impls.append(
                    {
                        "name": node.name,
                        "protocol": base_name,
                        "line": node.lineno,
                    }
                )

        # Check for registry patterns (dict with type as key)
        for stmt in node.body:
            if isinstance(stmt, ast.AnnAssign) and stmt.annotation:
                annotation = ast.unparse(stmt.annotation)
                if "dict" in annotation.lower() and (
                    "type" in annotation.lower() or "str" in annotation.lower()
                ):
                    if stmt.target and isinstance(stmt.target, ast.Name):
                        name = stmt.target.id
                        registry_words = ["registry", "plugins", "handlers"]
                        if any(word in name.lower() for word in registry_words):
                            self.registries.append(
                                {
                                    "name": name,
                                    "class": node.name,
                                    "line": stmt.lineno,
                                }
                            )

        self.generic_visit(node)

    def _get_base_name(self, node: ast.expr) -> str:
        """Get the name from a base class expression."""
        if isinstance(node, ast.Name):
            return node.id
        if isinstance(node, ast.Attribute):
            return node.attr
        if isinstance(node, ast.Subscript):
            return self._get_base_name(node.value)
        return ""

    def _get_methods(self, node: ast.ClassDef) -> list[str]:
        """Get method names from a class."""
        methods = []
        for item in node.body:
            if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                if not item.name.startswith("_") or item.name.startswith("__"):
                    methods.append(item.name)
        return methods


class FactoryDetector(ast.NodeVisitor):
    """Detect factory patterns - functions that create different types."""

    def __init__(self, source: str, file_path: str) -> None:
        self.source = source
        self.file_path = file_path
        self.factories: list[dict[str, Any]] = []

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._check_factory(node)
        self.generic_visit(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._check_factory(node)
        self.generic_visit(node)

    def _check_factory(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        """Check if a function is a factory."""
        # Look for factory naming patterns
        name_hints = ["create", "make", "build", "get", "factory", "new"]
        has_factory_name = any(hint in node.name.lower() for hint in name_hints)

        # Look for conditional returns of different types
        return_types = self._find_return_types(node)

        if has_factory_name and len(return_types) > 1:
            self.factories.append(
                {
                    "name": node.name,
                    "line": node.lineno,
                    "end_line": node.end_lineno,
                    "return_types": return_types,
                }
            )
        elif len(return_types) >= 3:  # Multiple return types even without factory name
            self.factories.append(
                {
                    "name": node.name,
                    "line": node.lineno,
                    "end_line": node.end_lineno,
                    "return_types": return_types,
                }
            )

    def _find_return_types(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[str]:
        """Find all return statement types in a function."""
        types = set()

        class ReturnVisitor(ast.NodeVisitor):
            def visit_Return(self, ret_node: ast.Return) -> None:
                if ret_node.value:
                    if isinstance(ret_node.value, ast.Call):
                        if isinstance(ret_node.value.func, ast.Name):
                            types.add(ret_node.value.func.id)
                        elif isinstance(ret_node.value.func, ast.Attribute):
                            types.add(ret_node.value.func.attr)

        ReturnVisitor().visit(node)
        return list(types)


class SingletonDetector(ast.NodeVisitor):
    """Detect singleton patterns."""

    def __init__(self, source: str, file_path: str) -> None:
        self.source = source
        self.file_path = file_path
        self.singletons: list[dict[str, Any]] = []

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        # Look for classic singleton patterns
        has_instance_attr = False
        has_new_override = False

        for stmt in node.body:
            # Class-level _instance attribute
            if isinstance(stmt, ast.AnnAssign) and stmt.target:
                if isinstance(stmt.target, ast.Name) and "_instance" in stmt.target.id:
                    has_instance_attr = True
            elif isinstance(stmt, ast.Assign):
                for target in stmt.targets:
                    if isinstance(target, ast.Name) and "_instance" in target.id:
                        has_instance_attr = True

            # __new__ override
            if isinstance(stmt, ast.FunctionDef) and stmt.name == "__new__":
                has_new_override = True

        if has_instance_attr and has_new_override:
            self.singletons.append(
                {
                    "name": node.name,
                    "line": node.lineno,
                    "end_line": node.end_lineno,
                }
            )

        self.generic_visit(node)


class CouplingAnalyzer(ast.NodeVisitor):
    """Analyze module coupling via imports."""

    def __init__(self, source: str, module_name: str) -> None:
        self.source = source
        self.module_name = module_name
        self.imports: list[str] = []

    def visit_Import(self, node: ast.Import) -> None:
        for alias in node.names:
            self.imports.append(alias.name)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        if node.module:
            self.imports.append(node.module)


# =============================================================================
# Main Analyzer
# =============================================================================


class PatternAnalyzer:
    """Analyzes a codebase for architectural patterns."""

    def __init__(
        self,
        root: Path,
        patterns: list[str] | None = None,
    ) -> None:
        """Initialize the analyzer.

        Args:
            root: Project root directory
            patterns: List of patterns to detect (None = all)
        """
        self.root = Path(root).resolve()
        self.requested_patterns = patterns or ["plugin", "factory", "singleton", "coupling"]

    def analyze(self) -> PatternAnalysis:
        """Run pattern analysis on the codebase."""
        result = PatternAnalysis(root=self.root)

        # Find all Python files
        python_files = list(self.root.rglob("*.py"))
        exclude_parts = [".venv", "venv", "node_modules", ".git", "__pycache__", "dist", "build"]
        python_files = [
            f for f in python_files if not any(part in str(f) for part in exclude_parts)
        ]

        # First pass: collect all protocols
        all_protocols: list[dict[str, Any]] = []

        for file_path in python_files:
            try:
                source = file_path.read_text()
                tree = ast.parse(source)

                if "plugin" in self.requested_patterns:
                    detector = ProtocolDetector(source, str(file_path))
                    detector.visit(tree)
                    all_protocols.extend(detector.protocols)
            except Exception as e:
                logger.debug("Failed to parse %s: %s", file_path, e)

        # Second pass: analyze each file
        coupling_data: dict[str, list[str]] = {}

        for file_path in python_files:
            try:
                source = file_path.read_text()
                tree = ast.parse(source)
                rel_path = str(file_path.relative_to(self.root))

                # Plugin/Protocol detection
                if "plugin" in self.requested_patterns:
                    detector = ProtocolDetector(source, rel_path)
                    detector.visit(tree)

                    for protocol in detector.protocols:
                        result.patterns.append(
                            PatternInstance(
                                pattern_type="plugin",
                                name=protocol["name"],
                                file_path=rel_path,
                                line_start=protocol["line"],
                                line_end=protocol.get("end_line"),
                                components=protocol.get("methods", []),
                                description=f"{len(protocol.get('methods', []))} methods",
                            )
                        )

                    for registry in detector.registries:
                        result.patterns.append(
                            PatternInstance(
                                pattern_type="plugin",
                                name=f"{registry['class']}.{registry['name']}",
                                file_path=rel_path,
                                line_start=registry["line"],
                                description="Plugin registry",
                            )
                        )

                # Factory detection
                if "factory" in self.requested_patterns:
                    detector = FactoryDetector(source, rel_path)
                    detector.visit(tree)

                    for factory in detector.factories:
                        result.patterns.append(
                            PatternInstance(
                                pattern_type="factory",
                                name=factory["name"],
                                file_path=rel_path,
                                line_start=factory["line"],
                                line_end=factory.get("end_line"),
                                components=factory.get("return_types", []),
                                description=f"Creates {len(factory.get('return_types', []))} types",
                            )
                        )

                # Singleton detection
                if "singleton" in self.requested_patterns:
                    detector = SingletonDetector(source, rel_path)
                    detector.visit(tree)

                    for singleton in detector.singletons:
                        result.patterns.append(
                            PatternInstance(
                                pattern_type="singleton",
                                name=singleton["name"],
                                file_path=rel_path,
                                line_start=singleton["line"],
                                line_end=singleton.get("end_line"),
                                description="Singleton pattern with _instance + __new__",
                            )
                        )

                # Coupling analysis
                if "coupling" in self.requested_patterns:
                    module_name = rel_path.replace("/", ".").replace(".py", "")
                    analyzer = CouplingAnalyzer(source, module_name)
                    analyzer.visit(tree)
                    coupling_data[module_name] = analyzer.imports

            except Exception as e:
                logger.debug("Failed to analyze %s: %s", file_path, e)

        # Build coupling graph
        if "coupling" in self.requested_patterns:
            for module, imports in coupling_data.items():
                result.coupling[module] = CouplingInfo(
                    module=module,
                    imports_from=imports,
                )

            # Calculate imported_by (reverse edges)
            for module, info in result.coupling.items():
                for imported in info.imports_from:
                    if imported in result.coupling:
                        result.coupling[imported].imported_by.append(module)

            # Generate suggestions for highly coupled modules
            for module, info in result.coupling.items():
                if len(info.imported_by) > 10:
                    result.suggestions.append(
                        f"{module} is imported by {len(info.imported_by)} modules - "
                        "consider if it's doing too much"
                    )
                if len(info.imports_from) > 15:
                    result.suggestions.append(
                        f"{module} imports {len(info.imports_from)} modules - "
                        "may have too many dependencies"
                    )

        return result


def format_pattern_analysis(analysis: PatternAnalysis) -> str:
    """Format pattern analysis as markdown."""
    lines = ["## Pattern Analysis", ""]

    # Summary
    lines.append("### Summary")
    lines.append(f"- Plugin systems: {len(analysis.plugin_systems)}")
    lines.append(f"- Factories: {len(analysis.factories)}")
    lines.append(f"- Strategies: {len(analysis.strategies)}")
    lines.append(f"- Total patterns: {len(analysis.patterns)}")
    lines.append("")

    # Patterns by type
    if analysis.plugin_systems:
        lines.append("### Plugin Systems")
        for p in analysis.plugin_systems:
            lines.append(f"- **{p.name}** (`{p.file_path}:{p.line_start}`)")
            if p.description:
                lines.append(f"  {p.description}")
            if p.components:
                lines.append(f"  Components: {', '.join(p.components[:5])}")
        lines.append("")

    if analysis.factories:
        lines.append("### Factories")
        for p in analysis.factories:
            lines.append(f"- **{p.name}** (`{p.file_path}:{p.line_start}`)")
            if p.description:
                lines.append(f"  {p.description}")
            if p.components:
                lines.append(f"  Returns: {', '.join(p.components)}")
        lines.append("")

    # Suggestions
    if analysis.suggestions:
        lines.append("### Suggestions")
        for s in analysis.suggestions:
            lines.append(f"- {s}")
        lines.append("")

    return "\n".join(lines)


def analyze_patterns(
    root: Path | str,
    patterns: list[str] | None = None,
) -> PatternAnalysis:
    """Convenience function to analyze patterns.

    Args:
        root: Project root directory
        patterns: Patterns to detect (None = all)

    Returns:
        PatternAnalysis with detected patterns
    """
    analyzer = PatternAnalyzer(Path(root), patterns=patterns)
    return analyzer.analyze()
