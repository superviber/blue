#!/bin/bash
# PreToolUse hook for Write/Edit/MultiEdit - enforces RFC 0038 worktree protection

# Extract file_path directly with jq (recommended pattern - avoids cat hanging)
FILE_PATH=$(jq -r '.tool_input.file_path // empty')

# If no file_path, allow (shouldn't happen for Write/Edit)
if [ -z "$FILE_PATH" ]; then
    exit 0
fi

# Call blue guard with the extracted path
# Use full path to target/release binary and close stdin
/Users/ericg/letemcook/blue/target/release/blue guard --path="$FILE_PATH" </dev/null
