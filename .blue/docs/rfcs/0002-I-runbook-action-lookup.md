# RFC 0002: Runbook Action Lookup

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Source Spike** | runbook-driven-actions |

---

## Summary

No way to discover and follow runbooks when performing repo actions. Claude guesses instead of following documented procedures for docker builds, deploys, releases, etc.

## Proposal

### 1. Action Tags in Runbooks

Add `actions` field to runbook frontmatter:

```markdown
# Runbook: Docker Build

| | |
|---|---|
| **Status** | Active |
| **Actions** | docker build, build image, container build |
```

Store actions in SQLite metadata table for fast lookup.

### 2. New Tool: `blue_runbook_lookup`

```
blue_runbook_lookup action="docker build"
```

Returns structured response:

```json
{
  "found": true,
  "runbook": {
    "title": "Docker Build",
    "file": ".blue/docs/runbooks/docker-build.md",
    "actions": ["docker build", "build image", "container build"],
    "operations": [
      {
        "name": "Build Production Image",
        "steps": ["...", "..."],
        "verification": "docker images | grep myapp",
        "rollback": "docker rmi myapp:latest"
      }
    ]
  },
  "hint": "Follow the steps above. Use verification to confirm success."
}
```

If no match: `{ "found": false, "hint": "No runbook found. Proceed with caution." }`

### 3. New Tool: `blue_runbook_actions`

List all registered actions:

```
blue_runbook_actions
```

Returns:
```json
{
  "actions": [
    { "action": "docker build", "runbook": "Docker Build" },
    { "action": "deploy staging", "runbook": "Deployment" },
    { "action": "run tests", "runbook": "Testing" }
  ]
}
```

### 4. Matching Algorithm

Word-based matching with priority:

1. **Exact match** - "docker build" matches "docker build" (100%)
2. **All words match** - "docker" matches "docker build" (90%)
3. **Partial words** - "build" matches "docker build" (80%)

If multiple runbooks match, return highest priority. Ties broken by most specific (more words in action).

### 5. Schema

```sql
-- In metadata table
INSERT INTO metadata (document_id, key, value)
VALUES (runbook_id, 'action', 'docker build');

-- Multiple actions = multiple rows
INSERT INTO metadata (document_id, key, value)
VALUES (runbook_id, 'action', 'build image');
```

### 6. Update `blue_runbook_create`

```
blue_runbook_create title="Docker Build" actions=["docker build", "build image"]
```

- Accept `actions` array parameter
- Store each action in metadata table
- Include in generated markdown

### 7. CLAUDE.md Guidance

Document the pattern for repos:

```markdown
## Runbooks

Before executing build, deploy, or release operations:

1. Check for runbook: `blue_runbook_lookup action="docker build"`
2. If found, follow the documented steps
3. Use verification commands to confirm success
4. If something fails, check rollback procedures

Available actions: `blue_runbook_actions`
```

## Security Note

Runbooks should **never** contain actual credentials or secrets. Use placeholders:

```markdown
**Steps**:
1. Export credentials: `export API_KEY=$YOUR_API_KEY`
2. Run deploy: `./deploy.sh`
```

Not:
```markdown
**Steps**:
1. Run deploy: `API_KEY=abc123 ./deploy.sh`  # WRONG!
```

## Example Runbook

```markdown
# Runbook: Docker Build

| | |
|---|---|
| **Status** | Active |
| **Actions** | docker build, build image, container build |
| **Owner** | Platform Team |

---

## Overview

Build and tag Docker images for the application.

## Prerequisites

- [ ] Docker installed and running
- [ ] Access to container registry
- [ ] `.env` file configured

## Common Operations

### Operation: Build Production Image

**When to use**: Preparing for deployment

**Steps**:
1. Ensure on correct branch: `git branch --show-current`
2. Pull latest: `git pull origin main`
3. Build image: `docker build -t myapp:$(git rev-parse --short HEAD) .`
4. Tag as latest: `docker tag myapp:$(git rev-parse --short HEAD) myapp:latest`

**Verification**:
```bash
docker images | grep myapp
docker run --rm myapp:latest --version
```

**Rollback**:
```bash
docker rmi myapp:latest
docker tag myapp:previous myapp:latest
```

## Troubleshooting

### Symptom: Build fails with "no space left"

**Resolution**:
1. `docker system prune -a`
2. Retry build
```

## Implementation

1. Add `actions` parameter to `blue_runbook_create`
2. Store actions in metadata table
3. Implement `blue_runbook_lookup` with matching algorithm
4. Implement `blue_runbook_actions` for discovery
5. Parse runbook markdown to extract operations
6. Update runbook markdown generation

## Test Plan

- [ ] Create runbook with actions tags
- [ ] Lookup by exact action match
- [ ] Lookup by partial match (word subset)
- [ ] No match returns gracefully
- [ ] Multiple runbooks - highest priority wins
- [ ] List all actions works
- [ ] Actions stored in SQLite metadata
- [ ] Operations parsed from markdown correctly
- [ ] Malformed runbook returns partial data gracefully

---

*"Right then. Let's get to it."*

— Blue
