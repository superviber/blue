#!/bin/sh
# org-context-hook-test.sh — Tests for org-context-hook.sh
#
# Creates a temporary org structure, runs the hook from various locations,
# and validates the output contains expected strings.
#
# Usage: ./scripts/org-context-hook-test.sh
#
# Exit code: 0 if all tests pass, 1 if any fail.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOOK="$SCRIPT_DIR/org-context-hook.sh"

# Colors (if terminal supports them)
if [ -t 1 ]; then
    GREEN='\033[0;32m'
    RED='\033[0;31m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    GREEN=''
    RED=''
    BOLD=''
    RESET=''
fi

PASS_COUNT=0
FAIL_COUNT=0

pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf "${GREEN}  PASS${RESET}: %s\n" "$1"
}

fail() {
    FAIL_COUNT=$((FAIL_COUNT + 1))
    printf "${RED}  FAIL${RESET}: %s\n" "$1"
    if [ -n "$2" ]; then
        printf "        Expected to find: %s\n" "$2"
        printf "        In output:\n"
        echo "$3" | sed 's/^/        | /'
    fi
}

# Assert output contains a string
assert_contains() {
    _label="$1"
    _expected="$2"
    _output="$3"
    if echo "$_output" | grep -qF "$_expected"; then
        pass "$_label"
    else
        fail "$_label" "$_expected" "$_output"
    fi
}

# Assert output does NOT contain a string
assert_not_contains() {
    _label="$1"
    _unexpected="$2"
    _output="$3"
    if echo "$_output" | grep -qF "$_unexpected"; then
        fail "$_label (should not contain: $_unexpected)" "" "$_output"
    else
        pass "$_label"
    fi
}

# Assert output is empty
assert_empty() {
    _label="$1"
    _output="$2"
    if [ -z "$_output" ]; then
        pass "$_label"
    else
        fail "$_label" "(empty output)" "$_output"
    fi
}

# --- Setup temp org structure ---

TMPDIR_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_ROOT"' EXIT

ORG_ROOT="$TMPDIR_ROOT/test-org"
mkdir -p "$ORG_ROOT"

# Create org.yaml
cat > "$ORG_ROOT/org.yaml" << 'EOF'
org: test-org
pm_repo: project-management
EOF

# Create PM repo with domain.yaml and jira.toml
PM_REPO="$ORG_ROOT/project-management"
mkdir -p "$PM_REPO/.blue/docs/rfcs"
mkdir -p "$PM_REPO/.git"

cat > "$PM_REPO/domain.yaml" << 'EOF'
org: test-org
key: TST

repos:
  - name: frontend-app
    url: git@github.com:test-org/frontend-app.git
    description: "React frontend application"
  - name: backend-api
    url: git@github.com:test-org/backend-api.git
    description: "Rust API server"
  - name: not-cloned-repo
    url: git@github.com:test-org/not-cloned-repo.git
    description: "A repo that is not cloned locally"

areas:
  - key: FE
    name: Frontend
    components: [Engineering, Design]
    repos: [frontend-app]
  - key: API
    name: Backend API
    components: [Engineering]
    repos: [backend-api, frontend-app]
EOF

cat > "$PM_REPO/jira.toml" << 'EOF'
domain = "test-org.atlassian.net"
project_key = "TST"
drift_policy = "warn"
EOF

# Create cloned repos (frontend-app and backend-api exist, not-cloned-repo does not)
mkdir -p "$ORG_ROOT/frontend-app/.git"
mkdir -p "$ORG_ROOT/frontend-app/src/components/deep"
mkdir -p "$ORG_ROOT/backend-api/.git"

# --- Run tests ---

printf "${BOLD}Running org-context-hook tests${RESET}\n\n"

# ============================================================
printf "${BOLD}Test group: From org root${RESET}\n"
# ============================================================

output="$(sh "$HOOK" "$ORG_ROOT")"

assert_contains "shows org name" "[Org Context: test-org]" "$output"
assert_contains "shows PM repo path" "PM repo: $PM_REPO" "$output"
assert_contains "detects org root" "Current location: org root" "$output"
assert_contains "lists frontend-app as exists" "frontend-app: React frontend application [exists]" "$output"
assert_contains "lists backend-api as exists" "backend-api: Rust API server [exists]" "$output"
assert_contains "lists not-cloned-repo as not cloned" "not-cloned-repo: A repo that is not cloned locally [not cloned]" "$output"
assert_contains "shows FE area" "FE (Frontend): frontend-app" "$output"
assert_contains "shows API area" "API (Backend API): backend-api,frontend-app" "$output"
assert_contains "shows Jira config" "Jira: TST @ test-org.atlassian.net" "$output"
assert_contains "shows org-wide RFC path" "Org-wide RFCs go in: $PM_REPO/.blue/docs/rfcs/" "$output"
assert_contains "shows repo-specific RFC path" "Repo-specific RFCs go in: {repo}/.blue/docs/rfcs/" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: From inside a repo${RESET}\n"
# ============================================================

output="$(sh "$HOOK" "$ORG_ROOT/frontend-app")"

assert_contains "detects repo name" "Current location: repo: frontend-app" "$output"
assert_contains "still shows org name" "[Org Context: test-org]" "$output"
assert_contains "still shows repos" "frontend-app" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: From deep inside a repo${RESET}\n"
# ============================================================

output="$(sh "$HOOK" "$ORG_ROOT/frontend-app/src/components/deep")"

assert_contains "detects repo from deep path" "Current location: repo: frontend-app" "$output"
assert_contains "still shows org context" "[Org Context: test-org]" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: From PM repo${RESET}\n"
# ============================================================

output="$(sh "$HOOK" "$PM_REPO")"

assert_contains "detects PM repo" "Current location: repo: project-management (PM)" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: No org.yaml (standalone repo)${RESET}\n"
# ============================================================

standalone="$TMPDIR_ROOT/standalone-repo"
mkdir -p "$standalone/.git"
mkdir -p "$standalone/.blue"

output="$(sh "$HOOK" "$standalone")"

assert_empty "no output for standalone repo" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: PM repo missing from disk${RESET}\n"
# ============================================================

BROKEN_ORG="$TMPDIR_ROOT/broken-org"
mkdir -p "$BROKEN_ORG"
cat > "$BROKEN_ORG/org.yaml" << 'EOF'
org: broken-org
pm_repo: missing-pm
EOF

output="$(sh "$HOOK" "$BROKEN_ORG")"

assert_contains "shows org name even if PM missing" "[Org Context: broken-org]" "$output"
assert_contains "indicates PM not found" "NOT FOUND" "$output"
echo ""

# ============================================================
printf "${BOLD}Test group: Jira config from domain.yaml fallback${RESET}\n"
# ============================================================

NOJIRA_ORG="$TMPDIR_ROOT/nojira-org"
mkdir -p "$NOJIRA_ORG"
cat > "$NOJIRA_ORG/org.yaml" << 'EOF'
org: nojira-org
pm_repo: pm
EOF

mkdir -p "$NOJIRA_ORG/pm/.git"
cat > "$NOJIRA_ORG/pm/domain.yaml" << 'EOF'
org: nojira-org
key: NJ

repos:
  - name: app
    description: "The app"
EOF

output="$(sh "$HOOK" "$NOJIRA_ORG")"

assert_contains "works without jira.toml" "[Org Context: nojira-org]" "$output"
assert_contains "lists repo without jira" "app: The app" "$output"
# No jira line expected — that's fine
echo ""

# ============================================================
printf "${BOLD}Test group: Quoted YAML values${RESET}\n"
# ============================================================

QUOTED_ORG="$TMPDIR_ROOT/quoted-org"
mkdir -p "$QUOTED_ORG"
cat > "$QUOTED_ORG/org.yaml" << 'EOF'
org: "quoted-org"
pm_repo: 'pm-repo'
EOF

mkdir -p "$QUOTED_ORG/pm-repo/.git"
cat > "$QUOTED_ORG/pm-repo/domain.yaml" << 'EOF'
org: quoted-org
repos:
  - name: my-app
    description: "An app with quotes"
EOF

output="$(sh "$HOOK" "$QUOTED_ORG")"

assert_contains "handles double-quoted org" "[Org Context: quoted-org]" "$output"
assert_contains "handles single-quoted pm_repo" "PM repo: $QUOTED_ORG/pm-repo" "$output"
echo ""

# --- Summary ---

printf "\n${BOLD}Results: ${GREEN}$PASS_COUNT passed${RESET}"
if [ "$FAIL_COUNT" -gt 0 ]; then
    printf ", ${RED}$FAIL_COUNT failed${RESET}"
fi
printf "\n"

if [ "$FAIL_COUNT" -gt 0 ]; then
    exit 1
fi
