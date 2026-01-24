#!/bin/bash
# Moss CLI installer
# Usage: curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | bash

set -e

REPO="rhi-zone/normalize"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
            aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64) TARGET="x86_64-apple-darwin" ;;
            arm64) TARGET="aarch64-apple-darwin" ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "For Windows, download from: https://github.com/$REPO/releases"
        exit 1
        ;;
esac

# Get latest version
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
    echo "Failed to fetch latest version"
    exit 1
fi

echo "Installing normalize $LATEST for $TARGET..."

# Download
URL="https://github.com/$REPO/releases/download/$LATEST/normalize-$TARGET.tar.gz"
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

curl -fsSL "$URL" | tar xz -C "$TMPDIR"

# Install
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMPDIR/normalize" "$INSTALL_DIR/normalize"
else
    echo "Installing to $INSTALL_DIR (requires sudo)..."
    sudo mv "$TMPDIR/normalize" "$INSTALL_DIR/normalize"
fi

chmod +x "$INSTALL_DIR/normalize"

echo "Installed moss $LATEST to $INSTALL_DIR/normalize"
echo "Run 'moss --help' to get started"
