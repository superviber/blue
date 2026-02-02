#!/bin/bash
# Managed by: blue install
# Blue PreToolUse hook - enforces RFC 0038 worktree protection

FILE_PATH=$(jq -r '.tool_input.file_path // empty')

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

blue guard --path="$FILE_PATH"
