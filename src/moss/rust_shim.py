"""Shim to delegate to Rust CLI for fast commands when available."""

import json
import shutil
import subprocess
from pathlib import Path


def find_rust_binary() -> Path | None:
    """Find the Rust moss binary if available."""
    # Check common locations
    candidates = [
        # Development: relative to repo root
        Path(__file__).parent.parent.parent / "target" / "release" / "moss",
        # Installed via cargo
        Path.home() / ".cargo" / "bin" / "moss-cli",
        # System PATH
        shutil.which("moss-cli"),
    ]

    for candidate in candidates:
        if candidate is None:
            continue
        path = Path(candidate) if isinstance(candidate, str) else candidate
        if path.exists() and path.is_file():
            return path

    return None


_RUST_BINARY: Path | None = None
_RUST_CHECKED: bool = False


def get_rust_binary() -> Path | None:
    """Get cached Rust binary path."""
    global _RUST_BINARY, _RUST_CHECKED
    if not _RUST_CHECKED:
        _RUST_BINARY = find_rust_binary()
        _RUST_CHECKED = True
    return _RUST_BINARY


def rust_available() -> bool:
    """Check if Rust CLI is available."""
    return get_rust_binary() is not None


def call_rust(args: list[str], json_output: bool = False) -> tuple[int, str]:
    """Call the Rust CLI with given arguments.

    Returns (exit_code, output).
    """
    binary = get_rust_binary()
    if binary is None:
        raise RuntimeError("Rust binary not available")

    cmd = [str(binary)]
    if json_output:
        cmd.append("--json")
    cmd.extend(args)

    result = subprocess.run(cmd, capture_output=True, text=True)
    output = result.stdout if result.returncode == 0 else result.stderr
    return result.returncode, output


def passthrough(subcommand: str, argv: list[str]) -> int:
    """Pass CLI args directly to Rust subcommand, streaming output.

    Args:
        subcommand: The Rust CLI subcommand (e.g., "expand", "callers")
        argv: Raw CLI arguments to pass through (after the subcommand)

    Returns:
        Exit code from Rust CLI.
    """
    binary = get_rust_binary()
    if binary is None:
        import sys

        print(
            f"Rust CLI required for {subcommand}. Build with: cargo build --release",
            file=sys.stderr,
        )
        return 1

    cmd = [str(binary), subcommand, *argv]
    result = subprocess.run(cmd)
    return result.returncode


def rust_find_symbols(
    name: str,
    kind: str | None = None,
    fuzzy: bool = True,
    limit: int = 50,
    root: str | None = None,
) -> list[dict] | None:
    """Find symbols by name using Rust CLI.

    Returns list of symbol matches or None if Rust not available.
    Each match: {"name", "kind", "file", "line", "end_line", "parent"}
    """
    if not rust_available():
        return None

    args = ["find-symbols", "-l", str(limit)]
    if kind:
        args.extend(["-k", kind])
    if not fuzzy:
        args.extend(["-f", "false"])
    if root:
        args.extend(["-r", root])
    args.append(name)

    code, output = call_rust(args, json_output=True)
    if code != 0:
        return []

    return json.loads(output)


def rust_grep(
    pattern: str,
    glob_pattern: str | None = None,
    limit: int = 100,
    ignore_case: bool = False,
    root: str | None = None,
) -> dict | None:
    """Search for text patterns using Rust CLI.

    Returns {"matches": [...], "total_matches": int, "files_searched": int}
    or None if Rust not available.
    """
    if not rust_available():
        return None

    args = ["grep", "-l", str(limit)]
    if ignore_case:
        args.append("-i")
    if glob_pattern:
        args.extend(["--glob", glob_pattern])
    if root:
        args.extend(["-r", root])
    args.append(pattern)

    code, output = call_rust(args, json_output=True)
    if code != 0:
        return {"matches": [], "total_matches": 0, "files_searched": 0}

    return json.loads(output)


def rust_context(file: str, root: str | None = None) -> dict | None:
    """Get compiled context for a file using Rust CLI.

    Returns {
        "file": str,
        "summary": {"lines", "classes", "functions", "methods", "imports", "exports"},
        "symbols": [...],
        "imports": [...],
        "exports": [...]
    } or None if Rust not available.
    """
    if not rust_available():
        return None

    args = ["context"]
    if root:
        args.extend(["-r", root])
    args.append(file)

    code, output = call_rust(args, json_output=True)
    if code != 0:
        return None

    return json.loads(output)


def rust_skeleton(file_path: str, root: str | None = None) -> str | None:
    """Extract code skeleton using Rust CLI.

    Returns formatted skeleton string or None if Rust not available.
    """
    if not rust_available():
        return None

    args = ["skeleton"]
    if root:
        args.extend(["-r", root])
    args.append(file_path)

    code, output = call_rust(args, json_output=False)
    if code != 0:
        return None

    return output


def rust_summarize(file_path: str, root: str | None = None) -> str | None:
    """Summarize a file using Rust CLI.

    Returns summary string or None if Rust not available.
    """
    if not rust_available():
        return None

    args = ["summarize"]
    if root:
        args.extend(["-r", root])
    args.append(file_path)

    code, output = call_rust(args, json_output=False)
    if code != 0:
        return None

    return output
