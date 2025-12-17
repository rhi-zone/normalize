"""Tests for diff analysis module."""

from pathlib import Path

import pytest

from moss.diff_analysis import (
    DiffAnalysis,
    FileDiff,
    SymbolChange,
    analyze_diff,
    analyze_symbol_changes,
    generate_summary,
    parse_diff,
)


class TestFileDiff:
    """Tests for FileDiff dataclass."""

    def test_default_values(self):
        diff = FileDiff(path=Path("test.py"))

        assert diff.path == Path("test.py")
        assert diff.old_path is None
        assert diff.status == "modified"
        assert diff.additions == 0
        assert diff.deletions == 0
        assert diff.hunks == []

    def test_renamed_file(self):
        diff = FileDiff(
            path=Path("new.py"),
            old_path=Path("old.py"),
            status="renamed",
        )

        assert diff.old_path == Path("old.py")
        assert diff.status == "renamed"


class TestSymbolChange:
    """Tests for SymbolChange dataclass."""

    def test_function_change(self):
        change = SymbolChange(
            name="my_func",
            kind="function",
            change_type="added",
            file_path=Path("test.py"),
        )

        assert change.name == "my_func"
        assert change.kind == "function"
        assert change.change_type == "added"

    def test_class_change(self):
        change = SymbolChange(
            name="MyClass",
            kind="class",
            change_type="deleted",
            file_path=Path("test.py"),
        )

        assert change.kind == "class"
        assert change.change_type == "deleted"


class TestDiffAnalysis:
    """Tests for DiffAnalysis dataclass."""

    def test_default_values(self):
        analysis = DiffAnalysis()

        assert analysis.files_changed == 0
        assert analysis.files_added == 0
        assert analysis.files_deleted == 0
        assert analysis.total_additions == 0
        assert analysis.total_deletions == 0
        assert analysis.file_diffs == []
        assert analysis.symbol_changes == []

    def test_to_dict(self):
        analysis = DiffAnalysis(
            files_changed=2,
            total_additions=10,
            total_deletions=5,
            file_diffs=[
                FileDiff(path=Path("a.py"), status="modified", additions=5, deletions=3),
                FileDiff(path=Path("b.py"), status="added", additions=5, deletions=2),
            ],
            symbol_changes=[
                SymbolChange(
                    name="foo",
                    kind="function",
                    change_type="added",
                    file_path=Path("a.py"),
                ),
            ],
            summary="test summary",
        )

        d = analysis.to_dict()

        assert d["files_changed"] == 2
        assert d["total_additions"] == 10
        assert d["total_deletions"] == 5
        assert len(d["files"]) == 2
        assert d["files"][0]["path"] == "a.py"
        assert d["files"][0]["status"] == "modified"
        assert len(d["symbol_changes"]) == 1
        assert d["symbol_changes"][0]["name"] == "foo"
        assert d["summary"] == "test summary"


class TestParseDiff:
    """Tests for parse_diff function."""

    def test_empty_diff(self):
        result = parse_diff("")
        assert result == []

    def test_single_file_modified(self):
        diff = """diff --git a/foo.py b/foo.py
index 1234567..abcdefg 100644
--- a/foo.py
+++ b/foo.py
@@ -1,3 +1,4 @@
 def hello():
     print("hello")
+    print("world")
"""
        result = parse_diff(diff)

        assert len(result) == 1
        assert result[0].path == Path("foo.py")
        assert result[0].status == "modified"
        assert result[0].additions == 1
        assert result[0].deletions == 0

    def test_new_file(self):
        diff = """diff --git a/new.py b/new.py
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/new.py
@@ -0,0 +1,3 @@
+def new_func():
+    pass
+
"""
        result = parse_diff(diff)

        assert len(result) == 1
        assert result[0].path == Path("new.py")
        assert result[0].status == "added"
        assert result[0].additions == 3
        assert result[0].deletions == 0

    def test_deleted_file(self):
        diff = """diff --git a/old.py b/old.py
deleted file mode 100644
index 1234567..0000000
--- a/old.py
+++ /dev/null
@@ -1,2 +0,0 @@
-def old_func():
-    pass
"""
        result = parse_diff(diff)

        assert len(result) == 1
        assert result[0].path == Path("old.py")
        assert result[0].status == "deleted"
        assert result[0].additions == 0
        assert result[0].deletions == 2

    def test_renamed_file(self):
        diff = """diff --git a/old_name.py b/new_name.py
similarity index 90%
rename from old_name.py
rename to new_name.py
index 1234567..abcdefg 100644
--- a/old_name.py
+++ b/new_name.py
@@ -1,2 +1,2 @@
 def func():
-    pass
+    return 42
"""
        result = parse_diff(diff)

        assert len(result) == 1
        assert result[0].path == Path("new_name.py")
        assert result[0].old_path == Path("old_name.py")
        assert result[0].status == "renamed"

    def test_multiple_files(self):
        diff = """diff --git a/a.py b/a.py
index 1234567..abcdefg 100644
--- a/a.py
+++ b/a.py
@@ -1,2 +1,3 @@
 def a():
     pass
+    # comment
diff --git a/b.py b/b.py
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/b.py
@@ -0,0 +1,2 @@
+def b():
+    pass
"""
        result = parse_diff(diff)

        assert len(result) == 2
        assert result[0].path == Path("a.py")
        assert result[1].path == Path("b.py")
        assert result[1].status == "added"

    def test_multiple_hunks(self):
        diff = """diff --git a/foo.py b/foo.py
index 1234567..abcdefg 100644
--- a/foo.py
+++ b/foo.py
@@ -1,3 +1,4 @@
 def hello():
     print("hello")
+    print("world")
@@ -10,3 +11,4 @@
 def goodbye():
     print("goodbye")
+    print("world")
"""
        result = parse_diff(diff)

        assert len(result) == 1
        assert len(result[0].hunks) == 2
        assert result[0].additions == 2


class TestAnalyzeSymbolChanges:
    """Tests for analyze_symbol_changes function."""

    def test_empty_diffs(self):
        result = analyze_symbol_changes([])
        assert result == []

    def test_non_python_file_ignored(self):
        diffs = [
            FileDiff(
                path=Path("readme.md"),
                hunks=["@@ -1 +1 @@\n-old\n+new"],
            )
        ]
        result = analyze_symbol_changes(diffs)
        assert result == []

    def test_added_function(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=["@@ -0,0 +1,2 @@\n+def new_func():\n+    pass"],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 1
        assert result[0].name == "new_func"
        assert result[0].kind == "function"
        assert result[0].change_type == "added"

    def test_deleted_function(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=["@@ -1,2 +0,0 @@\n-def old_func():\n-    pass"],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 1
        assert result[0].name == "old_func"
        assert result[0].kind == "function"
        assert result[0].change_type == "deleted"

    def test_modified_function(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=[
                    "@@ -1,2 +1,3 @@\n-def my_func():\n+def my_func():\n     pass\n+    return 1"
                ],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 1
        assert result[0].name == "my_func"
        assert result[0].change_type == "modified"

    def test_added_class(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=["@@ -0,0 +1,2 @@\n+class MyClass:\n+    pass"],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 1
        assert result[0].name == "MyClass"
        assert result[0].kind == "class"
        assert result[0].change_type == "added"

    def test_added_method(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=["@@ -1,2 +1,4 @@\n class Foo:\n+    def new_method(self):\n+        pass"],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 1
        assert result[0].name == "new_method"
        assert result[0].kind == "method"
        assert result[0].change_type == "added"

    def test_multiple_changes(self):
        diffs = [
            FileDiff(
                path=Path("test.py"),
                hunks=["@@ -0,0 +1,4 @@\n+def func_a():\n+    pass\n+def func_b():\n+    pass"],
            )
        ]
        result = analyze_symbol_changes(diffs)

        assert len(result) == 2
        names = {c.name for c in result}
        assert names == {"func_a", "func_b"}


class TestGenerateSummary:
    """Tests for generate_summary function."""

    def test_empty_analysis(self):
        analysis = DiffAnalysis()
        summary = generate_summary(analysis)

        assert "Files: 0 changed" in summary
        assert "Lines: +0 -0" in summary

    def test_with_file_changes(self):
        analysis = DiffAnalysis(
            files_changed=3,
            files_added=1,
            files_deleted=1,
            total_additions=15,
            total_deletions=8,
        )
        summary = generate_summary(analysis)

        assert "Files: 3 changed" in summary
        assert "1 added" in summary
        assert "1 deleted" in summary
        assert "Lines: +15 -8" in summary

    def test_with_symbol_changes(self):
        analysis = DiffAnalysis(
            files_changed=1,
            total_additions=10,
            symbol_changes=[
                SymbolChange(
                    name="new_func",
                    kind="function",
                    change_type="added",
                    file_path=Path("test.py"),
                ),
                SymbolChange(
                    name="MyClass",
                    kind="class",
                    change_type="modified",
                    file_path=Path("test.py"),
                ),
            ],
        )
        summary = generate_summary(analysis)

        assert "Symbol changes:" in summary
        assert "Added: 1" in summary
        assert "+ function new_func" in summary
        assert "Modified: 1" in summary
        assert "~ class MyClass" in summary

    def test_truncates_long_lists(self):
        analysis = DiffAnalysis(
            files_changed=1,
            symbol_changes=[
                SymbolChange(
                    name=f"func_{i}",
                    kind="function",
                    change_type="added",
                    file_path=Path("test.py"),
                )
                for i in range(10)
            ],
        )
        summary = generate_summary(analysis)

        assert "... and 5 more" in summary


class TestAnalyzeDiff:
    """Tests for analyze_diff function."""

    def test_empty_diff(self):
        analysis = analyze_diff("")

        assert analysis.files_changed == 0
        assert analysis.total_additions == 0
        assert analysis.total_deletions == 0
        assert analysis.summary != ""

    def test_full_analysis(self):
        diff = """diff --git a/test.py b/test.py
index 1234567..abcdefg 100644
--- a/test.py
+++ b/test.py
@@ -0,0 +1,3 @@
+def new_function():
+    return 42
+
"""
        analysis = analyze_diff(diff)

        assert analysis.files_changed == 1
        assert analysis.total_additions == 3
        assert len(analysis.file_diffs) == 1
        assert len(analysis.symbol_changes) == 1
        assert analysis.symbol_changes[0].name == "new_function"

    def test_analysis_counts_file_types(self):
        diff = """diff --git a/new.py b/new.py
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/new.py
@@ -0,0 +1 @@
+x = 1
diff --git a/old.py b/old.py
deleted file mode 100644
index 1234567..0000000
--- a/old.py
+++ /dev/null
@@ -1 +0,0 @@
-y = 2
diff --git a/mod.py b/mod.py
index 1234567..abcdefg 100644
--- a/mod.py
+++ b/mod.py
@@ -1 +1 @@
-z = 3
+z = 4
"""
        analysis = analyze_diff(diff)

        assert analysis.files_changed == 3
        assert analysis.files_added == 1
        assert analysis.files_deleted == 1


class TestGitIntegration:
    """Integration tests with actual git commands."""

    @pytest.fixture
    def git_repo(self, tmp_path: Path):
        """Create a temporary git repository."""
        import subprocess

        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create initial commit
        (tmp_path / "initial.py").write_text("# initial\n")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        return tmp_path

    def test_get_commit_diff(self, git_repo: Path):
        """Test getting diff between commits."""
        import subprocess

        from moss.diff_analysis import get_commit_diff

        # Make a change
        (git_repo / "new.py").write_text("def hello():\n    pass\n")
        subprocess.run(["git", "add", "."], cwd=git_repo, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "add new"],
            cwd=git_repo,
            capture_output=True,
        )

        diff = get_commit_diff(git_repo, "HEAD~1", "HEAD")

        assert "new.py" in diff
        assert "+def hello():" in diff

    def test_get_staged_diff(self, git_repo: Path):
        """Test getting staged diff."""
        import subprocess

        from moss.diff_analysis import get_staged_diff

        # Stage a change
        (git_repo / "staged.py").write_text("x = 1\n")
        subprocess.run(["git", "add", "."], cwd=git_repo, capture_output=True)

        diff = get_staged_diff(git_repo)

        assert "staged.py" in diff
        assert "+x = 1" in diff

    def test_get_working_diff(self, git_repo: Path):
        """Test getting working directory diff."""
        from moss.diff_analysis import get_working_diff

        # Modify tracked file without staging
        (git_repo / "initial.py").write_text("# modified\n")

        diff = get_working_diff(git_repo)

        assert "initial.py" in diff
        assert "-# initial" in diff
        assert "+# modified" in diff

    def test_analyze_commits(self, git_repo: Path):
        """Test full commit analysis."""
        import subprocess

        from moss.diff_analysis import analyze_commits

        # Make some changes
        (git_repo / "feature.py").write_text("def feature():\n    return 42\n")
        subprocess.run(["git", "add", "."], cwd=git_repo, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "add feature"],
            cwd=git_repo,
            capture_output=True,
        )

        analysis = analyze_commits(git_repo, "HEAD~1", "HEAD")

        assert analysis.files_changed == 1
        assert analysis.files_added == 1
        assert len(analysis.symbol_changes) == 1
        assert analysis.symbol_changes[0].name == "feature"
