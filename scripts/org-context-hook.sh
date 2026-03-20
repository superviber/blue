#!/bin/sh
# org-context-hook.sh — Inject org-level context into Claude Code sessions
#
# Called by Claude Code hooks (session start, post-compact) to dynamically
# inject org awareness. Reads org.yaml at the org root and domain.yaml +
# jira.toml from the PM repo.
#
# If no org.yaml is found, exits silently (standalone repo case).
#
# Usage: ./scripts/org-context-hook.sh [directory]
#   directory: optional starting directory (defaults to $PWD)
#
# See: RFC 0074 — Org-Level Operations

set -e

# Starting directory for org.yaml discovery
start_dir="${1:-$PWD}"

# --- YAML helpers (simple grep/sed, no yq or python needed) ---

# Read a top-level scalar value from a YAML file.
# Usage: yaml_value file key
yaml_value() {
    _file="$1"
    _key="$2"
    sed -n "s/^${_key}:[[:space:]]*//p" "$_file" | sed 's/^["'"'"']\(.*\)["'"'"']$/\1/' | tr -d '\r'
}

# Read a list of mappings from domain.yaml.
# Extracts field values for items under a top-level key.
# Usage: yaml_list_field file list_key field_key
#   e.g. yaml_list_field domain.yaml repos name
yaml_list_field() {
    _file="$1"
    _list_key="$2"
    _field_key="$3"
    awk -v list="$_list_key" -v field="$_field_key" '
        BEGIN { in_list = 0 }
        /^[a-zA-Z]/ {
            if ($0 ~ "^" list ":") { in_list = 1; next }
            else { in_list = 0 }
        }
        in_list && /^[[:space:]]*-[[:space:]]/ { is_item = 1 }
        in_list && is_item {
            # Match "  - field: value" or "    field: value"
            if (match($0, field ":[[:space:]]*(.*)")) {
                val = $0
                sub(".*" field ":[[:space:]]*", "", val)
                gsub(/^["'"'"']|["'"'"']$/, "", val)
                gsub(/\r/, "", val)
                print val
            }
        }
    ' "$_file"
}

# Read repo entries as "name|description" pairs
yaml_repos() {
    _file="$1"
    awk '
        BEGIN { in_repos = 0; name = ""; desc = "" }
        /^[a-zA-Z]/ {
            if ($0 ~ /^repos:/) { in_repos = 1; next }
            else if (in_repos) {
                if (name != "") { print name "|" desc; name = ""; desc = "" }
                in_repos = 0
            }
        }
        in_repos && /^[[:space:]]*-[[:space:]]*name:/ {
            if (name != "") { print name "|" desc; desc = "" }
            val = $0; sub(/.*name:[[:space:]]*/, "", val)
            gsub(/["'"'"'\r]/, "", val)
            name = val
        }
        in_repos && /^[[:space:]]*description:/ {
            val = $0; sub(/.*description:[[:space:]]*/, "", val)
            gsub(/["'"'"'\r]/, "", val)
            desc = val
        }
        END { if (name != "") print name "|" desc }
    ' "$_file"
}

# Read area entries as "key|name|repos-csv"
yaml_areas() {
    _file="$1"
    awk '
        BEGIN { in_areas = 0; key = ""; aname = ""; repos = "" }
        /^[a-zA-Z]/ {
            if ($0 ~ /^areas:/) { in_areas = 1; next }
            else if (in_areas) {
                if (key != "") { print key "|" aname "|" repos; key = ""; aname = ""; repos = "" }
                in_areas = 0
            }
        }
        in_areas && /^[[:space:]]*-[[:space:]]*key:/ {
            if (key != "") { print key "|" aname "|" repos; aname = ""; repos = "" }
            val = $0; sub(/.*key:[[:space:]]*/, "", val)
            gsub(/["'"'"'\r]/, "", val)
            key = val
        }
        in_areas && /^[[:space:]]*name:/ {
            val = $0; sub(/.*name:[[:space:]]*/, "", val)
            gsub(/["'"'"'\r]/, "", val)
            aname = val
        }
        in_areas && /repos:/ && /\[/ {
            val = $0; sub(/.*repos:[[:space:]]*\[/, "", val); sub(/\].*/, "", val)
            gsub(/["'"'"'\r[:space:]]/, "", val)
            repos = val
        }
        END { if (key != "") print key "|" aname "|" repos }
    ' "$_file"
}

# --- Walk up to find org.yaml ---

find_org_yaml() {
    _dir="$1"
    while [ "$_dir" != "/" ]; do
        if [ -f "$_dir/org.yaml" ]; then
            echo "$_dir/org.yaml"
            return 0
        fi
        _dir="$(dirname "$_dir")"
    done
    return 1
}

# --- Main ---

# Resolve start_dir to absolute path
start_dir="$(cd "$start_dir" 2>/dev/null && pwd)" || exit 0

# Find org.yaml
org_yaml="$(find_org_yaml "$start_dir")" || exit 0

org_root="$(dirname "$org_yaml")"

# Read org.yaml fields
org_name="$(yaml_value "$org_yaml" "org")"
pm_repo_rel="$(yaml_value "$org_yaml" "pm_repo")"

if [ -z "$org_name" ] || [ -z "$pm_repo_rel" ]; then
    exit 0
fi

pm_repo_path="$org_root/$pm_repo_rel"

# Validate PM repo exists
if [ ! -d "$pm_repo_path" ]; then
    echo "[Org Context: $org_name]"
    echo "PM repo: $pm_repo_path (NOT FOUND)"
    exit 0
fi

# --- Detect current location ---

current_location="unknown"

# Normalize start_dir for comparison
case "$start_dir" in
    "$org_root")
        current_location="org root"
        ;;
    "$org_root"/*)
        # Inside org — figure out which repo
        rel_path="${start_dir#$org_root/}"
        repo_dir="${rel_path%%/*}"
        # Check if it matches a known repo or PM repo
        if [ "$repo_dir" = "$pm_repo_rel" ]; then
            current_location="repo: $repo_dir (PM)"
        elif [ -d "$org_root/$repo_dir/.git" ] || [ -f "$org_root/$repo_dir/.git" ]; then
            current_location="repo: $repo_dir"
        else
            current_location="unknown ($repo_dir)"
        fi
        ;;
esac

# --- Read domain.yaml ---

domain_yaml="$pm_repo_path/domain.yaml"
repos_block=""
areas_block=""

if [ -f "$domain_yaml" ]; then
    # Build repos list
    repos_block="$(yaml_repos "$domain_yaml" | while IFS='|' read -r rname rdesc; do
        if [ -d "$org_root/$rname" ]; then
            status="exists"
        else
            status="not cloned"
        fi
        if [ -n "$rdesc" ]; then
            echo "  - $rname: $rdesc [$status]"
        else
            echo "  - $rname [$status]"
        fi
    done)"

    # Build areas list
    areas_block="$(yaml_areas "$domain_yaml" | while IFS='|' read -r akey aname arepos; do
        if [ -n "$arepos" ]; then
            echo "  - $akey ($aname): $arepos"
        else
            echo "  - $akey ($aname)"
        fi
    done)"
fi

# --- Read jira.toml ---

jira_toml="$pm_repo_path/jira.toml"
jira_line=""

if [ -f "$jira_toml" ]; then
    jira_project="$(sed -n 's/^project_key[[:space:]]*=[[:space:]]*"\{0,1\}\([^"]*\)"\{0,1\}/\1/p' "$jira_toml" | head -1)"
    jira_domain="$(sed -n 's/^domain[[:space:]]*=[[:space:]]*"\{0,1\}\([^"]*\)"\{0,1\}/\1/p' "$jira_toml" | head -1)"
    if [ -n "$jira_project" ] && [ -n "$jira_domain" ]; then
        jira_line="$jira_project @ $jira_domain"
    fi
elif [ -f "$domain_yaml" ]; then
    # Fallback: try jira config from domain.yaml
    jira_project="$(yaml_value "$domain_yaml" "  project_key" 2>/dev/null || true)"
    jira_domain="$(yaml_value "$domain_yaml" "  domain" 2>/dev/null || true)"
    if [ -n "$jira_project" ] && [ -n "$jira_domain" ]; then
        jira_line="$jira_project @ $jira_domain"
    fi
fi

# --- Output context block ---

echo "[Org Context: $org_name]"
echo "PM repo: $pm_repo_path"
echo "Current location: $current_location"

if [ -n "$repos_block" ]; then
    echo ""
    echo "Repos:"
    echo "$repos_block"
fi

if [ -n "$areas_block" ]; then
    echo ""
    echo "Areas:"
    echo "$areas_block"
fi

if [ -n "$jira_line" ]; then
    echo ""
    echo "Jira: $jira_line"
fi

echo ""
echo "Org-wide RFCs go in: $pm_repo_path/.blue/docs/rfcs/"
echo "Repo-specific RFCs go in: {repo}/.blue/docs/rfcs/"
