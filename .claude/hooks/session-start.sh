#!/bin/bash
# SessionStart hook - sets up PATH for blue CLI
# RFC 0051: Portable hook binary resolution

if [ -n "$CLAUDE_ENV_FILE" ] && [ -n "$CLAUDE_PROJECT_DIR" ]; then
  echo "export PATH=\"\$CLAUDE_PROJECT_DIR/target/release:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi

exit 0
