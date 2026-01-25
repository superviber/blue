#!/bin/bash
# Install Blue CLI to system path

set -e

# Default install location
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "Building Blue (release)..."
cargo build --release

BINARY="target/release/blue"

if [ ! -f "$BINARY" ]; then
    echo -e "${RED}Build failed - binary not found${NC}"
    exit 1
fi

echo "Installing to $INSTALL_DIR..."

if [ -w "$INSTALL_DIR" ]; then
    cp "$BINARY" "$INSTALL_DIR/blue"
else
    echo "Need sudo for $INSTALL_DIR"
    sudo cp "$BINARY" "$INSTALL_DIR/blue"
fi

# Verify installation
if command -v blue &> /dev/null; then
    echo -e "${GREEN}Installed successfully${NC}"
    echo ""
    blue --version 2>/dev/null || blue help 2>/dev/null | head -1 || echo "blue installed to $INSTALL_DIR/blue"
else
    echo -e "${GREEN}Installed to $INSTALL_DIR/blue${NC}"
    echo "Add $INSTALL_DIR to PATH if not already present"
fi

# Update MCP config if it exists
MCP_CONFIG="$HOME/.config/claude-code/mcp.json"
if [ -f "$MCP_CONFIG" ]; then
    echo ""
    echo "Updating MCP config to use installed path..."

    # Check if config references the old path
    if grep -q "target/release/blue" "$MCP_CONFIG" 2>/dev/null; then
        if command -v jq &> /dev/null; then
            jq '.mcpServers.blue.command = "blue"' "$MCP_CONFIG" > "$MCP_CONFIG.tmp" && mv "$MCP_CONFIG.tmp" "$MCP_CONFIG"
            echo -e "${GREEN}MCP config updated${NC}"
        else
            echo "Install jq to auto-update MCP config, or manually change:"
            echo "  command: \"blue\""
        fi
    fi
fi

# Install Blue skills to Claude Code
SKILLS_DIR="$HOME/.claude/skills"
BLUE_SKILLS_DIR="$(dirname "$0")/skills"

if [ -d "$BLUE_SKILLS_DIR" ] && [ -d "$HOME/.claude" ]; then
    echo ""
    echo "Installing Blue skills..."
    mkdir -p "$SKILLS_DIR"

    for skill in "$BLUE_SKILLS_DIR"/*; do
        if [ -d "$skill" ]; then
            skill_name=$(basename "$skill")
            cp -r "$skill" "$SKILLS_DIR/"
            echo "  Installed skill: $skill_name"
        fi
    done

    echo -e "${GREEN}Skills installed to $SKILLS_DIR${NC}"
fi

# Install Blue hooks to Claude Code
HOOKS_FILE="$HOME/.claude/hooks.json"
BLUE_ROOT="$(cd "$(dirname "$0")" && pwd)"

if [ -d "$HOME/.claude" ]; then
    echo ""
    echo "Configuring Blue hooks..."

    # Create hooks.json if it doesn't exist
    if [ ! -f "$HOOKS_FILE" ]; then
        echo '{"hooks":{}}' > "$HOOKS_FILE"
    fi

    # Update hooks using jq if available, otherwise create fresh
    if command -v jq &> /dev/null; then
        jq --arg blue_root "$BLUE_ROOT" '.hooks.SessionStart.command = ($blue_root + "/hooks/session-start") | .hooks.SessionEnd.command = ($blue_root + "/target/release/blue session-end") | .hooks.PreToolUse.command = ($blue_root + "/target/release/blue session-heartbeat") | .hooks.PreToolUse.match = "blue_*"' "$HOOKS_FILE" > "$HOOKS_FILE.tmp" && mv "$HOOKS_FILE.tmp" "$HOOKS_FILE"
        echo -e "${GREEN}Hooks configured${NC}"
    else
        # Fallback: write hooks directly
        cat > "$HOOKS_FILE" << EOF
{
  "hooks": {
    "SessionStart": {
      "command": "$BLUE_ROOT/hooks/session-start"
    },
    "SessionEnd": {
      "command": "$BLUE_ROOT/target/release/blue session-end"
    },
    "PreToolUse": {
      "command": "$BLUE_ROOT/target/release/blue session-heartbeat",
      "match": "blue_*"
    }
  }
}
EOF
        echo -e "${GREEN}Hooks configured (install jq for safer merging)${NC}"
    fi
fi

echo ""
echo "Done. Restart Claude Code to use the new installation."
