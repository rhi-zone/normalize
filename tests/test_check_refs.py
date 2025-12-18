"""Tests for the bidirectional reference checking module."""

from datetime import datetime
from pathlib import Path

from moss.check_refs import (
    CodeReference,
    DocReference,
    RefChecker,
    RefCheckResult,
    StaleReference,
    create_ref_checker,
)

# =============================================================================
# CodeReference Tests
# =============================================================================


class TestCodeReference:
    def test_create_reference(self):
        ref = CodeReference(
            source_file=Path("src/foo.py"),
            source_line=10,
            target_doc=Path("docs/spec.md"),
            raw_text="# See: docs/spec.md",
        )
        assert ref.source_file == Path("src/foo.py")
        assert ref.source_line == 10
        assert ref.target_doc == Path("docs/spec.md")
        assert ref.raw_text == "# See: docs/spec.md"

    def test_to_dict(self):
        ref = CodeReference(
            source_file=Path("src/foo.py"),
            source_line=10,
            target_doc=Path("docs/spec.md"),
            raw_text="# See: docs/spec.md",
        )
        d = ref.to_dict()
        assert d["source_file"] == "src/foo.py"
        assert d["source_line"] == 10
        assert d["target_doc"] == "docs/spec.md"
        assert d["raw_text"] == "# See: docs/spec.md"


# =============================================================================
# DocReference Tests
# =============================================================================


class TestDocReference:
    def test_create_reference(self):
        ref = DocReference(
            source_doc=Path("docs/spec.md"),
            source_line=25,
            target_file=Path("src/foo.py"),
            raw_text="`src/foo.py`",
        )
        assert ref.source_doc == Path("docs/spec.md")
        assert ref.source_line == 25
        assert ref.target_file == Path("src/foo.py")
        assert ref.raw_text == "`src/foo.py`"

    def test_to_dict(self):
        ref = DocReference(
            source_doc=Path("docs/spec.md"),
            source_line=25,
            target_file=Path("src/foo.py"),
            raw_text="`src/foo.py`",
        )
        d = ref.to_dict()
        assert d["source_doc"] == "docs/spec.md"
        assert d["source_line"] == 25
        assert d["target_file"] == "src/foo.py"
        assert d["raw_text"] == "`src/foo.py`"


# =============================================================================
# StaleReference Tests
# =============================================================================


class TestStaleReference:
    def test_create_stale_reference(self):
        ref = StaleReference(
            source_path=Path("src/foo.py"),
            target_path=Path("docs/spec.md"),
            source_mtime=datetime(2024, 1, 1),
            target_mtime=datetime(2024, 1, 15),
            reference_line=10,
            is_code_to_doc=True,
        )
        assert ref.source_path == Path("src/foo.py")
        assert ref.target_path == Path("docs/spec.md")
        assert ref.is_code_to_doc is True

    def test_staleness_days(self):
        ref = StaleReference(
            source_path=Path("src/foo.py"),
            target_path=Path("docs/spec.md"),
            source_mtime=datetime(2024, 1, 1),
            target_mtime=datetime(2024, 1, 15),
            reference_line=10,
            is_code_to_doc=True,
        )
        assert ref.staleness_days == 14

    def test_staleness_days_zero(self):
        ref = StaleReference(
            source_path=Path("src/foo.py"),
            target_path=Path("docs/spec.md"),
            source_mtime=datetime(2024, 1, 15),
            target_mtime=datetime(2024, 1, 1),  # target older than source
            reference_line=10,
            is_code_to_doc=True,
        )
        # staleness_days returns max(0, delta.days), should be 0
        assert ref.staleness_days == 0

    def test_to_dict(self):
        ref = StaleReference(
            source_path=Path("src/foo.py"),
            target_path=Path("docs/spec.md"),
            source_mtime=datetime(2024, 1, 1),
            target_mtime=datetime(2024, 1, 15),
            reference_line=10,
            is_code_to_doc=True,
        )
        d = ref.to_dict()
        assert d["source_path"] == "src/foo.py"
        assert d["target_path"] == "docs/spec.md"
        assert d["staleness_days"] == 14
        assert d["reference_line"] == 10
        assert d["direction"] == "code_to_doc"

    def test_to_dict_doc_to_code(self):
        ref = StaleReference(
            source_path=Path("docs/spec.md"),
            target_path=Path("src/foo.py"),
            source_mtime=datetime(2024, 1, 1),
            target_mtime=datetime(2024, 1, 15),
            reference_line=10,
            is_code_to_doc=False,
        )
        d = ref.to_dict()
        assert d["direction"] == "doc_to_code"


# =============================================================================
# RefCheckResult Tests
# =============================================================================


class TestRefCheckResult:
    def test_empty_result(self):
        result = RefCheckResult()
        assert not result.has_errors
        assert not result.has_warnings
        assert result.error_count == 0
        assert result.warning_count == 0

    def test_has_errors_code_to_docs_broken(self):
        result = RefCheckResult(
            code_to_docs_broken=[
                CodeReference(
                    source_file=Path("src/foo.py"),
                    source_line=10,
                    target_doc=Path("docs/missing.md"),
                    raw_text="# See: docs/missing.md",
                )
            ]
        )
        assert result.has_errors
        assert result.error_count == 1

    def test_has_errors_docs_to_code_broken(self):
        result = RefCheckResult(
            docs_to_code_broken=[
                DocReference(
                    source_doc=Path("docs/spec.md"),
                    source_line=10,
                    target_file=Path("src/missing.py"),
                    raw_text="`src/missing.py`",
                )
            ]
        )
        assert result.has_errors
        assert result.error_count == 1

    def test_has_warnings_stale(self):
        result = RefCheckResult(
            stale_references=[
                StaleReference(
                    source_path=Path("src/foo.py"),
                    target_path=Path("docs/spec.md"),
                    source_mtime=datetime(2024, 1, 1),
                    target_mtime=datetime(2024, 2, 1),
                    reference_line=10,
                    is_code_to_doc=True,
                )
            ]
        )
        assert result.has_warnings
        assert result.warning_count == 1

    def test_to_dict(self):
        result = RefCheckResult(
            code_files_checked=5,
            doc_files_checked=3,
            code_to_docs=[
                CodeReference(
                    source_file=Path("src/foo.py"),
                    source_line=10,
                    target_doc=Path("docs/spec.md"),
                    raw_text="# See: docs/spec.md",
                )
            ],
        )
        d = result.to_dict()
        assert d["stats"]["code_files_checked"] == 5
        assert d["stats"]["doc_files_checked"] == 3
        assert d["stats"]["code_to_docs_valid"] == 1
        assert d["stats"]["errors"] == 0

    def test_to_markdown_no_issues(self):
        result = RefCheckResult(
            code_files_checked=5,
            doc_files_checked=3,
        )
        md = result.to_markdown()
        assert "Reference Check Results" in md
        assert "All references are valid and up-to-date" in md

    def test_to_markdown_broken_code_to_doc(self):
        result = RefCheckResult(
            code_to_docs_broken=[
                CodeReference(
                    source_file=Path("src/foo.py"),
                    source_line=10,
                    target_doc=Path("docs/missing.md"),
                    raw_text="# See: docs/missing.md",
                )
            ]
        )
        md = result.to_markdown()
        assert "Broken Code -> Doc References" in md
        assert "src/foo.py:10" in md
        assert "docs/missing.md" in md

    def test_to_markdown_stale_references(self):
        result = RefCheckResult(
            stale_references=[
                StaleReference(
                    source_path=Path("src/foo.py"),
                    target_path=Path("docs/spec.md"),
                    source_mtime=datetime(2024, 1, 1),
                    target_mtime=datetime(2024, 2, 1),
                    reference_line=10,
                    is_code_to_doc=True,
                )
            ]
        )
        md = result.to_markdown()
        assert "Stale References" in md
        assert "src/foo.py" in md
        assert "docs/spec.md" in md


# =============================================================================
# RefChecker Tests
# =============================================================================


class TestRefChecker:
    def test_create_checker(self, tmp_path: Path):
        checker = RefChecker(tmp_path)
        assert checker.root == tmp_path

    def test_is_valid_doc_path_valid(self, tmp_path: Path):
        checker = RefChecker(tmp_path)
        assert checker._is_valid_doc_path("docs/spec.md")
        assert checker._is_valid_doc_path("docs/design/architecture.md")

    def test_is_valid_doc_path_invalid(self, tmp_path: Path):
        checker = RefChecker(tmp_path)
        assert not checker._is_valid_doc_path("")
        assert not checker._is_valid_doc_path("src/foo.py")
        assert not checker._is_valid_doc_path("docs/foo.txt")
        assert not checker._is_valid_doc_path("docs/<script>.md")

    def test_is_valid_code_path_valid(self, tmp_path: Path):
        checker = RefChecker(tmp_path)
        assert checker._is_valid_code_path("src/foo.py")
        assert checker._is_valid_code_path("src/moss/cli.py")

    def test_is_valid_code_path_invalid(self, tmp_path: Path):
        checker = RefChecker(tmp_path)
        assert not checker._is_valid_code_path("")
        assert not checker._is_valid_code_path("docs/spec.md")
        assert not checker._is_valid_code_path("src/foo.js")
        assert not checker._is_valid_code_path("src/<script>.py")

    def test_find_code_files(self, tmp_path: Path):
        # Create src directory with Python files
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "foo.py").write_text("# code")
        (tmp_path / "src" / "bar.py").write_text("# code")
        (tmp_path / "root.py").write_text("# root code")

        checker = RefChecker(tmp_path)
        files = checker._find_code_files()
        assert len(files) == 3
        assert tmp_path / "src" / "foo.py" in files

    def test_find_doc_files(self, tmp_path: Path):
        # Create docs directory with markdown files
        (tmp_path / "docs").mkdir()
        (tmp_path / "docs" / "spec.md").write_text("# Spec")
        (tmp_path / "docs" / "design.md").write_text("# Design")
        (tmp_path / "README.md").write_text("# README")

        checker = RefChecker(tmp_path)
        files = checker._find_doc_files()
        assert tmp_path / "docs" / "spec.md" in files
        assert tmp_path / "README.md" in files

    def test_extract_code_to_doc_refs_see(self, tmp_path: Path):
        # Create a code file with doc references
        (tmp_path / "src").mkdir()
        code_file = tmp_path / "src" / "foo.py"
        code_file.write_text('"""Module.\n\n# See: docs/spec.md\n"""\n')

        checker = RefChecker(tmp_path)
        refs = checker._extract_code_to_doc_refs(code_file)
        assert len(refs) == 1
        assert refs[0].target_doc == Path("docs/spec.md")

    def test_extract_code_to_doc_refs_ref(self, tmp_path: Path):
        # Test # Ref: pattern
        (tmp_path / "src").mkdir()
        code_file = tmp_path / "src" / "foo.py"
        code_file.write_text("# Ref: docs/design.md\n")

        checker = RefChecker(tmp_path)
        refs = checker._extract_code_to_doc_refs(code_file)
        assert len(refs) == 1
        assert refs[0].target_doc == Path("docs/design.md")

    def test_extract_code_to_doc_refs_quoted(self, tmp_path: Path):
        # Test quoted path pattern
        (tmp_path / "src").mkdir()
        code_file = tmp_path / "src" / "foo.py"
        code_file.write_text('path = "docs/usage.md"\n')

        checker = RefChecker(tmp_path)
        refs = checker._extract_code_to_doc_refs(code_file)
        assert len(refs) == 1
        assert refs[0].target_doc == Path("docs/usage.md")

    def test_extract_doc_to_code_refs_backtick(self, tmp_path: Path):
        # Create a doc file with code references
        (tmp_path / "docs").mkdir()
        doc_file = tmp_path / "docs" / "spec.md"
        doc_file.write_text("See `src/moss/cli.py` for details.\n")

        checker = RefChecker(tmp_path)
        refs = checker._extract_doc_to_code_refs(doc_file)
        # Liberal matching may find multiple patterns for the same path
        assert len(refs) >= 1
        assert any(r.target_file == Path("src/moss/cli.py") for r in refs)

    def test_extract_doc_to_code_refs_html_comment(self, tmp_path: Path):
        # Test HTML comment pattern
        (tmp_path / "docs").mkdir()
        doc_file = tmp_path / "docs" / "spec.md"
        doc_file.write_text("<!-- Implementation: src/foo.py -->\n")

        checker = RefChecker(tmp_path)
        refs = checker._extract_doc_to_code_refs(doc_file)
        # Liberal matching may find multiple patterns for the same path
        assert len(refs) >= 1
        assert any(r.target_file == Path("src/foo.py") for r in refs)

    def test_extract_doc_to_code_refs_markdown_link(self, tmp_path: Path):
        # Test markdown link pattern
        (tmp_path / "docs").mkdir()
        doc_file = tmp_path / "docs" / "spec.md"
        doc_file.write_text("[source](src/bar.py)\n")

        checker = RefChecker(tmp_path)
        refs = checker._extract_doc_to_code_refs(doc_file)
        assert len(refs) >= 1
        assert any(r.target_file == Path("src/bar.py") for r in refs)

    def test_check_valid_references(self, tmp_path: Path):
        # Create matching code and docs
        (tmp_path / "src").mkdir()
        (tmp_path / "docs").mkdir()

        code_file = tmp_path / "src" / "foo.py"
        code_file.write_text("# See: docs/spec.md\n")

        doc_file = tmp_path / "docs" / "spec.md"
        doc_file.write_text("See `src/foo.py` for implementation.\n")

        checker = RefChecker(tmp_path)
        result = checker.check()

        assert not result.has_errors
        assert len(result.code_to_docs) >= 1
        # Liberal matching may find multiple patterns for the same path
        assert len(result.docs_to_code) >= 1

    def test_check_broken_code_to_doc(self, tmp_path: Path):
        # Create code referencing non-existent doc
        (tmp_path / "src").mkdir()
        code_file = tmp_path / "src" / "foo.py"
        code_file.write_text("# See: docs/missing.md\n")

        checker = RefChecker(tmp_path)
        result = checker.check()

        assert result.has_errors
        assert len(result.code_to_docs_broken) == 1
        assert result.code_to_docs_broken[0].target_doc == Path("docs/missing.md")

    def test_check_broken_doc_to_code(self, tmp_path: Path):
        # Create doc referencing non-existent code
        (tmp_path / "docs").mkdir()
        doc_file = tmp_path / "docs" / "spec.md"
        doc_file.write_text("See `src/missing.py` for details.\n")

        checker = RefChecker(tmp_path)
        result = checker.check()

        assert result.has_errors
        # Liberal matching may find multiple patterns for the same path
        assert len(result.docs_to_code_broken) >= 1
        assert any(r.target_file == Path("src/missing.py") for r in result.docs_to_code_broken)


# =============================================================================
# Factory Function Tests
# =============================================================================


class TestCreateRefChecker:
    def test_create_with_default_root(self, monkeypatch, tmp_path: Path):
        monkeypatch.chdir(tmp_path)
        checker = create_ref_checker()
        assert checker.root == tmp_path

    def test_create_with_explicit_root(self, tmp_path: Path):
        checker = create_ref_checker(root=tmp_path)
        assert checker.root == tmp_path

    def test_create_with_custom_staleness(self, tmp_path: Path):
        checker = create_ref_checker(root=tmp_path, staleness_days=60)
        assert checker.staleness_days == 60
