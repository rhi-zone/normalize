"""Tests for CLI interface."""

import subprocess
from pathlib import Path

import pytest

from moss.cli import (
    cmd_config,
    cmd_distros,
    cmd_init,
    cmd_run,
    cmd_status,
    create_parser,
    main,
)


class TestCreateParser:
    """Tests for create_parser."""

    def test_creates_parser(self):
        parser = create_parser()
        assert parser is not None
        assert parser.prog == "moss"

    def test_has_version(self):
        parser = create_parser()
        # Version action exists
        assert any(action.option_strings == ["--version"] for action in parser._actions)

    def test_has_subcommands(self):
        parser = create_parser()
        # Check subparsers exist
        subparsers_action = next((a for a in parser._actions if hasattr(a, "_parser_class")), None)
        assert subparsers_action is not None
        assert "init" in subparsers_action.choices
        assert "run" in subparsers_action.choices
        assert "status" in subparsers_action.choices
        assert "config" in subparsers_action.choices
        assert "distros" in subparsers_action.choices


class TestMain:
    """Tests for main entry point."""

    def test_no_command_shows_help(self, capsys):
        result = main([])
        assert result == 0
        captured = capsys.readouterr()
        assert "usage:" in captured.out.lower()

    def test_version(self):
        with pytest.raises(SystemExit) as exc:
            main(["--version"])
        assert exc.value.code == 0


class TestCmdInit:
    """Tests for init command."""

    def test_creates_config_file(self, tmp_path: Path):
        args = create_parser().parse_args(["init", str(tmp_path)])
        result = cmd_init(args)

        assert result == 0
        config_file = tmp_path / "moss_config.py"
        assert config_file.exists()

        content = config_file.read_text()
        assert "MossConfig" in content
        assert "with_project" in content

    def test_creates_moss_directory(self, tmp_path: Path):
        args = create_parser().parse_args(["init", str(tmp_path)])
        cmd_init(args)

        moss_dir = tmp_path / ".moss"
        assert moss_dir.exists()
        assert (moss_dir / ".gitignore").exists()

    def test_refuses_to_overwrite(self, tmp_path: Path):
        config_file = tmp_path / "moss_config.py"
        config_file.write_text("existing")

        args = create_parser().parse_args(["init", str(tmp_path)])
        result = cmd_init(args)

        assert result == 1

    def test_force_overwrites(self, tmp_path: Path):
        config_file = tmp_path / "moss_config.py"
        config_file.write_text("existing")

        args = create_parser().parse_args(["init", str(tmp_path), "--force"])
        result = cmd_init(args)

        assert result == 0
        assert "MossConfig" in config_file.read_text()

    def test_custom_distro(self, tmp_path: Path):
        args = create_parser().parse_args(["init", str(tmp_path), "--distro", "strict"])
        cmd_init(args)

        config_file = tmp_path / "moss_config.py"
        content = config_file.read_text()
        assert '"strict"' in content

    def test_nonexistent_directory(self, tmp_path: Path):
        args = create_parser().parse_args(["init", str(tmp_path / "nonexistent")])
        result = cmd_init(args)

        assert result == 1


class TestCmdStatus:
    """Tests for status command."""

    @pytest.fixture
    def git_repo(self, tmp_path: Path):
        """Create a minimal git repo."""
        repo = tmp_path / "repo"
        repo.mkdir()

        subprocess.run(["git", "init"], cwd=repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo, check=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=repo, check=True)
        (repo / "README.md").write_text("# Test")
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"], cwd=repo, capture_output=True, check=True
        )

        return repo

    def test_shows_status(self, git_repo: Path, capsys):
        args = create_parser().parse_args(["status", "-C", str(git_repo)])
        result = cmd_status(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "Moss Status" in captured.out
        assert "Active requests:" in captured.out
        assert "Active workers:" in captured.out


class TestCmdConfig:
    """Tests for config command."""

    def test_list_distros(self, capsys):
        args = create_parser().parse_args(["config", "--list-distros"])
        result = cmd_config(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "python" in captured.out

    def test_no_config_file(self, tmp_path: Path, capsys):
        args = create_parser().parse_args(["config", "-C", str(tmp_path)])
        result = cmd_config(args)

        assert result == 1
        captured = capsys.readouterr()
        assert "No config file" in captured.out

    def test_shows_config(self, tmp_path: Path, capsys):
        # Create a config
        config_file = tmp_path / "moss_config.py"
        config_file.write_text("""
from pathlib import Path
from moss.config import MossConfig

config = MossConfig().with_project(Path(__file__).parent, "test-project")
""")

        args = create_parser().parse_args(["config", "-C", str(tmp_path)])
        result = cmd_config(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "Configuration" in captured.out
        assert "test-project" in captured.out

    def test_validate_config(self, tmp_path: Path, capsys):
        # Create a valid config
        config_file = tmp_path / "moss_config.py"
        config_file.write_text("""
from pathlib import Path
from moss.config import MossConfig

config = MossConfig().with_project(Path(__file__).parent, "test-project")
""")

        args = create_parser().parse_args(["config", "-C", str(tmp_path), "--validate"])
        result = cmd_config(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "valid" in captured.out.lower()


class TestCmdDistros:
    """Tests for distros command."""

    def test_lists_distros(self, capsys):
        args = create_parser().parse_args(["distros"])
        result = cmd_distros(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "Available Distros" in captured.out
        assert "python" in captured.out
        assert "strict" in captured.out
        assert "lenient" in captured.out
        assert "fast" in captured.out


class TestCmdRun:
    """Tests for run command."""

    @pytest.fixture
    def git_repo(self, tmp_path: Path):
        """Create a minimal git repo."""
        repo = tmp_path / "repo"
        repo.mkdir()

        subprocess.run(["git", "init"], cwd=repo, capture_output=True, check=True)
        subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=repo, check=True)
        subprocess.run(["git", "config", "user.name", "Test User"], cwd=repo, check=True)
        (repo / "README.md").write_text("# Test")
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"], cwd=repo, capture_output=True, check=True
        )

        return repo

    def test_creates_task(self, git_repo: Path, capsys):
        args = create_parser().parse_args(
            [
                "run",
                "Test task",
                "-C",
                str(git_repo),
            ]
        )
        result = cmd_run(args)

        assert result == 0
        captured = capsys.readouterr()
        assert "Task created:" in captured.out
        assert "Ticket:" in captured.out

    def test_with_priority(self, git_repo: Path, capsys):
        args = create_parser().parse_args(
            [
                "run",
                "High priority task",
                "-C",
                str(git_repo),
                "--priority",
                "high",
            ]
        )
        result = cmd_run(args)

        assert result == 0

    def test_with_constraints(self, git_repo: Path, capsys):
        args = create_parser().parse_args(
            [
                "run",
                "Constrained task",
                "-C",
                str(git_repo),
                "-c",
                "no-tests",
                "-c",
                "dry-run",
            ]
        )
        result = cmd_run(args)

        assert result == 0
