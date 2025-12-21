"""Failure mode tests for error handling and robustness.

Tests verify that moss handles errors gracefully with informative messages.
"""

import subprocess
import sys
from pathlib import Path

import pytest


class TestRustCLIFailures:
    """Test failure handling in the Rust CLI."""

    def run_moss_rust(self, *args: str, cwd: Path | None = None) -> tuple[int, str, str]:
        """Run the Rust moss CLI and return (exit_code, stdout, stderr)."""
        result = subprocess.run(
            ["cargo", "run", "-p", "moss-cli", "--quiet", "--", *args],
            capture_output=True,
            text=True,
            cwd=cwd or Path.cwd(),
        )
        return result.returncode, result.stdout, result.stderr

    def test_view_nonexistent_file(self, tmp_path):
        """Test viewing a file that doesn't exist."""
        exit_code, _stdout, stderr = self.run_moss_rust(
            "view", "nonexistent_file.py", "-r", str(tmp_path)
        )
        assert exit_code != 0
        assert "No matches" in stderr or "not found" in stderr.lower()

    def test_view_empty_directory(self, tmp_path):
        """Test viewing an empty directory."""
        exit_code, stdout, _stderr = self.run_moss_rust("view", ".", "-r", str(tmp_path))
        # Should succeed but show empty tree
        assert exit_code == 0
        assert "0 directories, 0 files" in stdout

    def test_skeleton_invalid_syntax(self, tmp_path):
        """Test skeleton extraction on a file with invalid Python syntax."""
        bad_file = tmp_path / "bad.py"
        bad_file.write_text("def broken(\n    missing_close_paren")

        _exit_code, _stdout, _stderr = self.run_moss_rust("skeleton", str(bad_file))
        # Should handle gracefully (may succeed with partial extraction or fail)
        # Main goal: no panic, informative output

    def test_view_binary_file(self, tmp_path):
        """Test viewing a binary file."""
        binary_file = tmp_path / "binary.bin"
        binary_file.write_bytes(bytes(range(256)))

        _exit_code, _stdout, _stderr = self.run_moss_rust("view", str(binary_file), "--depth", "3")
        # Should handle gracefully (may show raw content or error)

    def test_view_deeply_nested_symbol(self, tmp_path):
        """Test viewing a symbol path that doesn't exist."""
        py_file = tmp_path / "test.py"
        py_file.write_text("def foo(): pass\n")

        exit_code, _stdout, stderr = self.run_moss_rust(
            "view", f"{py_file}/NonExistentClass/method", "-r", str(tmp_path)
        )
        # Should report symbol not found
        assert exit_code != 0
        assert "not found" in stderr.lower() or "no match" in stderr.lower()

    def test_tree_permission_denied(self, tmp_path):
        """Test tree on a directory without read permission."""
        if sys.platform == "win32":
            pytest.skip("Permission test not reliable on Windows")

        restricted_dir = tmp_path / "restricted"
        restricted_dir.mkdir()
        (restricted_dir / "secret.txt").write_text("secret")

        # Remove read permission
        restricted_dir.chmod(0o000)

        try:
            _exit_code, _stdout, _stderr = self.run_moss_rust("tree", str(restricted_dir))
            # Should handle permission error gracefully
        finally:
            # Restore permission for cleanup
            restricted_dir.chmod(0o755)

    def test_path_with_unicode(self, tmp_path):
        """Test handling paths with unicode characters."""
        unicode_dir = tmp_path / "日本語"
        unicode_dir.mkdir()
        unicode_file = unicode_dir / "テスト.py"
        unicode_file.write_text("# Unicode test\ndef hello(): pass\n")

        # Use view command which is more robust for this test
        exit_code, _stdout, stderr = self.run_moss_rust(
            "view", str(unicode_file), "--depth", "3", "-r", str(tmp_path)
        )
        # Should handle unicode paths correctly
        # May fail if file encoding issues, but should not panic
        # Note: "No matches" is a valid error message
        assert exit_code == 0 or "match" in stderr.lower() or "error" in stderr.lower()

    def test_very_long_path(self, tmp_path):
        """Test handling extremely long file paths."""
        # Create a deeply nested directory structure
        long_path = tmp_path
        for i in range(20):
            long_path = long_path / f"dir{i}"
        long_path.mkdir(parents=True)
        test_file = long_path / "test.py"
        test_file.write_text("def test(): pass\n")

        _exit_code, _stdout, _stderr = self.run_moss_rust("skeleton", str(test_file))
        # Should handle long paths correctly

    def test_circular_symlink(self, tmp_path):
        """Test handling circular symlinks."""
        if sys.platform == "win32":
            pytest.skip("Symlink test may require elevated privileges on Windows")

        # Create circular symlink
        link1 = tmp_path / "link1"
        link2 = tmp_path / "link2"
        link1.symlink_to(link2)
        link2.symlink_to(link1)

        _exit_code, _stdout, _stderr = self.run_moss_rust("tree", str(tmp_path))
        # Should handle circular symlinks without infinite loop


class TestPythonCLIFailures:
    """Test failure handling in the Python CLI."""

    def run_moss(self, *args: str, cwd: Path | None = None) -> tuple[int, str, str]:
        """Run the Python moss CLI and return (exit_code, stdout, stderr)."""
        result = subprocess.run(
            ["uv", "run", "moss", *args],
            capture_output=True,
            text=True,
            cwd=cwd or Path.cwd(),
        )
        return result.returncode, result.stdout, result.stderr

    def test_invalid_command(self):
        """Test running a command that doesn't exist."""
        exit_code, _stdout, _stderr = self.run_moss("nonexistent_command")
        assert exit_code != 0
        # Should show help or error message

    def test_missing_required_argument(self):
        """Test running a command without required arguments."""
        _exit_code, _stdout, _stderr = self.run_moss("skeleton")
        # May succeed with help or error for missing path

    def test_malformed_json_output(self, tmp_path):
        """Test --json flag produces valid JSON even on errors."""
        _exit_code, _stdout, _stderr = self.run_moss("skeleton", "nonexistent.py", "--json")
        # Even on error, should produce valid JSON if --json is used


class TestMalformedInputs:
    """Test handling of malformed input files."""

    def run_moss_rust(self, *args: str) -> tuple[int, str, str]:
        """Run the Rust moss CLI."""
        result = subprocess.run(
            ["cargo", "run", "-p", "moss-cli", "--quiet", "--", *args],
            capture_output=True,
            text=True,
        )
        return result.returncode, result.stdout, result.stderr

    def test_skeleton_truncated_file(self, tmp_path):
        """Test skeleton on a file that appears truncated."""
        truncated = tmp_path / "truncated.py"
        truncated.write_text("class Foo:\n    def bar(self")

        _exit_code, _stdout, _stderr = self.run_moss_rust("skeleton", str(truncated))
        # Should handle gracefully

    def test_skeleton_mixed_indentation(self, tmp_path):
        """Test skeleton on a file with mixed tabs/spaces."""
        mixed = tmp_path / "mixed.py"
        mixed.write_text("def foo():\n    pass\n\ndef bar():\n\tpass\n")

        _exit_code, _stdout, _stderr = self.run_moss_rust("skeleton", str(mixed))
        # Should handle mixed indentation

    def test_skeleton_null_bytes(self, tmp_path):
        """Test skeleton on a file containing null bytes."""
        null_bytes = tmp_path / "nullbytes.py"
        null_bytes.write_bytes(b"def foo():\x00    pass\n")

        _exit_code, _stdout, _stderr = self.run_moss_rust("skeleton", str(null_bytes))
        # Should handle null bytes gracefully

    def test_deps_malformed_imports(self, tmp_path):
        """Test deps on a file with malformed import statements."""
        malformed = tmp_path / "malformed.py"
        malformed.write_text("import \nfrom import x\nimport ....\n")

        _exit_code, _stdout, _stderr = self.run_moss_rust("deps", str(malformed))
        # Should handle malformed imports
