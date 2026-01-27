# Alignment Dialogue: Cross-Repo Coordination with Realms

| | |
|---|---|
| **Topic** | How should Blue implement cross-repo coordination across ownership boundaries? |
| **Constraint** | Repos may be under different ownership/orgs; need higher-level "realm" above domains |
| **Format** | 12 experts, up to 12 rounds |
| **Started** | 2026-01-24 |

---

## Expert Panel

| ID | Expert | Perspective |
|----|--------|-------------|
| DS | **Distributed Systems Architect** | Consistency, partition tolerance, eventual sync |
| MT | **Multi-Tenant Security Engineer** | Isolation, trust boundaries, access control |
| GW | **Git Workflow Specialist** | Submodules, worktrees, cross-repo PRs |
| DX | **Developer Experience Lead** | Friction, discoverability, cognitive load |
| IC | **IaC Platform Engineer** | Terraform, CDK, cross-account dependencies |
| DB | **Database Architect** | Schema design, federation, replication |
| FD | **Federation/Decentralization Expert** | ActivityPub, DIDs, trust networks |
| EA | **Enterprise Architect** | Governance, compliance, organizational boundaries |
| OS | **Open Source Maintainer** | Forks, upstreams, contribution models |
| PM | **Product Manager** | User stories, prioritization, MVP scope |
| API | **API Designer** | Contracts, versioning, discovery |
| ML | **ML Platform Engineer** | Training pipelines, model registries, lineage |

---

## Round 1: Problem Framing

**Facilitator:** We have two repos (`aperture`, `fungal-image-analysis`) with cross-account AWS dependencies. They're under different ownership. When IAM policies change in one, the other needs updates. How do we coordinate Blue sessions across these boundaries? And what's the hierarchy: Repo → Domain → Realm?

---

**DS (Distributed Systems):**
The fundamental tension is between autonomy and coordination. Each repo is a sovereign unit - it has its own lifecycle, its own Blue database, its own RFCs. Cross-repo coordination is essentially a distributed consensus problem.

I propose we think in terms of **event sourcing**: each repo publishes change events to a shared log. Other repos subscribe to relevant events. No central authority required.

```
Repo A publishes: { type: "iam_policy_change", paths: ["s3://bucket/jobs/*"] }
Repo B subscribes: { filter: "iam_policy_change", affects: ["training_tools_access_stack.py"] }
```

**Concern:** How do we handle the case where Repo B is offline or the maintainer is unavailable?

---

**MT (Multi-Tenant Security):**
Different ownership is the critical constraint here. We cannot assume:
- Shared filesystem access
- Same AWS account
- Trust between maintainers

The security model must be **zero-trust between repos**. A repo can *declare* dependencies, but cannot *write* to another repo. At most, it can *request* changes via a pull-based mechanism.

**Proposal:** Repos sign their change announcements. Dependent repos verify signatures before acting on them.

---

**GW (Git Workflow):**
From a Git perspective, cross-org coordination already has patterns:
1. **Forks + PRs** - Request changes to upstream
2. **Git submodules** - Pin to specific versions
3. **Monorepo** - Avoid the problem entirely

But none of these work well for *reactive* coordination. When aperture adds a new S3 path, we need fungal-image-analysis to *react*, not just be notified.

**Question:** Is the goal to automate the PR creation, or to automate the change detection?

---

**DX (Developer Experience):**
Let's step back. What's the user story?

> As a developer in aperture, when I add a new S3 path to my training code, I want Blue to tell me that fungal-image-analysis needs an IAM policy update, and ideally create a draft PR for me.

The hierarchy question (Repo → Domain → Realm) is about *discovery*. How does Blue in aperture even know that fungal-image-analysis exists and cares about S3 paths?

**Proposal:**
- **Repo**: Single git repository with its own `.blue/` directory
- **Domain**: Collection of repos under common *technical* ownership (same team/org)
- **Realm**: Federation of domains that have declared *trust relationships*

---

**IC (IaC Platform):**
The aperture/fungal example is classic cross-account IaC. In CDK/Terraform world, we solve this with:
1. **Shared parameter stores** (SSM, Secrets Manager)
2. **Stack outputs → Stack inputs** (CloudFormation exports)
3. **Terraform remote state** references

Blue could model this directly:
```yaml
# aperture/.blue/exports.yaml
exports:
  s3_paths:
    - jobs/*
    - training-runs/*

# fungal-image-analysis/.blue/imports.yaml
imports:
  - from: aperture
    resource: s3_paths
    used_in: cdk/training_tools_access_stack.py
```

When exports change, Blue knows exactly what files in importers need review.

---

**DB (Database):**
For the realm/domain/repo hierarchy, I see three levels of storage:

| Level | Storage | Scope | Trust |
|-------|---------|-------|-------|
| Repo | `.blue/blue.db` | Single repo | Full |
| Domain | `~/.blue/domains/{name}/domain.db` | Same org/machine | High |
| Realm | Federated/replicated | Cross-org | Verified |

The realm level is interesting. Options:
1. **Centralized service** - Single realm server (SaaS model)
2. **Peer-to-peer** - Repos gossip directly (complex)
3. **Git-based** - Realm metadata lives in a git repo that all members clone

I lean toward **git-based realm index**. It's auditable, version-controlled, and doesn't require new infrastructure.

---

**FD (Federation):**
The realm concept maps well to **ActivityPub federation**. Each domain is an "actor" that can:
- Announce changes (Create activity)
- Request changes (Offer activity)
- Accept/reject requests (Accept/Reject)

But ActivityPub requires HTTP endpoints, which is heavy for local dev.

**Simpler alternative:** A realm is a git repo containing:
```
realm.yaml          # Realm metadata, trust policies
domains/
  aperture.yaml     # Domain declaration + public key
  fungal.yaml
index/
  s3-paths.yaml     # Cross-domain resource index
  iam-policies.yaml
```

Domains push signed updates to the realm repo. Other domains pull and verify.

---

**EA (Enterprise Architect):**
Governance is the elephant in the room. Different ownership means:
- Different approval processes
- Different deployment schedules
- Different risk tolerances

Blue cannot assume it can *make* changes in foreign repos. It can only:
1. **Detect** that a change is needed
2. **Propose** the change (PR, issue, notification)
3. **Track** whether the change was made

The realm should have a **compatibility matrix**:
```yaml
# realm/compatibility.yaml
aperture@v2.3:
  requires:
    fungal-image-analysis: ">=1.5.0"
  tested_with:
    fungal-image-analysis: "1.5.2"
```

---

**OS (Open Source):**
This is exactly how large open source ecosystems work. Consider:
- **Kubernetes** + **Helm charts** + **Operators** - all different repos/orgs
- **React** + **React Router** + **Redux** - coordinated releases

The pattern is: **interface contracts** + **compatibility testing** + **communication channels**

Blue's role could be:
1. Define interface contracts (exports/imports)
2. Run compatibility checks in CI
3. Open issues/PRs when contracts break

**Key insight:** The realm is the *interface*, not the implementation.

---

**PM (Product Manager):**
For MVP, I'd scope this down dramatically:

**Phase 1: Awareness only**
- Manually register repos in a realm index
- Blue shows "this change may affect: [list of repos]"
- No automation, just visibility

**Phase 2: Detection**
- Define export/import contracts
- Blue detects when exports change
- Creates issues in dependent repos

**Phase 3: Automation**
- Blue creates draft PRs with suggested fixes
- Cross-repo RFC linking
- Session coordination

**Question for the room:** Is Phase 1 sufficient for the aperture/fungal case, or do we need Phase 2 immediately?

---

**API (API Designer):**
The contracts between repos are essentially APIs. I'd model them as:

```yaml
# aperture/.blue/contract.yaml
provides:
  - name: s3-training-paths
    version: "1.0"
    schema:
      type: array
      items:
        type: string
        pattern: "^s3://.*"
    current_value:
      - "s3://cultivarium-fungal-imaging/jobs/*"
      - "s3://cultivarium-fungal-imaging/training-runs/*"

consumes:
  - name: iam-cross-account-role
    from: fungal-image-analysis
    version: ">=1.0"
```

When `current_value` changes, Blue knows to notify consumers.

---

**ML (ML Platform):**
From ML perspective, this is **lineage tracking**. We track:
- Data lineage (where did training data come from?)
- Model lineage (what code/data produced this model?)
- Infrastructure lineage (what IAM/compute enabled this?)

The realm index is a **lineage graph**:
```
aperture:training-code
  → fungal:s3-bucket
  → fungal:iam-role
  → aperture:training-job
```

When any node changes, we can trace impact through the graph.

---

## Round 1 Scoreboard

| Expert | Position | Confidence |
|--------|----------|------------|
| DS | Event sourcing between repos | 0.7 |
| MT | Zero-trust, signature-verified | 0.8 |
| GW | Automate detection, not changes | 0.6 |
| DX | Repo → Domain → Realm hierarchy | 0.9 |
| IC | Export/import contracts | 0.8 |
| DB | Git-based realm index | 0.7 |
| FD | Realm as git repo with signed updates | 0.7 |
| EA | Compatibility matrix + governance | 0.6 |
| OS | Interface contracts pattern | 0.8 |
| PM | Phase 1 (awareness) as MVP | 0.9 |
| API | Versioned contract schemas | 0.8 |
| ML | Lineage graph in realm | 0.7 |

**Emerging consensus:**
1. Realm as a git repo (auditable, no new infra)
2. Export/import contracts between repos
3. Start with awareness, not automation
4. Zero-trust between different owners

---

## Round 2: Realm Architecture Deep Dive

**Facilitator:** We're converging on a git-based realm. Let's get specific. What's in the realm repo? How do domains join? How are changes propagated?

---

**DB (Database):**
Let me propose a concrete schema for the realm repo:

```
realm-letemcook/                    # The realm git repo
├── realm.yaml                      # Realm metadata
├── domains/
│   ├── aperture/
│   │   ├── domain.yaml            # Domain declaration
│   │   ├── exports.yaml           # What this domain provides
│   │   └── imports.yaml           # What this domain consumes
│   └── fungal-image-analysis/
│       ├── domain.yaml
│       ├── exports.yaml
│       └── imports.yaml
├── contracts/
│   ├── s3-paths.schema.yaml       # Shared schema definitions
│   └── iam-roles.schema.yaml
└── graph/
    └── dependencies.yaml          # Computed dependency graph
```

The realm repo is the **source of truth** for cross-domain relationships. Individual repos reference it but don't contain the full picture.

---

**FD (Federation):**
For the trust model, I propose **domain signing**:

```yaml
# domains/aperture/domain.yaml
name: aperture
owner: eric@example.com
repo_url: https://github.com/cultivarium/aperture
public_key: |
  -----BEGIN PUBLIC KEY-----
  MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A...
  -----END PUBLIC KEY-----
joined_at: 2026-01-24T10:00:00Z
trust_level: full  # full | read-only | pending
```

When aperture updates its exports, it signs the commit. Other domains can verify the signature before accepting the change.

**Question:** Who approves new domains joining the realm?

---

**EA (Enterprise Architect):**
The realm needs **governance rules**:

```yaml
# realm.yaml
name: letemcook
version: "1.0"
governance:
  # Who can add new domains?
  admission:
    type: approval  # open | approval | invite-only
    approvers: [eric@example.com]

  # What happens when contracts break?
  breaking_changes:
    require_approval: true
    grace_period_days: 14

  # How are disputes resolved?
  arbitration: maintainer-decides
```

For aperture/fungal (different orgs), I'd use `approval` admission with both owners as approvers.

---

**IC (IaC Platform):**
Let me show how the IAM cross-account case would work concretely:

```yaml
# domains/aperture/exports.yaml
exports:
  - name: required-s3-permissions
    version: "1.2"
    description: S3 paths that aperture needs to access
    schema: $ref:contracts/s3-paths.schema.yaml
    value:
      read:
        - "jobs/*/masks/*"
        - "jobs/*/*/config.json"
        - "training-runs/*"
      write:
        - "jobs/*/*/manifest.json"
        - "training-metrics/*"
```

```yaml
# domains/fungal-image-analysis/imports.yaml
imports:
  - name: required-s3-permissions
    from: aperture
    version: ">=1.0"
    binding: cdk/training_tools_access_stack.py
    transform: |
      # How to apply this import to the binding file
      Update PolicyStatement resources to match imported paths
```

When aperture adds a new path to exports, Blue in fungal sees the import is stale.

---

**DS (Distributed Systems):**
The propagation model matters. Options:

**Pull-based (simpler):**
1. Each Blue session periodically pulls the realm repo
2. Compares local imports against remote exports
3. Flags mismatches

**Push-based (faster):**
1. Realm repo has webhooks
2. When exports change, notify affected domains
3. Requires infrastructure (webhook receiver)

**Hybrid:**
1. Pull on session start
2. Pull before creating RFCs that touch imports/exports
3. Webhook notifications as optimization

For MVP, **pull-based** is sufficient. The realm repo is small; pulling takes milliseconds.

---

**DX (Developer Experience):**
Here's the user flow I envision:

```bash
# First time: join the realm
$ blue realm join ../realm-letemcook
✓ Registered aperture in realm 'letemcook'
✓ Found 1 related domain: fungal-image-analysis
✓ Detected 3 imports from fungal-image-analysis

# During development: Blue warns about cross-repo impact
$ blue status
⚠️ Cross-realm impact detected:
   Your changes to aperture/models/training/s3_paths.py
   affect export 'required-s3-permissions' v1.2

   Consumers:
   - fungal-image-analysis (imports v>=1.0)

   Run 'blue realm check' for details

# Explicit check
$ blue realm check
📋 Export 'required-s3-permissions' changed:
   + "training-runs/checkpoints/*" (new path)

   Impact on fungal-image-analysis:
   - cdk/training_tools_access_stack.py needs update
   - Suggested change: Add path to ReadWriteTrainingData policy

   Actions:
   [1] Create issue in fungal-image-analysis
   [2] Create PR in fungal-image-analysis (if permitted)
   [3] Update realm export version only
```

---

**MT (Multi-Tenant Security):**
For different ownership, we need **permission levels**:

```yaml
# domains/aperture/permissions.yaml
permissions:
  fungal-image-analysis:
    can_read_exports: true
    can_propose_changes: true  # Create PRs
    can_push_changes: false    # Direct commits
    notification_channel: github-issue
```

Aperture allows fungal to *see* its exports and *propose* changes, but not directly modify anything. This respects org boundaries.

---

**OS (Open Source):**
The realm is essentially a **package registry** for infrastructure contracts. Like npm for dependencies, but for cross-repo coordination.

Key insight: The realm shouldn't contain *code*, only *metadata*. The actual implementation lives in each repo. The realm is just the index.

```
realm = { contracts, relationships, versions }
repo = { implementation, .blue/local-config }
```

This separation means repos can evolve independently as long as they satisfy their contracts.

---

**API (API Designer):**
Versioning is critical. I propose **semver for exports**:

- **PATCH**: Value changes within schema (add a new S3 path)
- **MINOR**: Schema extends (add optional field)
- **MAJOR**: Breaking schema change (remove field, rename)

```yaml
# Export version history
exports:
  - name: required-s3-permissions
    version: "1.2.3"
    changelog:
      - "1.2.3: Added training-runs/checkpoints/* path"
      - "1.2.0: Added write permissions"
      - "1.0.0: Initial export"
```

Importers specify version ranges: `>=1.0 <2.0` means "any 1.x version".

---

**PM (Product Manager):**
Scoping for MVP:

**Must have:**
- `blue realm init` - Create realm repo
- `blue realm join` - Register domain in realm
- `blue realm export` - Declare exports
- `blue realm check` - Compare exports vs imports

**Should have:**
- `blue realm import` - Declare imports
- Automatic staleness detection on `blue status`
- Create GitHub issue for mismatches

**Could have:**
- PR creation
- Webhook notifications
- Signature verification

**Won't have (v1):**
- Automatic code changes
- Multi-realm federation
- Conflict resolution

---

**ML (ML Platform):**
One addition: **provenance tracking**. When aperture's training job uses data from fungal's S3 bucket, we should record that lineage:

```yaml
# domains/aperture/lineage.yaml
artifacts:
  - name: swin-mae-v01
    type: model
    inputs:
      - source: fungal-image-analysis
        resource: s3://cultivarium-fungal-imaging/jobs/*/masks/*
        accessed_at: 2026-01-20T10:00:00Z
```

This helps answer: "What models are affected if fungal changes their data format?"

---

**GW (Git Workflow):**
For the realm repo workflow:

1. Domain maintainers have write access to their `domains/{name}/` directory only
2. Changes to `realm.yaml` or `contracts/` require realm owner approval
3. The `graph/` directory is auto-generated by CI

```yaml
# .github/CODEOWNERS
/domains/aperture/       @aperture-maintainer
/domains/fungal-*/       @fungal-maintainer
/realm.yaml              @realm-owner
/contracts/              @realm-owner
```

This gives each domain autonomy over their declarations while protecting shared resources.

---

## Round 2 Scoreboard

| Expert | Position | Confidence |
|--------|----------|------------|
| DB | Concrete realm repo structure | 0.9 |
| FD | Domain signing with public keys | 0.7 |
| EA | Governance rules in realm.yaml | 0.8 |
| IC | Export/import YAML with transforms | 0.9 |
| DS | Pull-based propagation for MVP | 0.8 |
| DX | CLI flow: join → status → check | 0.9 |
| MT | Permission levels between domains | 0.8 |
| OS | Realm as contract registry, not code | 0.9 |
| API | Semver for export versions | 0.8 |
| PM | MVP scope: init, join, export, check | 0.9 |
| ML | Provenance/lineage tracking | 0.6 |
| GW | CODEOWNERS for domain isolation | 0.8 |

**Emerging consensus:**
1. Realm repo with `domains/`, `contracts/`, `graph/` structure
2. Each domain owns their subdirectory
3. Pull-based sync, semver exports
4. MVP: awareness and manual issue creation

---

## Round 3: The Index Problem

**Facilitator:** We've established Repo → Domain → Realm. But the user asked about a *higher level* that ties realms together - an "index". What is this index? How does discovery work across realms?

---

**FD (Federation):**
The index is **realm discovery**. Consider:
- Realm A: `letemcook` (aperture + fungal)
- Realm B: `cultivarium-public` (open source tools)
- Realm C: `ml-infra` (shared ML infrastructure)

A project might participate in multiple realms. The index answers: "What realms exist? What do they provide?"

```yaml
# ~/.blue/index.yaml (local index cache)
realms:
  - name: letemcook
    url: git@github.com:cultivarium/realm-letemcook.git
    domains: [aperture, fungal-image-analysis]

  - name: ml-infra
    url: https://github.com/org/realm-ml-infra.git
    domains: [training-platform, model-registry]
```

---

**EA (Enterprise Architect):**
The index serves different purposes at different scales:

| Scale | Index Purpose |
|-------|---------------|
| Personal | "What realms am I part of?" |
| Team | "What realms does our team maintain?" |
| Org | "What realms exist in our org?" |
| Public | "What public realms can I discover?" |

For the personal/team case, `~/.blue/index.yaml` is sufficient.
For org/public, we need a **registry service** (like Docker Hub for containers).

---

**DS (Distributed Systems):**
I see three index architectures:

**1. Centralized registry:**
```
index.blue.dev/realms/letemcook
index.blue.dev/realms/ml-infra
```
Simple, but single point of failure. Who runs it?

**2. Git-based index of indexes:**
```
github.com/blue-realms/index/
  realms/
    letemcook.yaml → points to realm repo
    ml-infra.yaml
```
Decentralized discovery, but requires coordination.

**3. DNS-like federation:**
```
_blue.letemcook.dev TXT "realm=git@github.com:cultivarium/realm-letemcook.git"
```
Fully decentralized, leverages existing infrastructure.

For MVP, I'd go with **local index file** + **manual realm addition**.

---

**DX (Developer Experience):**
User journey for multi-realm:

```bash
# Discover realms (future: could query registry)
$ blue realm search "ml training"
Found 3 realms:
  1. ml-infra (github.com/org/realm-ml-infra)
  2. pytorch-ecosystem (github.com/pytorch/realm)
  3. letemcook (private - requires auth)

# Join multiple realms
$ blue realm join git@github.com:cultivarium/realm-letemcook.git
$ blue realm join https://github.com/org/realm-ml-infra.git

# See all relationships
$ blue realm graph
aperture (letemcook)
  ├── imports from: fungal-image-analysis (letemcook)
  └── imports from: training-platform (ml-infra)
```

---

**OS (Open Source):**
For public/open-source realms, the index could be **awesome-list style**:

```markdown
# awesome-blue-realms

## ML/AI
- [ml-infra](https://github.com/org/realm-ml-infra) - Shared ML training infrastructure
- [huggingface-ecosystem](https://github.com/hf/realm) - HuggingFace integration contracts

## Cloud Infrastructure
- [aws-cdk-patterns](https://github.com/aws/realm-cdk) - CDK construct contracts
```

No infrastructure needed. Just a curated list that anyone can PR to.

---

**MT (Multi-Tenant Security):**
Trust becomes critical at the index level:

```yaml
# ~/.blue/trust.yaml
trusted_realms:
  - name: letemcook
    url: git@github.com:cultivarium/realm-letemcook.git
    trust_level: full

  - name: ml-infra
    url: https://github.com/org/realm-ml-infra.git
    trust_level: read-only  # Can read exports, won't auto-apply changes

untrusted_realms:
  - pattern: "*.example.com"
    action: block
```

A domain in an untrusted realm can't affect your repo, even if it claims to export something you import.

---

**API (API Designer):**
The index should support **contract discovery**:

```bash
$ blue contract search "s3-access-policy"
Found in 2 realms:
  1. letemcook: required-s3-permissions@1.2.3 (aperture)
  2. aws-patterns: s3-bucket-policy@2.0.0 (aws-cdk-patterns)

$ blue contract show letemcook:required-s3-permissions
Schema: contracts/s3-paths.schema.yaml
Provided by: aperture
Consumed by: fungal-image-analysis
Version: 1.2.3
```

This lets you find existing contracts before defining new ones.

---

**PM (Product Manager):**
For MVP, the index is simply:

```yaml
# ~/.blue/index.yaml
realms:
  - path: /Users/ericg/repos/realm-letemcook
    # or
  - url: git@github.com:cultivarium/realm-letemcook.git
```

That's it. Manual addition, local storage. Federation and discovery come later.

The hierarchy becomes:
```
Index (~/.blue/index.yaml)
  └── Realm (git repo)
        └── Domain (directory in realm)
              └── Repo (.blue/ in actual code repo)
```

---

**DB (Database):**
For local storage, I'd add realm tracking to the domain-level DB:

```sql
-- ~/.blue/domains/{domain}/domain.db

CREATE TABLE realm_memberships (
  realm_name TEXT PRIMARY KEY,
  realm_path TEXT,  -- Local path or URL
  last_synced_at TEXT,
  local_commit TEXT,  -- Last known realm commit
  remote_commit TEXT  -- Latest remote commit (if known)
);

CREATE TABLE cross_realm_imports (
  import_id INTEGER PRIMARY KEY,
  from_realm TEXT,
  from_domain TEXT,
  contract_name TEXT,
  contract_version TEXT,
  local_binding TEXT,  -- File path in this repo
  last_checked_at TEXT,
  status TEXT  -- current | stale | broken
);
```

---

**IC (IaC Platform):**
The index should also track **infrastructure boundaries**:

```yaml
# In realm
infrastructure:
  aws_accounts:
    - id: "111111111111"
      name: training-tools
      domains: [aperture]
    - id: "222222222222"
      name: fungal-analysis
      domains: [fungal-image-analysis]

  cross_account_trust:
    - from: aperture
      to: fungal-image-analysis
      mechanism: iam-assume-role
      role_arn: arn:aws:iam::222222222222:role/training-tools-webapp-access
```

This makes the infrastructure relationships explicit and queryable.

---

**ML (ML Platform):**
At the index level, we can track **artifact registries**:

```yaml
# In index or realm
registries:
  - type: model
    name: cultivarium-models
    url: s3://cultivarium-models/
    realms: [letemcook, ml-infra]

  - type: dataset
    name: fungal-datasets
    url: s3://cultivarium-fungal-imaging/
    realms: [letemcook]
```

When searching for a model's provenance, we can query across realms.

---

**GW (Git Workflow):**
For the realm repo itself, consider **realm releases**:

```bash
$ cd realm-letemcook
$ git tag -a v1.5.0 -m "Added training-runs/checkpoints/* to aperture exports"
$ git push --tags
```

Domains can pin to realm versions:
```yaml
# .blue/config.yaml
realm:
  name: letemcook
  ref: v1.5.0  # or 'main' for latest
```

This gives stability guarantees across different org deployment schedules.

---

## Round 3 Scoreboard

| Expert | Position | Confidence |
|--------|----------|------------|
| FD | Index as realm discovery mechanism | 0.8 |
| EA | Different index scales (personal → public) | 0.7 |
| DS | Local index file for MVP, federation later | 0.9 |
| DX | `blue realm search/join/graph` commands | 0.8 |
| OS | Awesome-list style public index | 0.7 |
| MT | Trust levels per realm in local config | 0.9 |
| API | Contract discovery across realms | 0.7 |
| PM | MVP: ~/.blue/index.yaml, manual only | 0.9 |
| DB | realm_memberships table in domain.db | 0.8 |
| IC | Infrastructure boundaries in realm | 0.8 |
| ML | Artifact registry tracking | 0.6 |
| GW | Realm versioning with git tags | 0.8 |

**Emerging consensus:**
1. Index = `~/.blue/index.yaml` listing realm paths/URLs
2. Realms can be versioned (git tags)
3. Trust levels per realm (full/read-only/blocked)
4. Contract discovery is a "nice to have"
5. Public discovery via awesome-list or registry is future scope

---

## Round 4: The Aperture/Fungal Concrete Case

**Facilitator:** Let's ground this in the specific case. Walk through exactly how aperture and fungal-image-analysis would use this system today.

---

**IC (IaC Platform):**
Let me trace the exact scenario:

**Current state:**
- `aperture` needs S3 access to `cultivarium-fungal-imaging` bucket
- `fungal-image-analysis` has `TrainingToolsAccessStack` granting that access
- When aperture adds a new path, fungal's IAM policy must update

**With Blue realms:**

```bash
# Step 1: Create realm (one-time)
$ mkdir realm-letemcook && cd realm-letemcook
$ blue realm init --name letemcook
Created realm.yaml

# Step 2: Add aperture to realm
$ cd ../aperture
$ blue realm join ../realm-letemcook --as aperture
Created domains/aperture/domain.yaml
Detected exports: required-s3-permissions (s3 paths from training code)

# Step 3: Add fungal to realm
$ cd ../fungal-image-analysis
$ blue realm join ../realm-letemcook --as fungal-image-analysis
Created domains/fungal-image-analysis/domain.yaml
Detected imports: required-s3-permissions → cdk/training_tools_access_stack.py
```

---

**DX (Developer Experience):**
**Day-to-day workflow:**

```bash
# Developer in aperture adds new training metrics path
$ cd aperture
$ vim models/training/metrics_exporter.py
# Added: s3://cultivarium-fungal-imaging/training-metrics/experiments/*

$ blue status
📊 aperture status:
   1 RFC in progress: training-metrics-v2

⚠️  Cross-realm change detected:
    Export 'required-s3-permissions' has new path:
    + training-metrics/experiments/*

    Affected:
    - fungal-image-analysis: cdk/training_tools_access_stack.py

    Run 'blue realm sync' to notify

$ blue realm sync
📤 Updating realm export...
   Updated: domains/aperture/exports.yaml
   New version: 1.3.0 (was 1.2.3)

📋 Created notification:
   - GitHub issue #42 in fungal-image-analysis:
     "Update IAM policy for new S3 path: training-metrics/experiments/*"
```

---

**MT (Multi-Tenant Security):**
**The trust flow:**

1. Aperture updates its export in the realm repo
2. Aperture signs the commit with its domain key
3. Fungal's Blue (on next sync) sees the change
4. Fungal verifies aperture's signature
5. Fungal's maintainer receives notification
6. Fungal's maintainer updates IAM policy
7. Fungal marks import as "resolved"

At no point does aperture have write access to fungal's repo.

---

**GW (Git Workflow):**
**Realm repo activity:**

```bash
$ cd realm-letemcook
$ git log --oneline
abc1234 (HEAD) aperture: export required-s3-permissions@1.3.0
def5678 fungal: resolved import required-s3-permissions@1.2.3
ghi9012 aperture: export required-s3-permissions@1.2.3
...
```

Each domain pushes to their own directory. The realm repo becomes an audit log of cross-repo coordination.

---

**EA (Enterprise Architect):**
**Governance in action:**

Since aperture and fungal are different orgs:
1. Realm has `admission: approval` - both owners approved the realm creation
2. Each domain has `trust_level: full` for the other
3. Breaking changes require 14-day grace period (per realm.yaml)

If aperture tried to remove a path that fungal still needs:
```bash
$ blue realm sync
❌ Breaking change detected:
   Removing path: training-runs/*
   Still imported by: fungal-image-analysis

   This requires:
   1. Coordination with fungal-image-analysis maintainer
   2. 14-day grace period (per realm governance)

   Override with --force (not recommended)
```

---

**DB (Database):**
**What gets stored where:**

```
realm-letemcook/                    # Git repo (shared)
├── domains/aperture/exports.yaml   # Aperture's declared exports
└── domains/fungal/imports.yaml     # Fungal's declared imports

~/.blue/domains/letemcook/          # Local domain-level DB
└── domain.db
    ├── realm_memberships           # Track realm sync state
    └── cross_realm_imports         # Track import health

aperture/.blue/                     # Repo-level
└── blue.db
    ├── documents                   # RFCs, spikes, etc.
    └── realm_binding               # "This repo is aperture in letemcook realm"

fungal-image-analysis/.blue/
└── blue.db
    ├── documents
    └── realm_binding
```

---

**API (API Designer):**
**The export contract for this case:**

```yaml
# realm-letemcook/domains/aperture/exports.yaml
exports:
  - name: required-s3-permissions
    version: 1.3.0
    description: S3 paths that aperture training code needs to access
    schema:
      type: object
      properties:
        read:
          type: array
          items: { type: string, pattern: "^[a-z0-9-/*]+$" }
        write:
          type: array
          items: { type: string, pattern: "^[a-z0-9-/*]+$" }
    value:
      read:
        - "jobs/*/masks/*"
        - "jobs/*/*/config.json"
        - "jobs/*/*/manifest.json"
        - "jobs/*/*/results.json"
        - "training-runs/*"
        - "training-metrics/*"
        - "training-metrics/experiments/*"  # NEW in 1.3.0
      write:
        - "jobs/*/*/manifest.json"
        - "training-metrics/*"
        - "training-metrics/experiments/*"  # NEW in 1.3.0
    changelog:
      - version: 1.3.0
        date: 2026-01-24
        changes:
          - Added training-metrics/experiments/* for experiment tracking
```

---

**PM (Product Manager):**
**MVP implementation order:**

1. **Week 1:** `blue realm init`, basic realm.yaml structure
2. **Week 2:** `blue realm join`, domain registration
3. **Week 3:** Export/import declaration (`blue realm export`, `blue realm import`)
4. **Week 4:** Sync and notification (`blue realm sync`, GitHub issue creation)

**Out of scope for MVP:**
- Automatic code changes
- Signature verification
- Multiple realms per repo
- Public realm registry

---

**DS (Distributed Systems):**
**Sync protocol:**

```python
def realm_sync(repo, realm):
    # 1. Pull latest realm state
    realm.git_pull()

    # 2. Check our exports
    local_exports = detect_exports(repo)
    declared_exports = realm.get_exports(repo.domain_name)

    if local_exports != declared_exports:
        # 3. Update our exports in realm
        realm.update_exports(repo.domain_name, local_exports)
        realm.git_push()

        # 4. Find affected importers
        for importer in realm.find_importers(local_exports.changed):
            create_notification(importer, local_exports.changes)

    # 5. Check our imports
    for imp in realm.get_imports(repo.domain_name):
        export = realm.get_export(imp.from_domain, imp.contract)
        if export.version > imp.resolved_version:
            flag_stale_import(imp, export)
```

---

**OS (Open Source):**
**For the open-source-like case:**

If aperture were public and fungal were a customer:
1. Aperture publishes exports to a public realm
2. Fungal (private) imports from that public realm
3. Aperture doesn't even know fungal exists
4. Fungal gets notified when aperture's exports change

This is exactly how npm/PyPI work - publish contracts, consumers discover and depend.

---

**ML (ML Platform):**
**Lineage integration:**

When aperture runs a training job:
```yaml
# Recorded in aperture/.blue/lineage.yaml
runs:
  - id: run-20260124-001
    type: training
    rfc: training-metrics-v2
    inputs:
      - realm: letemcook
        domain: fungal-image-analysis
        contract: required-s3-permissions
        paths_accessed:
          - training-metrics/experiments/exp-001/*
    outputs:
      - s3://models/swin-mae-v02/
```

This lineage record proves the training job used data from fungal under the agreed contract.

---

## Round 4 Scoreboard

| Expert | Position | Key Contribution |
|--------|----------|------------------|
| IC | Concrete step-by-step setup | init → join → export → sync |
| DX | Day-to-day workflow | status shows cross-realm impact |
| MT | Trust flow without write access | Sign exports, verify on import |
| GW | Realm repo as audit log | Each domain pushes to own directory |
| EA | Breaking change governance | 14-day grace, coordination required |
| DB | Three-level storage model | Realm repo / domain.db / repo.db |
| API | Concrete export YAML | Versioned, schematized, changelogged |
| PM | 4-week MVP timeline | init, join, export, sync |
| DS | Sync protocol pseudocode | Pull, compare, push, notify |
| OS | Public realm pattern | Publish/subscribe without knowing consumers |
| ML | Lineage integration | Record what contracts were used |

**Consensus achieved:** The aperture/fungal case is fully specced. Ready for implementation.

---

## Round 5: What Could Go Wrong?

**Facilitator:** Before we commit to implementation, let's stress-test. What failure modes, edge cases, or concerns haven't we addressed?

---

**DS (Distributed Systems):**
**Concurrency issues:**

What if aperture and fungal both push to the realm repo simultaneously?
- Git handles this with merge conflicts
- But what if both update the same contract version?

**Mitigation:** Version bumps must be monotonic. If conflict, higher version wins. Or use CRDTs for the version number.

---

**MT (Multi-Tenant Security):**
**Trust revocation:**

What if aperture goes rogue? Can they:
1. Push malicious exports that break fungal's CI?
2. Flood the realm with changes?
3. Claim to own contracts they don't?

**Mitigations:**
1. Imports have validation schemas - reject invalid exports
2. Rate limiting on realm pushes
3. CODEOWNERS enforces domain ownership

**Bigger concern:** What if the realm repo itself is compromised?
- Should critical imports have out-of-band verification?
- Maybe high-trust imports require manual approval even on patch versions?

---

**EA (Enterprise Architect):**
**Organizational drift:**

Over time:
- Maintainers leave, domains become orphaned
- Contracts accumulate but aren't cleaned up
- Realm governance becomes stale

**Mitigations:**
1. `blue realm audit` - Check for orphaned domains, stale contracts
2. Require periodic "domain health checks" - maintainer confirms ownership
3. Sunset policy for inactive domains

---

**DX (Developer Experience):**
**Friction concerns:**

1. Extra steps to maintain realm membership
2. Developers forget to run `blue realm sync`
3. Too many notifications ("alert fatigue")

**Mitigations:**
1. `blue status` automatically checks realm state
2. Pre-commit hook runs realm sync
3. Notification batching and filtering

**Worry:** Is this too complex for small teams? Maybe realms are overkill for 2 repos?

---

**GW (Git Workflow):**
**Git-specific issues:**

1. Realm repo becomes huge if many domains/versions
2. Merge conflicts in YAML files are annoying
3. What if someone force-pushes the realm?

**Mitigations:**
1. Prune old export versions after grace period
2. Use line-per-item YAML format for better diffs
3. Protect main branch, require PRs for realm changes

---

**PM (Product Manager):**
**Adoption risk:**

Will people actually use this? Concerns:
1. "Too complex" - just use Slack/email
2. "Not my problem" - maintainers ignore notifications
3. "Works on my machine" - skip the realm step

**Mitigation:** Prove value with aperture/fungal first. If it saves time there, expand.

**Counter-risk:** If we over-engineer, we'll never ship. MVP should be "awareness only" - no automation, just visibility.

---

**IC (IaC Platform):**
**Infrastructure drift:**

The exports say "I need these paths" but what if:
1. The actual IAM policy is different from what's declared?
2. Someone manually edits the policy in AWS console?
3. The CDK code doesn't match the deployed stack?

**Mitigation:** `blue realm verify` should check actual infrastructure state, not just code.

```bash
$ blue realm verify --domain fungal-image-analysis
Checking import: required-s3-permissions@1.3.0

❌ Drift detected:
   Expected: training-metrics/experiments/* in ReadWriteTrainingData
   Actual: Not present in deployed policy

   CDK code: ✓ Updated
   Deployed: ✗ Not deployed

   Run 'cdk deploy TrainingToolsAccessStack' to fix
```

---

**API (API Designer):**
**Schema evolution:**

What if a contract schema needs to change incompatibly?
1. Old importers break on new schema
2. Version 2.0 means everyone must update simultaneously
3. Migration path unclear

**Mitigation:**
1. Support multiple schema versions simultaneously
2. Deprecation period with both old and new exports
3. Migration guides in changelog

---

**DB (Database):**
**Data model limitations:**

Current model assumes:
- One repo = one domain
- One domain = one realm
- Exports are simple key-value

What about:
- Monorepos with multiple domains?
- Same domain in multiple realms?
- Complex exports (e.g., GraphQL schemas)?

**For MVP:** Keep it simple. One repo = one domain = one realm. Revisit if needed.

---

**OS (Open Source):**
**Forking problem:**

If aperture forks:
1. Does the fork inherit realm membership?
2. Can the fork claim the same domain name?
3. What happens to existing contracts?

**Mitigation:** Domain identity should include repo URL, not just name. Forks get new domain identity.

---

**FD (Federation):**
**Realm splits:**

What if letemcook realm splits into two?
- aperture moves to realm-aperture
- fungal stays in realm-letemcook
- They still need to coordinate

**Mitigation:** Cross-realm imports should be possible:
```yaml
imports:
  - contract: required-s3-permissions
    from: realm-aperture:aperture  # realm:domain syntax
```

But this adds complexity. Defer until needed.

---

**ML (ML Platform):**
**Stale lineage:**

Training runs record what contracts they used. But:
1. Contracts change after the run
2. Historical lineage becomes misleading
3. "What version did run X use?" becomes hard

**Mitigation:** Lineage records should snapshot the exact contract version:
```yaml
inputs:
  - contract: required-s3-permissions@1.3.0
    # Includes full contract value at time of use
    snapshot: { read: [...], write: [...] }
```

---

## Round 5 Scoreboard

| Risk | Severity | Mitigation | Confidence |
|------|----------|------------|------------|
| Concurrent pushes | Medium | Git handles, monotonic versions | 0.8 |
| Trust revocation | High | Validation, rate limits, CODEOWNERS | 0.7 |
| Organizational drift | Medium | Audit, health checks, sunset | 0.6 |
| Developer friction | High | Auto-check in status, hooks | 0.7 |
| Git merge conflicts | Low | Line-per-item YAML | 0.9 |
| Adoption | High | Prove value first, MVP only | 0.6 |
| Infrastructure drift | Medium | Verify command | 0.7 |
| Schema evolution | Medium | Multi-version, deprecation | 0.7 |
| Monorepo edge case | Low | Defer, keep simple | 0.8 |
| Fork identity | Low | Include repo URL | 0.8 |
| Realm splits | Low | Defer cross-realm imports | 0.9 |
| Stale lineage | Medium | Snapshot contracts | 0.7 |

**Key decisions:**
1. Start with MVP (awareness only)
2. Prove value on aperture/fungal before expanding
3. Don't solve edge cases until they occur
4. Keep one-repo-one-domain-one-realm for now

---

## Round 6: Implementation Plan

**Facilitator:** We've designed and stress-tested. Let's produce a concrete implementation plan for Blue.

---

**PM (Product Manager):**
**Phased rollout:**

| Phase | Scope | Tools | Duration |
|-------|-------|-------|----------|
| 0 | Foundation | Data model in blue-core | 1 week |
| 1 | Realm init | `blue realm init`, realm.yaml | 1 week |
| 2 | Domain join | `blue realm join`, exports.yaml | 1 week |
| 3 | Awareness | `blue status` shows realm state | 1 week |
| 4 | Sync | `blue realm sync`, notifications | 2 weeks |
| 5 | Polish | Docs, error handling, tests | 1 week |

**Total:** 7 weeks for MVP

---

**DB (Database):**
**Phase 0 - Data model:**

Add to `blue-core/src/`:

```rust
// realm.rs
pub struct Realm {
    pub name: String,
    pub path: PathBuf,  // Local path to realm repo
}

pub struct Domain {
    pub name: String,
    pub realm: String,
    pub repo_path: PathBuf,
}

pub struct Export {
    pub name: String,
    pub version: String,
    pub schema: Option<serde_json::Value>,
    pub value: serde_json::Value,
}

pub struct Import {
    pub contract: String,
    pub from_domain: String,
    pub version_req: String,  // semver requirement
    pub binding: String,      // local file affected
    pub status: ImportStatus, // Current | Stale | Broken
}

pub enum ImportStatus {
    Current,
    Stale { available: String },
    Broken { reason: String },
}
```

---

**IC (IaC Platform):**
**Phase 1 - Realm init:**

```bash
$ blue realm init --name letemcook
```

Creates:
```
realm-letemcook/
├── realm.yaml
├── domains/
└── contracts/
```

```yaml
# realm.yaml
name: letemcook
version: "0.1.0"
created_at: 2026-01-24T10:00:00Z
governance:
  admission: approval
  approvers: []
```

**Tool:** `blue_realm_init`

---

**GW (Git Workflow):**
**Phase 2 - Domain join:**

```bash
$ cd aperture
$ blue realm join ../realm-letemcook --as aperture
```

Actions:
1. Validate realm exists
2. Create `domains/aperture/domain.yaml`
3. Auto-detect exports from code
4. Create `domains/aperture/exports.yaml`
5. Store realm reference in `.blue/config.yaml`
6. Commit to realm repo

```yaml
# .blue/config.yaml (in aperture)
realm:
  name: letemcook
  path: ../realm-letemcook
  domain: aperture
```

**Tool:** `blue_realm_join`

---

**DX (Developer Experience):**
**Phase 3 - Status integration:**

Modify `blue status` to include:

```bash
$ blue status
📊 aperture (domain in letemcook realm)

RFCs:
  - training-metrics-v2 [in-progress]

Realm:
  ✓ Exports: 1 contract (required-s3-permissions@1.2.3)
  ⚠️ Local changes not synced to realm

Related domains:
  - fungal-image-analysis: imports required-s3-permissions
```

**Implementation:** Check realm state on every `blue_status` call.

---

**DS (Distributed Systems):**
**Phase 4 - Sync:**

```bash
$ blue realm sync
```

Protocol:
1. `git pull` realm repo
2. Detect local export changes
3. Update `domains/{name}/exports.yaml`
4. Bump version
5. `git commit` and `git push`
6. Find affected importers
7. Create GitHub issues via `gh` CLI

```bash
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Exports updated:
  required-s3-permissions: 1.2.3 → 1.3.0
  + training-metrics/experiments/*

Notifying consumers:
  - fungal-image-analysis: Created issue #42
    "Update IAM policy: new S3 path training-metrics/experiments/*"

✓ Realm synced
```

**Tools:** `blue_realm_sync`, `blue_realm_check`

---

**API (API Designer):**
**New tools summary:**

| Tool | Description |
|------|-------------|
| `blue_realm_init` | Create new realm |
| `blue_realm_join` | Join repo to realm as domain |
| `blue_realm_leave` | Remove domain from realm |
| `blue_realm_export` | Declare/update exports |
| `blue_realm_import` | Declare imports |
| `blue_realm_sync` | Push exports, check imports |
| `blue_realm_check` | Dry-run sync, show impact |
| `blue_realm_verify` | Check actual infra matches |
| `blue_realm_graph` | Show dependency graph |

**MVP:** init, join, sync, check

---

**MT (Multi-Tenant Security):**
**Permission model for MVP:**

```yaml
# realm.yaml
governance:
  admission: open  # Anyone can join (simplify for MVP)
```

```yaml
# domains/aperture/domain.yaml
name: aperture
maintainers: [eric@example.com]
repo_url: /Users/ericg/letemcook/aperture
# No signatures for MVP
```

**Future:** Add signing, permission levels, trust configuration.

---

**EA (Enterprise Architect):**
**Documentation needed:**

1. **Concept guide:** What are realms, domains, exports, imports?
2. **Tutorial:** Setting up aperture + fungal coordination
3. **Reference:** All realm tools and their options
4. **Troubleshooting:** Common issues and fixes

---

**OS (Open Source):**
**Testing strategy:**

1. Unit tests for realm/domain/export data structures
2. Integration test: Create realm, join two domains, sync
3. E2E test: Simulate the aperture/fungal workflow
4. Property tests: Concurrent syncs, version ordering

---

## Final Convergence

**Facilitator:** Let's summarize our recommendations.

---

## Recommendations

### Architecture

```
Index (~/.blue/index.yaml)
  └── Realm (git repo: realm-{name}/)
        ├── realm.yaml (governance)
        ├── domains/{domain}/
        │   ├── domain.yaml
        │   ├── exports.yaml
        │   └── imports.yaml
        └── contracts/ (shared schemas)

Domain (~/.blue/domains/{name}/)
  └── domain.db (sync state, import health)

Repo (.blue/)
  ├── config.yaml (realm membership)
  └── blue.db (documents, local state)
```

### MVP Scope (7 weeks)

1. `blue_realm_init` - Create realm
2. `blue_realm_join` - Register domain
3. `blue_realm_export` - Declare exports (auto-detect for S3 paths)
4. `blue_realm_import` - Declare imports
5. `blue_realm_sync` - Push exports, create issues for stale imports
6. `blue_realm_check` - Dry-run sync
7. Integrate realm status into `blue_status`

### Key Design Decisions

1. **Realm = git repo** - Auditable, no new infrastructure
2. **Pull-based sync** - Simple, sufficient for small teams
3. **GitHub issues for notifications** - Use existing workflow
4. **One repo = one domain** - Keep simple for MVP
5. **No signatures** - Trust within team, add later if needed
6. **Semver exports** - PATCH/MINOR/MAJOR versioning

### The Aperture/Fungal Workflow

```bash
# Setup (one-time)
$ mkdir realm-letemcook && cd realm-letemcook
$ blue realm init --name letemcook
$ cd ../aperture && blue realm join ../realm-letemcook
$ cd ../fungal-image-analysis && blue realm join ../realm-letemcook

# Daily use
$ cd aperture
$ vim models/training/new_feature.py  # Add S3 path
$ blue status  # Shows realm impact
$ blue realm sync  # Creates issue in fungal

$ cd ../fungal-image-analysis
$ blue status  # Shows stale import
$ vim cdk/training_tools_access_stack.py  # Update policy
$ blue realm sync  # Marks import resolved
```

### Not in MVP

- Signature verification
- Multiple realms per repo
- Public realm registry
- Automatic code changes
- Cross-realm imports
- Infrastructure verification

---

## Dialogue Complete

| Metric | Value |
|--------|-------|
| Rounds | 6 |
| Experts | 12 |
| Consensus | High |
| Ready for RFC | Yes |

**Next step:** Create RFC from this dialogue.
