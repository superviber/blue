# RFC 0012: Alignment Dialogue Orchestration

| | |
|---|---|
| **Status** | In-Progress |
| **Date** | 2026-01-25 |
| **Source Spike** | Background Agents and Dialogue Creation Not Triggering |
| **Depends On** | RFC 0005 (Local LLM Integration) |

---

## Summary

Users expect to run "play alignment with 12 experts to 95%" and have Blue spawn multiple LLM-powered expert agents that deliberate in rounds until reaching a convergence threshold, then save the resulting dialogue document.

Currently, Blue has dialogue document tools (create/save/lint) but no orchestration layer. The alignment dialogue format exists (rounds, scoreboard, perspectives, tensions, convergence gates) but generation is manual.

This RFC proposes `blue_alignment_play` - a tool that uses Ollama (RFC 0005) to run multi-agent deliberation locally, tracking convergence and producing validated dialogue documents.

---

## Key Insight from coherence-mcp

The orchestration was **never in the MCP server**. It's done by **Claude itself** using the **Task tool** to spawn parallel background agents.

From ADR 0006 (alignment-dialogue-agents):
- **N+1 Architecture**: N "Cupcake" 🧁 agents + 1 "Judge" 💙 (the main Claude session)
- **Parallel Execution**: All N agents spawned in a **single message** with N Task tool calls
- **Claude orchestrates**: Main session acts as Judge, spawns agents, collects outputs, scores
- **Blue MCP's role**: Dialogue extraction (`blue_extract_dialogue`), saving (`blue_dialogue_save`), and linting (`blue_dialogue_lint`)

The "play alignment" command should trigger Claude to:
1. Spawn N parallel Task agents (the experts/cupcakes)
2. Collect their outputs
3. Score and update the dialogue file
4. Repeat until convergence

This means we need a **prompt/skill** that instructs Claude how to orchestrate, NOT a new Blue MCP tool.

---

## Design

### Option A: Claude Code Skill (Recommended)

Create a skill that Claude recognizes and executes:

```markdown
# /alignment-dialogue skill

When user says "play alignment with N experts to X%":

1. Parse: experts=N (default 12), convergence=X (default 95)
2. Generate expert panel appropriate to the topic
3. Create .dialogue.md with empty scoreboard
4. For each round:
   a. Spawn N Task agents in parallel (single message)
   b. Wait for all to complete
   c. Extract outputs via blue_extract_dialogue
   d. Score contributions, update scoreboard
   e. Check convergence (velocity → 0, or threshold met)
5. Save final dialogue via blue_dialogue_save
6. Validate via blue_dialogue_lint
```

### Option B: Blue MCP Orchestration Tool (Alternative)

If we want Blue to drive the orchestration:

### Tool: `blue_alignment_play`

```yaml
parameters:
  topic: string          # Required: What to deliberate on
  constraint: string     # Optional: Key constraint or boundary
  expert_count: int      # Default: 12
  convergence: float     # Default: 0.95 (95%)
  max_rounds: int        # Default: 12
  rfc_title: string      # Optional: Link dialogue to RFC
  model: string          # Default: from blue config or "qwen2.5-coder:7b"

returns:
  dialogue_file: path    # Path to generated dialogue
  rounds: int            # How many rounds ran
  final_convergence: float
  expert_panel: list     # The experts that participated
```

### Expert Panel Generation

Blue generates a domain-appropriate expert panel based on the topic:

```rust
// Example panel for "cross-repo coordination"
struct Expert {
    id: String,        // "DS", "MT", "GW"
    name: String,      // "Distributed Systems Architect"
    perspective: String, // "Consistency, partition tolerance"
    emoji: Option<String>,
}

fn generate_panel(topic: &str, count: usize) -> Vec<Expert> {
    // Use LLM to generate relevant experts
    // Or select from predefined templates
}
```

**Predefined Templates:**
- `infrastructure` - DS, Security, IaC, DX, API, DB, DevOps, SRE, ...
- `product` - PM, UX, Engineering, QA, Support, Analytics, ...
- `ml` - ML Engineer, Data Scientist, MLOps, Research, Ethics, ...
- `general` - Mixed panel for broad topics

### Round Orchestration

```rust
struct Round {
    number: u32,
    responses: Vec<ExpertResponse>,
    convergence_score: f64,
}

struct ExpertResponse {
    expert_id: String,
    content: String,
    position: String,       // Summary of stance
    confidence: f64,        // 0.0 - 1.0
    tensions: Vec<String>,  // Disagreements raised
    perspectives: Vec<String>, // [PERSPECTIVE Pnn: ...] markers
}

async fn run_round(
    round_num: u32,
    experts: &[Expert],
    topic: &str,
    history: &[Round],
    ollama: &OllamaClient,
) -> Round {
    let mut responses = Vec::new();

    for expert in experts {
        let prompt = build_expert_prompt(expert, topic, history, round_num);
        let response = ollama.generate(&prompt).await?;
        responses.push(parse_response(expert, response));
    }

    let convergence = calculate_convergence(&responses);
    Round { number: round_num, responses, convergence_score: convergence }
}
```

### Convergence Calculation

Convergence is measured by position alignment across experts:

```rust
fn calculate_convergence(responses: &[ExpertResponse]) -> f64 {
    // Extract positions
    let positions: Vec<&str> = responses.iter()
        .map(|r| r.position.as_str())
        .collect();

    // Cluster similar positions (semantic similarity via embeddings)
    let clusters = cluster_positions(&positions);

    // Convergence = size of largest cluster / total experts
    let largest = clusters.iter().map(|c| c.len()).max().unwrap_or(0);
    largest as f64 / responses.len() as f64
}
```

**Alternative: Confidence-weighted voting**
```rust
fn calculate_convergence(responses: &[ExpertResponse]) -> f64 {
    // Weight by confidence
    let weighted_votes = responses.iter()
        .map(|r| (r.position.clone(), r.confidence))
        .collect();

    // Group and sum weights
    // Return proportion of weight in largest group
}
```

### Dialogue Generation

After reaching convergence (or max rounds), generate the dialogue document:

```rust
fn generate_dialogue(
    topic: &str,
    constraint: Option<&str>,
    experts: &[Expert],
    rounds: &[Round],
    final_convergence: f64,
) -> String {
    let mut md = String::new();

    // Header
    md.push_str(&format!("# Alignment Dialogue: {}\n\n", topic));
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!("| **Topic** | {} |\n", topic));
    if let Some(c) = constraint {
        md.push_str(&format!("| **Constraint** | {} |\n", c));
    }
    md.push_str(&format!("| **Format** | {} experts, {} rounds |\n",
        experts.len(), rounds.len()));
    md.push_str(&format!("| **Final Convergence** | {:.0}% |\n",
        final_convergence * 100.0));

    // Expert Panel table
    md.push_str("\n## Expert Panel\n\n");
    md.push_str("| ID | Expert | Perspective |\n");
    md.push_str("|----|--------|-------------|\n");
    for e in experts {
        md.push_str(&format!("| {} | **{}** | {} |\n",
            e.id, e.name, e.perspective));
    }

    // Rounds
    for round in rounds {
        md.push_str(&format!("\n## Round {}\n\n", round.number));
        for resp in &round.responses {
            md.push_str(&format!("**{} ({}):**\n",
                resp.expert_id, get_expert_name(&resp.expert_id, experts)));
            md.push_str(&resp.content);
            md.push_str("\n\n---\n\n");
        }

        // Scoreboard
        md.push_str(&format!("### Round {} Scoreboard\n\n", round.number));
        md.push_str("| Expert | Position | Confidence |\n");
        md.push_str("|--------|----------|------------|\n");
        for resp in &round.responses {
            md.push_str(&format!("| {} | {} | {:.1} |\n",
                resp.expert_id, resp.position, resp.confidence));
        }
        md.push_str(&format!("\n**Convergence:** {:.0}%\n",
            round.convergence_score * 100.0));
    }

    // Final recommendations
    md.push_str("\n## Recommendations\n\n");
    // Extract from final round consensus

    md
}
```

### Integration with Existing Tools

```
blue_alignment_play
    │
    ├── Uses: blue-ollama (RFC 0005)
    │         └── Ollama API for LLM generation
    │
    ├── Calls: blue_dialogue_create
    │         └── Creates document record in SQLite
    │
    ├── Calls: blue_dialogue_lint
    │         └── Validates generated dialogue
    │
    └── Links: blue_rfc (if rfc_title provided)
              └── Associates dialogue with RFC
```

### CLI Usage

```bash
# Basic usage
$ blue alignment play "API versioning strategy"
Starting alignment dialogue with 12 experts...

Round 1: Gathering perspectives...
  ████████████ 12/12 experts responded
  Convergence: 42%

Round 2: Addressing tensions...
  ████████████ 12/12 experts responded
  Convergence: 67%

Round 3: Building consensus...
  ████████████ 12/12 experts responded
  Convergence: 89%

Round 4: Final positions...
  ████████████ 12/12 experts responded
  Convergence: 96%

✓ Dialogue complete at 96% convergence
  Saved: .blue/docs/dialogues/2026-01-25-api-versioning-strategy.dialogue.md

# With options
$ blue alignment play "cross-account IAM" \
    --constraint "different AWS accounts" \
    --experts 8 \
    --convergence 0.90 \
    --rfc "realm-mcp-integration"
```

### MCP Tool Definition

```json
{
  "name": "blue_alignment_play",
  "description": "Run a multi-expert alignment dialogue to deliberate on a topic until convergence",
  "inputSchema": {
    "type": "object",
    "properties": {
      "topic": {
        "type": "string",
        "description": "The topic to deliberate on"
      },
      "constraint": {
        "type": "string",
        "description": "Key constraint or boundary for the discussion"
      },
      "expert_count": {
        "type": "integer",
        "default": 12,
        "description": "Number of experts in the panel"
      },
      "convergence": {
        "type": "number",
        "default": 0.95,
        "description": "Target convergence threshold (0.0-1.0)"
      },
      "max_rounds": {
        "type": "integer",
        "default": 12,
        "description": "Maximum rounds before stopping"
      },
      "rfc_title": {
        "type": "string",
        "description": "RFC to link the dialogue to"
      },
      "template": {
        "type": "string",
        "enum": ["infrastructure", "product", "ml", "general"],
        "description": "Expert panel template"
      }
    },
    "required": ["topic"]
  }
}
```

---

## Implementation Plan

### Phase 1: Core Orchestration
- [x] Add `alignment` module to `blue-core`
- [x] Define `Expert`, `Round`, `ExpertResponse` structs
- [x] Implement `run_round()` with Ollama integration
- [x] Implement basic convergence calculation

### Phase 2: Expert Generation
- [x] Create expert panel templates (infrastructure, product, ml, governance, general)
- [ ] Implement LLM-based expert generation for custom topics
- [x] Add expert prompt templates

### Phase 3: Dialogue Output
- [x] Implement `generate_dialogue()` markdown generation
- [x] Integrate with `blue_dialogue_create` for SQLite tracking
- [ ] Add `blue_dialogue_lint` validation post-generation

### Phase 4: MCP Tool
- [x] Add `blue_alignment_play` handler to `blue-mcp`
- [ ] Add CLI subcommand `blue alignment play`
- [ ] Progress reporting during rounds

### Phase 5: Polish
- [ ] Streaming output during generation
- [ ] Interrupt handling (save partial dialogue)
- [ ] Configuration for default model/convergence

---

## Test Plan

- [x] Unit: Expert panel generation produces valid experts
- [x] Unit: Convergence calculation returns 0.0-1.0
- [ ] Unit: Dialogue markdown is valid and passes lint
- [ ] Integration: Full dialogue run with mock Ollama
- [ ] E2E: Real Ollama dialogue on simple topic
- [ ] E2E: Dialogue links correctly to RFC

---

## Open Questions (Answered from coherence-mcp ADR 0006)

1. **Parallelism**: **PARALLEL within rounds**. All N agents spawn in single message, no first-mover advantage. Sequential between rounds (each round sees previous).

2. **Memory**: Perspectives Inventory IS the summary. Long dialogues truncate to: Inventory + Last 2 rounds + Current tensions. Judge maintains continuity.

3. **Interruption**: **Save partial**. Dialogue state is in the file. Can resume by reading file, reconstructing inventories, continuing from last round.

4. **Embedding model**: Not needed. Convergence is measured by:
   - ALIGNMENT Velocity (score delta between rounds) → 0
   - All tensions resolved
   - Mutual recognition (majority of agents state convergence)

---

## Why It Stopped Working

The functionality relied on:
1. **Claude Code recognizing** "play alignment" as a trigger phrase
2. **A prompt/skill** teaching Claude the orchestration pattern
3. **ADR 0006** being in Claude's context (via CLAUDE.md)

When the codebase was migrated from coherence-mcp to blue:
- The ADR was not migrated
- No skill was created to teach Claude the pattern
- Claude no longer knows how to orchestrate alignment dialogues

## Restoration Plan

### Immediate (Copy from coherence-mcp)
1. Copy `docs/adrs/0006-alignment-dialogue-agents.md` to Blue
2. Reference it in CLAUDE.md so Claude knows the pattern
3. User says "play alignment" → Claude follows ADR 0006

### Better (Create explicit skill)
1. Create `/alignment-play` skill that encodes the orchestration
2. Skill triggers on "play alignment with N experts to X%"
3. Skill instructs Claude step-by-step on what to do

### Best (Blue MCP tool + skill)
1. `blue_alignment_play` tool that manages the full lifecycle
2. Uses Ollama for expert generation (RFC 0005)
3. Integrates with existing `blue_dialogue_*` tools
4. Saves dialogue automatically

---

*"Right then. Let's get to it."*

— Blue
