"""Tests for multi-file refactoring module."""

from pathlib import Path

import pytest


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    """Create a test workspace with Python files."""
    # Create a simple module structure
    (tmp_path / "main.py").write_text("""
from utils import helper_func

def main():
    result = helper_func()
    return result
""")

    (tmp_path / "utils.py").write_text('''
def helper_func():
    """A helper function."""
    return 42

def other_func():
    return helper_func() + 1
''')

    (tmp_path / "tests" / "__init__.py").parent.mkdir()
    (tmp_path / "tests" / "__init__.py").write_text("")
    (tmp_path / "tests" / "test_main.py").write_text("""
from main import main
from utils import helper_func

def test_main():
    assert main() == 42

def test_helper():
    assert helper_func() == 42
""")

    return tmp_path


class TestRefactoringScope:
    """Tests for RefactoringScope."""

    def test_scope_values(self):
        from moss.refactoring import RefactoringScope

        assert RefactoringScope.FILE.value == "file"
        assert RefactoringScope.DIRECTORY.value == "directory"
        assert RefactoringScope.WORKSPACE.value == "workspace"


class TestFileChange:
    """Tests for FileChange."""

    def test_create_file_change(self, tmp_path: Path):
        from moss.refactoring import FileChange

        path = tmp_path / "test.py"
        change = FileChange(
            path=path,
            original_content="old content",
            new_content="new content",
        )

        assert change.path == path
        assert change.has_changes is True

    def test_no_changes(self, tmp_path: Path):
        from moss.refactoring import FileChange

        path = tmp_path / "test.py"
        change = FileChange(
            path=path,
            original_content="same",
            new_content="same",
        )

        assert change.has_changes is False

    def test_to_diff(self, tmp_path: Path):
        from moss.refactoring import FileChange

        path = tmp_path / "test.py"
        change = FileChange(
            path=path,
            original_content="line1\nline2\n",
            new_content="line1\nmodified\n",
        )

        diff = change.to_diff()
        assert "-line2" in diff
        assert "+modified" in diff


class TestRefactoringResult:
    """Tests for RefactoringResult."""

    def test_success_result(self):
        from moss.refactoring import RefactoringResult

        result = RefactoringResult(success=True)

        assert result.success is True
        assert result.total_changes == 0
        assert result.errors == []

    def test_with_changes(self, tmp_path: Path):
        from moss.refactoring import FileChange, RefactoringResult

        change = FileChange(
            path=tmp_path / "test.py",
            original_content="old",
            new_content="new",
        )
        result = RefactoringResult(success=True, changes=[change])

        assert result.total_changes == 1


class TestRenameRefactoring:
    """Tests for RenameRefactoring."""

    def test_rename_function(self):
        from moss.refactoring import RenameRefactoring

        refactoring = RenameRefactoring(old_name="old_func", new_name="new_func")

        content = """
def old_func():
    return 1

result = old_func()
"""
        new_content = refactoring.apply_to_file(Path("test.py"), content)

        assert new_content is not None
        assert "new_func" in new_content
        assert "old_func" not in new_content

    def test_rename_class(self):
        from moss.refactoring import RenameRefactoring

        refactoring = RenameRefactoring(
            old_name="OldClass", new_name="NewClass", symbol_type="class"
        )

        content = """
class OldClass:
    pass

instance = OldClass()
"""
        new_content = refactoring.apply_to_file(Path("test.py"), content)

        assert new_content is not None
        assert "NewClass" in new_content
        assert "OldClass" not in new_content

    def test_rename_preserves_other_code(self):
        from moss.refactoring import RenameRefactoring

        refactoring = RenameRefactoring(old_name="target", new_name="renamed")

        content = """
def target():
    return 1

def other():
    return 2
"""
        new_content = refactoring.apply_to_file(Path("test.py"), content)

        assert new_content is not None
        assert "renamed" in new_content
        assert "other" in new_content


class TestMoveRefactoring:
    """Tests for MoveRefactoring."""

    def test_update_imports(self, tmp_path: Path):
        from moss.refactoring import MoveRefactoring

        refactoring = MoveRefactoring(
            source_file=Path("old_module.py"),
            target_file=Path("new_module.py"),
            symbol_name="my_func",
        )

        content = "from old_module import my_func\n"
        new_content = refactoring.apply_to_file(tmp_path / "test.py", content)

        assert new_content is not None
        assert "new_module" in new_content
        assert "old_module" not in new_content


class TestExtractRefactoring:
    """Tests for ExtractRefactoring."""

    def test_extract_simple(self):
        from moss.refactoring import ExtractRefactoring

        refactoring = ExtractRefactoring(
            start_line=3,
            end_line=4,
            new_name="extracted",
        )

        content = """def main():
    x = 1
    y = 2
    z = x + y
    return z
"""
        new_content = refactoring.apply_to_file(Path("test.py"), content)

        assert new_content is not None
        assert "def extracted" in new_content


class TestRefactorer:
    """Tests for Refactorer."""

    def test_create_refactorer(self, workspace: Path):
        from moss.refactoring import Refactorer

        refactorer = Refactorer(workspace)

        assert refactorer.workspace == workspace

    @pytest.mark.asyncio
    async def test_apply_rename(self, workspace: Path):
        from moss.refactoring import Refactorer, RefactoringScope, RenameRefactoring

        refactorer = Refactorer(workspace)
        refactoring = RenameRefactoring(
            old_name="helper_func",
            new_name="renamed_helper",
            scope=RefactoringScope.WORKSPACE,
        )

        result = await refactorer.apply(refactoring, dry_run=True)

        assert result.success is True
        # Should affect multiple files
        assert len(result.changes) > 0

    def test_preview(self, workspace: Path):
        from moss.refactoring import Refactorer, RefactoringScope, RenameRefactoring

        refactorer = Refactorer(workspace)
        refactoring = RenameRefactoring(
            old_name="helper_func",
            new_name="renamed_helper",
            scope=RefactoringScope.WORKSPACE,
        )

        refactorer.preview(refactoring)

        # Dry run - files should not be modified
        utils_content = (workspace / "utils.py").read_text()
        assert "helper_func" in utils_content
        assert "renamed_helper" not in utils_content

    def test_generate_diff(self, workspace: Path):
        from moss.refactoring import Refactorer, RefactoringScope, RenameRefactoring

        refactorer = Refactorer(workspace)
        refactoring = RenameRefactoring(
            old_name="helper_func",
            new_name="renamed_helper",
            scope=RefactoringScope.WORKSPACE,
        )

        result = refactorer.preview(refactoring)
        diff = refactorer.generate_diff(result)

        # The diff contains the changes
        assert "renamed_helper" in diff
        assert len(diff) > 0


class TestRenameSymbol:
    """Tests for rename_symbol convenience function."""

    @pytest.mark.asyncio
    async def test_rename_in_workspace(self, workspace: Path):
        from moss.refactoring import rename_symbol

        result = await rename_symbol(
            workspace=workspace,
            old_name="helper_func",
            new_name="new_helper",
            dry_run=True,
        )

        assert result.success is True


class TestMoveSymbol:
    """Tests for move_symbol convenience function."""

    @pytest.mark.asyncio
    async def test_move_updates_imports(self, workspace: Path):
        from moss.refactoring import move_symbol

        result = await move_symbol(
            workspace=workspace,
            symbol_name="helper_func",
            source_file=workspace / "utils.py",
            target_file=workspace / "helpers.py",
            dry_run=True,
        )

        assert result.success is True


class TestAnalyzeVariables:
    """Tests for variable analysis."""

    def test_analyze_used_variables(self):
        from moss.refactoring import _analyze_used_variables

        code = "result = x + y"
        used = _analyze_used_variables(code)

        assert "x" in used
        assert "y" in used
        assert "result" not in used  # assigned, not used

    def test_analyze_assigned_variables(self):
        from moss.refactoring import _analyze_assigned_variables

        code = "x = 1\ny = 2"
        assigned = _analyze_assigned_variables(code)

        assert "x" in assigned
        assert "y" in assigned


class TestPathToModule:
    """Tests for path to module conversion."""

    def test_simple_path(self):
        from moss.refactoring import _path_to_module

        path = Path("utils.py")
        module = _path_to_module(path)

        assert module == "utils"

    def test_nested_path(self):
        from moss.refactoring import _path_to_module

        path = Path("package/subpackage/module.py")
        module = _path_to_module(path)

        assert module == "package.subpackage.module"
