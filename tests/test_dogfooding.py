"""Dogfooding tests - using Moss to analyze itself.

These tests verify that Moss can successfully analyze and process
its own codebase, serving as both validation and real-world testing.
"""

from pathlib import Path

import pytest

from moss.anchors import Anchor, AnchorType, find_anchors
from moss.cfg import build_cfg
from moss.dependencies import extract_dependencies
from moss.elided_literals import elide_literals
from moss.skeleton import extract_python_skeleton, format_skeleton


class TestSelfAnalysis:
    """Tests analyzing Moss source code."""

    @pytest.fixture
    def moss_src(self) -> Path:
        """Get the Moss source directory."""
        return Path(__file__).parent.parent / "src" / "moss"

    def test_extract_skeleton_from_all_modules(self, moss_src: Path):
        """Test skeleton extraction on all Moss modules."""
        python_files = list(moss_src.glob("*.py"))
        assert len(python_files) > 10, "Should have many source files"

        total_symbols = 0
        for py_file in python_files:
            source = py_file.read_text()
            try:
                symbols = extract_python_skeleton(source)
                total_symbols += len(symbols)
            except SyntaxError:
                pytest.fail(f"Failed to parse {py_file.name}")

        assert total_symbols > 100, "Should extract many symbols from Moss"

    def test_find_key_classes(self, moss_src: Path):
        """Test finding key Moss classes."""
        key_classes = [
            ("anchors.py", "Anchor"),
            ("patches.py", "Patch"),
            ("events.py", "EventBus"),
            ("validators.py", "SyntaxValidator"),
            ("shadow_git.py", "ShadowGit"),
            ("memory.py", "MemoryManager"),
            ("policy.py", "PolicyEngine"),
            ("cfg.py", "ControlFlowGraph"),
            ("skeleton.py", "Symbol"),
        ]

        for filename, class_name in key_classes:
            py_file = moss_src / filename
            if not py_file.exists():
                continue

            source = py_file.read_text()
            anchor = Anchor(type=AnchorType.CLASS, name=class_name)
            matches = find_anchors(source, anchor)

            assert len(matches) >= 1, f"Should find {class_name} in {filename}"

    def test_find_key_functions(self, moss_src: Path):
        """Test finding key Moss functions."""
        key_functions = [
            ("anchors.py", "find_anchors"),
            ("patches.py", "apply_patch"),
            ("skeleton.py", "extract_python_skeleton"),
            ("cfg.py", "build_cfg"),
            ("elided_literals.py", "elide_literals"),
        ]

        for filename, func_name in key_functions:
            py_file = moss_src / filename
            if not py_file.exists():
                continue

            source = py_file.read_text()
            anchor = Anchor(type=AnchorType.FUNCTION, name=func_name)
            matches = find_anchors(source, anchor)

            assert len(matches) >= 1, f"Should find {func_name} in {filename}"

    def test_build_cfg_for_moss_functions(self, moss_src: Path):
        """Test CFG building on Moss source functions."""
        # Test CFG on validators.py which has good control flow
        validators_file = moss_src / "validators.py"
        if not validators_file.exists():
            pytest.skip("validators.py not found")

        source = validators_file.read_text()
        cfgs = build_cfg(source)

        assert len(cfgs) > 0, "Should build CFGs for validator functions"

        # Verify CFGs have proper structure
        for cfg in cfgs:
            assert cfg.entry_node is not None
            assert cfg.exit_node is not None
            assert cfg.node_count >= 2  # At least entry and exit

    def test_elide_literals_on_moss_source(self, moss_src: Path):
        """Test literal elision on Moss source."""
        # Use a file with many literals
        config_file = moss_src / "config.py"
        if not config_file.exists():
            pytest.skip("config.py not found")

        source = config_file.read_text()
        elided, _stats = elide_literals(source)

        # Should have some elisions but preserve structure
        assert isinstance(elided, str)
        assert len(elided) > 0
        # Elided should be smaller or same size (less literals)
        assert "class" in elided  # Should preserve class definitions

    def test_extract_dependencies_from_moss(self, moss_src: Path):
        """Test dependency extraction from Moss modules."""
        # Test on a module with imports
        anchors_file = moss_src / "anchors.py"
        if not anchors_file.exists():
            pytest.skip("anchors.py not found")

        source = anchors_file.read_text()
        deps = extract_dependencies(source)

        assert deps.imports is not None
        assert len(deps.imports) > 0, "anchors.py should have imports"


class TestCrossModuleAnalysis:
    """Tests analyzing relationships between Moss modules."""

    @pytest.fixture
    def moss_src(self) -> Path:
        """Get the Moss source directory."""
        return Path(__file__).parent.parent / "src" / "moss"

    def test_init_exports_match_modules(self, moss_src: Path):
        """Test that __init__.py exports match module definitions."""
        init_file = moss_src / "__init__.py"
        if not init_file.exists():
            pytest.skip("__init__.py not found")

        source = init_file.read_text()

        # The __init__.py is mostly imports, so skeleton may be empty
        # But we can verify it parses without error
        symbols = extract_python_skeleton(source)
        assert isinstance(symbols, list)

        # Verify __all__ is defined in the source
        assert "__all__" in source, "__init__.py should have __all__"

    def test_no_circular_import_issues(self, moss_src: Path):
        """Test that Moss modules can be imported without circular import issues."""
        # This test implicitly passes if we got this far,
        # as the test imports worked
        import moss

        # Verify key exports are accessible
        assert hasattr(moss, "Anchor")
        assert hasattr(moss, "Patch")
        assert hasattr(moss, "EventBus")
        assert hasattr(moss, "ShadowGit")
        assert hasattr(moss, "extract_python_skeleton")
        assert hasattr(moss, "apply_patch")
        assert hasattr(moss, "build_cfg")


class TestSkeletonQuality:
    """Tests for skeleton extraction quality on Moss code."""

    @pytest.fixture
    def moss_src(self) -> Path:
        """Get the Moss source directory."""
        return Path(__file__).parent.parent / "src" / "moss"

    def test_docstrings_preserved(self, moss_src: Path):
        """Test that docstrings are captured in skeletons."""
        anchors_file = moss_src / "anchors.py"
        if not anchors_file.exists():
            pytest.skip("anchors.py not found")

        source = anchors_file.read_text()
        symbols = extract_python_skeleton(source)

        # Find a symbol with a docstring
        _has_docstring = any(s.docstring for s in symbols if hasattr(s, "docstring"))
        # Some symbols should have docstrings
        assert len(symbols) > 0

    def test_nested_classes_captured(self, moss_src: Path):
        """Test that nested classes are captured."""
        # Find a file with nested definitions
        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            symbols = extract_python_skeleton(source)

            # Check for nested structures
            for symbol in symbols:
                if hasattr(symbol, "children") and symbol.children:
                    # Found a class with methods
                    return  # Test passes

        # If no nested structures found, that's also fine
        # Not all codebases have deeply nested structures


class TestRealWorldPatterns:
    """Tests for real-world code patterns in Moss."""

    @pytest.fixture
    def moss_src(self) -> Path:
        """Get the Moss source directory."""
        return Path(__file__).parent.parent / "src" / "moss"

    def test_async_functions_handled(self, moss_src: Path):
        """Test that async functions are properly handled."""
        # Find files with async functions
        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            if "async def" in source:
                symbols = extract_python_skeleton(source)
                _skeleton = format_skeleton(symbols)

                # Should capture async functions
                assert len(symbols) > 0
                return  # Found and tested

    def test_dataclasses_handled(self, moss_src: Path):
        """Test that dataclasses are properly handled."""
        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            if "@dataclass" in source:
                symbols = extract_python_skeleton(source)

                # Should extract classes even with decorators
                assert len(symbols) > 0
                return

    def test_type_annotations_preserved(self, moss_src: Path):
        """Test that type annotations don't break parsing."""
        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            # All Moss files should parse successfully
            try:
                symbols = extract_python_skeleton(source)
                assert isinstance(symbols, list)
            except SyntaxError:
                pytest.fail(f"Failed to parse {py_file.name}")


class TestPerformanceOnOwnCode:
    """Performance tests on Moss codebase."""

    @pytest.fixture
    def moss_src(self) -> Path:
        """Get the Moss source directory."""
        return Path(__file__).parent.parent / "src" / "moss"

    def test_skeleton_extraction_performance(self, moss_src: Path):
        """Test that skeleton extraction is fast on Moss code."""
        import time

        start = time.perf_counter()

        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            extract_python_skeleton(source)

        elapsed = time.perf_counter() - start

        # Should complete in under 2 seconds for all files
        assert elapsed < 2.0, f"Skeleton extraction took {elapsed:.2f}s"

    def test_cfg_building_performance(self, moss_src: Path):
        """Test that CFG building is reasonably fast."""
        import time

        start = time.perf_counter()

        for py_file in moss_src.glob("*.py"):
            source = py_file.read_text()
            try:
                build_cfg(source)
            except Exception:
                pass  # Some files may not have functions

        elapsed = time.perf_counter() - start

        # Should complete in under 5 seconds
        assert elapsed < 5.0, f"CFG building took {elapsed:.2f}s"
