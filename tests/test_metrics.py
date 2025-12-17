"""Tests for metrics module."""

from pathlib import Path

import pytest

from moss.metrics import (
    CodebaseMetrics,
    FileMetrics,
    ModuleMetrics,
    analyze_file,
    collect_metrics,
    generate_dashboard,
)


class TestFileMetrics:
    """Tests for FileMetrics dataclass."""

    def test_default_values(self):
        metrics = FileMetrics(path=Path("test.py"))

        assert metrics.path == Path("test.py")
        assert metrics.lines == 0
        assert metrics.code_lines == 0
        assert metrics.classes == 0
        assert metrics.functions == 0

    def test_with_values(self):
        metrics = FileMetrics(
            path=Path("test.py"),
            lines=100,
            code_lines=80,
            comment_lines=10,
            blank_lines=10,
            classes=2,
            functions=5,
            methods=8,
        )

        assert metrics.lines == 100
        assert metrics.code_lines == 80


class TestModuleMetrics:
    """Tests for ModuleMetrics dataclass."""

    def test_default_values(self):
        metrics = ModuleMetrics(name="mymodule")

        assert metrics.name == "mymodule"
        assert metrics.file_count == 0
        assert metrics.total_lines == 0


class TestCodebaseMetrics:
    """Tests for CodebaseMetrics dataclass."""

    def test_default_values(self):
        metrics = CodebaseMetrics()

        assert metrics.total_files == 0
        assert metrics.total_lines == 0
        assert metrics.files == []
        assert metrics.modules == []

    def test_to_dict(self):
        metrics = CodebaseMetrics(
            total_files=5,
            total_lines=500,
            total_code_lines=400,
            total_classes=10,
            total_functions=30,
            avg_file_lines=100.0,
            timestamp="2024-01-01T00:00:00",
            root_path="/path/to/project",
        )

        d = metrics.to_dict()

        assert d["total_files"] == 5
        assert d["total_lines"] == 500
        assert d["total_classes"] == 10
        assert d["timestamp"] == "2024-01-01T00:00:00"


class TestAnalyzeFile:
    """Tests for analyze_file function."""

    def test_analyze_simple_file(self, tmp_path: Path):
        test_file = tmp_path / "simple.py"
        test_file.write_text("""# This is a comment
import os

def hello():
    '''A docstring'''
    print("hello")

class MyClass:
    def method(self):
        pass
""")

        metrics = analyze_file(test_file)

        assert metrics.lines >= 10  # Depends on trailing newlines
        assert metrics.functions == 1
        assert metrics.classes == 1
        assert metrics.methods == 1
        assert metrics.imports == 1
        assert metrics.comment_lines >= 1  # At least the # comment

    def test_analyze_file_with_docstrings(self, tmp_path: Path):
        test_file = tmp_path / "docstring.py"
        test_file.write_text('''"""Module docstring."""

def foo():
    """Function docstring."""
    pass
''')

        metrics = analyze_file(test_file)

        assert metrics.comment_lines >= 2  # Module + function docstrings
        assert metrics.functions == 1

    def test_analyze_nonexistent_file(self, tmp_path: Path):
        metrics = analyze_file(tmp_path / "nonexistent.py")

        assert metrics.lines == 0
        assert metrics.code_lines == 0

    def test_analyze_file_with_syntax_error(self, tmp_path: Path):
        test_file = tmp_path / "broken.py"
        test_file.write_text("def broken(\n")

        # Should not raise, just return metrics without symbols
        metrics = analyze_file(test_file)

        assert metrics.lines == 1

    def test_complexity_counting(self, tmp_path: Path):
        test_file = tmp_path / "complex.py"
        test_file.write_text("""def complex_func(x):
    if x > 0:
        for i in range(x):
            if i % 2 == 0:
                print(i)
    elif x < 0:
        while x < 0:
            x += 1
    else:
        pass
""")

        metrics = analyze_file(test_file)

        # Should detect: if, for, if, elif, while
        assert metrics.complexity >= 4


class TestCollectMetrics:
    """Tests for collect_metrics function."""

    @pytest.fixture
    def sample_project(self, tmp_path: Path) -> Path:
        """Create a sample project structure."""
        # Create src directory
        src = tmp_path / "src" / "mypackage"
        src.mkdir(parents=True)

        # Create module files
        (src / "__init__.py").write_text('"""Package init."""\n')
        (src / "core.py").write_text("""'''Core module.'''
import os
import sys

class CoreClass:
    '''Main class.'''

    def method_one(self):
        pass

    def method_two(self):
        if True:
            pass

def helper():
    '''Helper function.'''
    return 42
""")

        (src / "utils.py").write_text("""'''Utilities.'''

def util_func():
    pass
""")

        # Create tests directory
        tests = tmp_path / "tests"
        tests.mkdir()
        (tests / "test_core.py").write_text("""'''Tests for core.'''
import pytest

def test_helper():
    assert True
""")

        return tmp_path

    def test_collect_all_files(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        assert metrics.total_files >= 4  # At least our 4 files

    def test_collect_with_pattern(self, sample_project: Path):
        metrics = collect_metrics(sample_project, pattern="**/test_*.py")

        assert metrics.total_files == 1

    def test_aggregates_totals(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        assert metrics.total_lines > 0
        assert metrics.total_code_lines > 0
        assert metrics.total_classes >= 1
        assert metrics.total_functions >= 2

    def test_calculates_averages(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        assert metrics.avg_file_lines > 0

    def test_groups_by_module(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        module_names = [m.name for m in metrics.modules]
        assert "mypackage" in module_names or "src" in module_names

    def test_records_timestamp(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        assert metrics.timestamp != ""
        assert "T" in metrics.timestamp  # ISO format

    def test_records_root_path(self, sample_project: Path):
        metrics = collect_metrics(sample_project)

        assert str(sample_project) in metrics.root_path


class TestGenerateDashboard:
    """Tests for generate_dashboard function."""

    def test_generates_html(self):
        metrics = CodebaseMetrics(
            total_files=10,
            total_lines=1000,
            total_code_lines=800,
            total_comment_lines=100,
            total_blank_lines=100,
            total_classes=5,
            total_functions=20,
            avg_file_lines=100.0,
            timestamp="2024-01-01T00:00:00",
            root_path="/project",
        )

        html = generate_dashboard(metrics)

        assert "<!DOCTYPE html>" in html
        assert "</html>" in html

    def test_includes_metrics_values(self):
        metrics = CodebaseMetrics(
            total_files=42,
            total_lines=5000,
            total_code_lines=4000,
            timestamp="2024-01-01T00:00:00",
            root_path="/project",
        )

        html = generate_dashboard(metrics)

        assert "42" in html  # total files
        assert "5,000" in html or "5000" in html  # total lines
        assert "4,000" in html or "4000" in html  # code lines

    def test_custom_title(self):
        metrics = CodebaseMetrics()

        html = generate_dashboard(metrics, title="My Project Dashboard")

        assert "My Project Dashboard" in html

    def test_escapes_html_in_title(self):
        metrics = CodebaseMetrics()

        html = generate_dashboard(metrics, title="Test <script>alert('xss')</script>")

        assert "<script>" not in html
        assert "&lt;script&gt;" in html

    def test_includes_modules_table(self):
        metrics = CodebaseMetrics(
            modules=[
                ModuleMetrics(name="core", file_count=5, total_lines=500),
                ModuleMetrics(name="utils", file_count=3, total_lines=200),
            ],
        )

        html = generate_dashboard(metrics)

        assert "core" in html
        assert "utils" in html

    def test_includes_largest_files(self):
        metrics = CodebaseMetrics(
            files=[
                FileMetrics(path=Path("/project/big.py"), lines=500),
                FileMetrics(path=Path("/project/small.py"), lines=50),
            ],
            root_path="/project",
        )

        html = generate_dashboard(metrics)

        assert "big.py" in html
        assert "small.py" in html

    def test_includes_chart(self):
        metrics = CodebaseMetrics(
            files=[
                FileMetrics(path=Path("a.py"), lines=30),
                FileMetrics(path=Path("b.py"), lines=80),
                FileMetrics(path=Path("c.py"), lines=150),
            ],
        )

        html = generate_dashboard(metrics)

        # Chart should have size ranges
        assert "0-50" in html
        assert "51-100" in html
