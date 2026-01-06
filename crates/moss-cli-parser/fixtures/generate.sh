#!/usr/bin/env bash
# Regenerate all CLI --help fixtures.
# Run from within nix-shell for all dependencies.

set -euo pipefail

cd "$(dirname "$0")"

echo "=== Generating clap fixtures ==="
(cd clap && cargo build --release 2>/dev/null)
./clap/target/release/example --help > clap/example.help
./clap/target/release/example build --help > clap/example-build.help
./clap/target/release/example run --help > clap/example-run.help
echo "  clap/example.help"
echo "  clap/example-build.help"
echo "  clap/example-run.help"

if [ -d argparse ]; then
    echo "=== Generating argparse fixtures ==="
    python argparse/example.py --help > argparse/example.help
    echo "  argparse/example.help"
fi

if [ -d click ]; then
    echo "=== Generating click fixtures ==="
    python click/example.py --help > click/example.help
    echo "  click/example.help"
fi

if [ -d commander ]; then
    echo "=== Generating commander fixtures ==="
    node commander/example.js --help > commander/example.help
    echo "  commander/example.help"
fi

echo ""
echo "Done! All fixtures regenerated."
