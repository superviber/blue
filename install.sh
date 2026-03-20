#!/bin/bash
# Install Blue CLI and Claude Code integration
# Usage: ./install.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "Installing Blue..."

# 1. Build and install binary via cargo
echo "Building release binary..."
cargo install --path "$(cd "$(dirname "$0")" && pwd)/apps/blue-cli"

# 2. Verify installation
echo "Verifying..."
if command -v blue &> /dev/null; then
    echo -e "${GREEN}Binary installed${NC}"
    blue --version 2>/dev/null || true
else
    echo -e "${RED}Binary not found in PATH${NC}"
    echo "Ensure ~/.cargo/bin is in your PATH"
    exit 1
fi

# 3. Install hooks, skills, and Claude Code integration
echo ""
blue install

echo ""
echo -e "${GREEN}Done.${NC} Restart Claude Code to activate."
