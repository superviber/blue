#!/bin/bash
# Managed by: blue install
# Blue SessionStart hook - sets up PATH and detects worktree context

if [ -n "$CLAUDE_ENV_FILE" ] && [ -n "$CLAUDE_PROJECT_DIR" ]; then
  echo "export PATH=\"\$CLAUDE_PROJECT_DIR/target/release:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi

# RFC 0073: Detect worktree context and suggest /wt start
if [ -f "$CLAUDE_PROJECT_DIR/.git" ]; then
  # .git is a file = we're in a worktree
  BRANCH=$(git -C "$CLAUDE_PROJECT_DIR" branch --show-current 2>/dev/null)
  if [[ "$BRANCH" == feature/* ]]; then
    SLUG="${BRANCH#feature/}"
    echo "Worktree detected: $BRANCH"
    echo "Run /wt start to load RFC context and begin work."
  fi
fi

exit 0
