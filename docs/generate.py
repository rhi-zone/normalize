#!/usr/bin/env python3
"""Generate API documentation for Moss.

Usage:
    python docs/generate.py          # Generate HTML docs
    python docs/generate.py --serve  # Serve docs locally

Requires: pip install pdoc
"""

import subprocess
import sys
from pathlib import Path


def main():
    docs_dir = Path(__file__).parent
    project_root = docs_dir.parent
    output_dir = docs_dir / "api"

    # Check if pdoc is available
    try:
        import pdoc  # noqa: F401
    except ImportError:
        print("pdoc not installed. Install with: pip install pdoc")
        print("Or: uv add pdoc --optional docs")
        sys.exit(1)

    if "--serve" in sys.argv:
        # Serve docs locally
        print("Starting documentation server...")
        subprocess.run(
            [
                sys.executable,
                "-m",
                "pdoc",
                "--docformat",
                "google",
                "moss",
            ],
            cwd=project_root,
        )
    else:
        # Generate static HTML
        print(f"Generating documentation to {output_dir}")
        output_dir.mkdir(exist_ok=True)

        subprocess.run(
            [
                sys.executable,
                "-m",
                "pdoc",
                "--docformat",
                "google",
                "--output-directory",
                str(output_dir),
                "moss",
            ],
            cwd=project_root,
            check=True,
        )
        print(f"Documentation generated at {output_dir}")
        print(f"Open {output_dir / 'index.html'} in a browser to view")


if __name__ == "__main__":
    main()
