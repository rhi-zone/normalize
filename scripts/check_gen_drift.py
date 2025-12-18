#!/usr/bin/env python3
"""Check for drift between generated interfaces and committed specs.

This script regenerates interface specifications (OpenAPI, MCP tools) and
compares them to committed versions. Used in CI to ensure the library API
and generated interfaces stay in sync.

Usage:
    # Check for drift (fails if specs differ)
    python scripts/check_gen_drift.py

    # Update committed specs to match current generation
    python scripts/check_gen_drift.py --update

    # Check specific target only
    python scripts/check_gen_drift.py --target openapi
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Project root
ROOT = Path(__file__).parent.parent
SPECS_DIR = ROOT / "specs"


def generate_openapi() -> dict:
    """Generate OpenAPI spec from current library."""
    from moss.gen.http import generate_openapi

    return generate_openapi()


def generate_mcp_tools() -> list[dict]:
    """Generate MCP tool definitions from current library."""
    from moss.gen.mcp import generate_mcp_definitions

    return generate_mcp_definitions()


def load_json(path: Path) -> dict | list | None:
    """Load JSON file if it exists."""
    if not path.exists():
        return None
    return json.loads(path.read_text())


def save_json(path: Path, data: dict | list) -> None:
    """Save data to JSON file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def compare_json(generated: dict | list, committed: dict | list | None, name: str) -> bool:
    """Compare generated spec to committed version.

    Returns:
        True if they match (or committed doesn't exist), False if drift detected
    """
    if committed is None:
        print(f"  {name}: No committed spec found (will be created with --update)")
        return True  # No drift if nothing to compare

    # Normalize for comparison
    gen_str = json.dumps(generated, indent=2, sort_keys=True)
    com_str = json.dumps(committed, indent=2, sort_keys=True)

    if gen_str == com_str:
        print(f"  {name}: OK (matches committed spec)")
        return True
    else:
        print(f"  {name}: DRIFT DETECTED!")
        print("    Generated spec differs from committed version.")
        print("    Run 'python scripts/check_gen_drift.py --update' to update.")
        return False


def check_openapi(update: bool = False) -> bool:
    """Check OpenAPI spec for drift."""
    spec_path = SPECS_DIR / "openapi.json"
    generated = generate_openapi()
    committed = load_json(spec_path)

    if update:
        save_json(spec_path, generated)
        print(f"  openapi: Updated {spec_path.relative_to(ROOT)}")
        return True

    return compare_json(generated, committed, "openapi")


def check_mcp_tools(update: bool = False) -> bool:
    """Check MCP tool definitions for drift."""
    spec_path = SPECS_DIR / "mcp_tools.json"
    generated = generate_mcp_tools()
    committed = load_json(spec_path)

    if update:
        save_json(spec_path, generated)
        print(f"  mcp_tools: Updated {spec_path.relative_to(ROOT)}")
        return True

    return compare_json(generated, committed, "mcp_tools")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check for drift between generated interfaces and committed specs",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Update committed specs to match current generation",
    )
    parser.add_argument(
        "--target",
        choices=["openapi", "mcp", "all"],
        default="all",
        help="Which spec to check (default: all)",
    )
    args = parser.parse_args()

    print("Checking generated interface drift...")

    all_ok = True
    targets = ["openapi", "mcp"] if args.target == "all" else [args.target]

    for target in targets:
        if target == "openapi":
            if not check_openapi(args.update):
                all_ok = False
        elif target == "mcp":
            if not check_mcp_tools(args.update):
                all_ok = False

    if all_ok:
        if args.update:
            print("\nSpecs updated successfully.")
        else:
            print("\nNo drift detected.")
        return 0
    else:
        print("\nDrift detected! Run with --update to fix.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
