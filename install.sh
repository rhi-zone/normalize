#!/bin/bash
# Normalize CLI installer
# Usage: curl -fsSL https://raw.githubusercontent.com/rhi-zone/normalize/master/install.sh | sh
# Version pinning: NORMALIZE_VERSION=0.2.0 curl -fsSL ... | sh

set -e

REPO="rhi-zone/normalize"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
# Musl artifacts ship a wrapper + runtime/ bundle; install under LIBEXEC_DIR
# and symlink into INSTALL_DIR so the wrapper can find its sibling runtime/.
LIBEXEC_DIR="${LIBEXEC_DIR:-$HOME/.local/share/normalize}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)
                # The musl artifact is fully self-contained (wrapper + bundled
                # ld-musl loader + libc + libgcc_s from Alpine). It works on
                # NixOS and any non-FHS distro without system musl installed.
                # Prefer gnu only when the glibc dynamic linker is present.
                if [ -e /lib64/ld-linux-x86-64.so.2 ] && [ ! -f /etc/NIXOS ]; then
                    TARGET="x86_64-unknown-linux-gnu"
                else
                    TARGET="x86_64-unknown-linux-musl"
                fi
                ;;
            aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            arm64) TARGET="aarch64-apple-darwin" ;;
            x86_64) echo "Intel Macs are not supported. Use an Apple Silicon Mac or Linux."; exit 1 ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "For Windows, use: irm https://raw.githubusercontent.com/$REPO/master/install.ps1 | iex"
        exit 1
        ;;
esac

# Resolve version
if [ -n "$NORMALIZE_VERSION" ]; then
    VERSION="$NORMALIZE_VERSION"
    # Strip leading 'v' if present
    VERSION="${VERSION#v}"
    TAG="v$VERSION"
else
    echo "Fetching latest release..."
    TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$TAG" ]; then
        echo "Failed to fetch latest version"
        exit 1
    fi
    VERSION="${TAG#v}"
fi

# Check existing installation before downloading anything
EXISTING=""
if [ -x "$INSTALL_DIR/normalize" ]; then
    EXISTING=$("$INSTALL_DIR/normalize" --version 2>/dev/null | awk '{print $2}' || true)
fi
if [ "$EXISTING" = "$VERSION" ]; then
    echo "normalize $TAG is already installed."
    SKIP_INSTALL=true
fi

if [ -z "$SKIP_INSTALL" ]; then
    echo "Installing normalize $TAG for $TARGET..."
fi

if [ -z "$SKIP_INSTALL" ]; then
    # Download archive and checksums
    BASE_URL="https://github.com/$REPO/releases/download/$TAG"
    ARCHIVE="normalize-$TARGET.tar.gz"
    TMPWORK=$(mktemp -d)
    trap "rm -rf $TMPWORK" EXIT

    curl -fsSL "$BASE_URL/$ARCHIVE" -o "$TMPWORK/normalize.tar.gz"
    curl -fsSL "$BASE_URL/SHA256SUMS.txt" -o "$TMPWORK/SHA256SUMS.txt"

    # Verify checksum
    EXPECTED=$(grep "$ARCHIVE" "$TMPWORK/SHA256SUMS.txt" | awk '{print $1}')
    if [ -z "$EXPECTED" ]; then
        echo "No checksum found for $ARCHIVE in SHA256SUMS.txt"
        exit 1
    fi

    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL=$(sha256sum "$TMPWORK/normalize.tar.gz" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        ACTUAL=$(shasum -a 256 "$TMPWORK/normalize.tar.gz" | awk '{print $1}')
    else
        echo "Warning: no sha256sum or shasum found; skipping checksum verification"
        ACTUAL="$EXPECTED"
    fi

    if [ "$ACTUAL" != "$EXPECTED" ]; then
        echo "Checksum mismatch!"
        echo "  Expected: $EXPECTED"
        echo "  Got:      $ACTUAL"
        exit 1
    fi

    echo "Checksum verified."

    # Extract and install
    tar xz -C "$TMPWORK" -f "$TMPWORK/normalize.tar.gz"
    mkdir -p "$INSTALL_DIR"

    # Musl artifacts ship wrapper + runtime/ bundle; install the whole layout
    # under LIBEXEC_DIR and symlink into INSTALL_DIR. The wrapper resolves its
    # real path at runtime to find the sibling runtime/ directory.
    if [ -d "$TMPWORK/runtime" ] && [ -f "$TMPWORK/normalize" ]; then
        rm -rf "$LIBEXEC_DIR/runtime" "$LIBEXEC_DIR/normalize"
        mkdir -p "$LIBEXEC_DIR"
        mv "$TMPWORK/runtime" "$LIBEXEC_DIR/runtime"
        mv "$TMPWORK/normalize" "$LIBEXEC_DIR/normalize"
        chmod +x "$LIBEXEC_DIR/normalize"
        chmod +x "$LIBEXEC_DIR/runtime/ld-musl-x86_64.so.1" 2>/dev/null || true
        if [ -w "$INSTALL_DIR" ]; then
            ln -sf "$LIBEXEC_DIR/normalize" "$INSTALL_DIR/normalize"
        else
            sudo ln -sf "$LIBEXEC_DIR/normalize" "$INSTALL_DIR/normalize"
        fi
    else
        if [ -w "$INSTALL_DIR" ]; then
            mv "$TMPWORK/normalize" "$INSTALL_DIR/normalize"
        else
            echo "Installing to $INSTALL_DIR (requires sudo)..."
            sudo mv "$TMPWORK/normalize" "$INSTALL_DIR/normalize"
        fi
        chmod +x "$INSTALL_DIR/normalize"
    fi
fi

if [ -z "$SKIP_INSTALL" ]; then
    echo ""
    if [ -n "$EXISTING" ]; then
        echo "Upgraded normalize $EXISTING → $VERSION at $INSTALL_DIR/normalize"
    else
        echo "Installed normalize $TAG to $INSTALL_DIR/normalize"
    fi

    # Verify
    "$INSTALL_DIR/normalize" --version 2>/dev/null || true
fi

# PATH hint if needed
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo ""
        echo "NOTE: $INSTALL_DIR is not in your PATH."
        case "${SHELL##*/}" in
            fish)
                echo "Run:"
                echo "  fish_add_path $INSTALL_DIR"
                ;;
            zsh)
                echo "Run (>> appends, never use >):"
                echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc"
                ;;
            *)
                echo "Run (>> appends, never use >):"
                echo "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.bashrc"
                ;;
        esac
        ;;
esac
