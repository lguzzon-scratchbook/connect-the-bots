#!/bin/bash
# Setup fast Rust builds - run once per dev machine

set -e

echo "Setting up fast Rust build environment..."

# Install sccache if not present
if ! command -v sccache &> /dev/null; then
    echo "Installing sccache..."
    cargo install sccache --locked
fi

# Add to shell profile
SHELL_RC=""
if [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
elif [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
fi

if [ -n "$SHELL_RC" ]; then
    echo "" >> "$SHELL_RC"
    echo "# Fast Rust builds" >> "$SHELL_RC"
    echo "export RUSTC_WRAPPER=sccache" >> "$SHELL_RC"
    echo "export SCCACHE_CACHE_SIZE=10G" >> "$SHELL_RC"
    echo "Added to $SHELL_RC - reload your shell or run: source $SHELL_RC"
fi

# Install mold linker on Linux
if [ "$(uname)" = "Linux" ] && command -v apt-get &> /dev/null; then
    if ! command -v mold &> /dev/null; then
        echo "Installing mold linker..."
        sudo apt-get update && sudo apt-get install -y mold
    fi
fi

echo "Done! Next build will use sccache and fast linker if available."
