#!/bin/bash
# Blue install script
# Usage: ./scripts/install.sh

set -e

BLUE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Installing Blue from $BLUE_DIR"

# 1. Build and install binary
echo "Building and installing..."
cargo install --path "$BLUE_DIR/apps/blue-cli"

# 2. Set up hooks, skills, and Claude Code integration
echo "Configuring Claude Code integration..."
blue install

echo ""
echo "Installation complete!"
echo ""
echo "To activate changes, restart Claude Code."
echo ""
