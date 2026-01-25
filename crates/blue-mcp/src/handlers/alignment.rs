//! Alignment Dialogue Orchestration Handler
//!
//! Implements RFC 0012: blue_alignment_play
//! Uses local Ollama to run multi-expert deliberation until convergence.

use std::fs;
use std::path::PathBuf;

use blue_core::{
    AlignmentDialogue, DialogueStatus, DocType, Document, ExpertResponse,
    LinkType, PanelTemplate, Perspective, ProjectState, Round,
    Tension, TensionStatus, build_expert_prompt, parse_expert_response, CompletionOptions,
};
use blue_ollama::{EmbeddedOllama, HealthStatus};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Default model for alignment dialogues
const DEFAULT_MODEL: &str = "qwen2.5:7b";

/// Handle blue_alignment_play
///
/// Run a multi-expert alignment dialogue to deliberate on a topic until convergence.
pub fn handle_play(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let topic = args
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let constraint = args.get("constraint").and_then(|v| v.as_str());
    let expert_count = args
        .get("expert_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(12) as usize;
    let convergence = args
        .get("convergence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.95);
    let max_rounds = args
        .get("max_rounds")
        .and_then(|v| v.as_u64())
        .unwrap_or(12) as u32;
    let rfc_title = args.get("rfc_title").and_then(|v| v.as_str());
    let template = args.get("template").and_then(|v| v.as_str());
    let model = args
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_MODEL);

    // Validate RFC exists if provided
    let _rfc_doc = if let Some(rfc) = rfc_title {
        Some(
            state
                .store
                .find_document(DocType::Rfc, rfc)
                .map_err(|_| ServerError::NotFound(format!("RFC '{}' not found", rfc)))?,
        )
    } else {
        None
    };

    // Get Ollama instance
    let ollama_config = blue_core::LocalLlmConfig {
        use_external: true,
        model: model.to_string(),
        ..Default::default()
    };
    let ollama = EmbeddedOllama::new(&ollama_config);

    // Verify Ollama is running
    if !ollama.is_ollama_running() {
        return Err(ServerError::CommandFailed(
            "Ollama not running. Start it with blue_llm_start or run 'ollama serve'.".to_string(),
        ));
    }

    // Check health
    match ollama.health_check() {
        HealthStatus::Healthy { .. } => {}
        HealthStatus::Unhealthy { error } => {
            return Err(ServerError::CommandFailed(format!(
                "Ollama unhealthy: {}",
                error
            )));
        }
        HealthStatus::NotRunning => {
            return Err(ServerError::CommandFailed("Ollama not running.".to_string()));
        }
    }

    // Generate expert panel based on template
    let panel_template = match template {
        Some("infrastructure") => PanelTemplate::Infrastructure,
        Some("product") => PanelTemplate::Product,
        Some("ml") => PanelTemplate::MachineLearning,
        Some("governance") => PanelTemplate::Governance,
        _ => PanelTemplate::General,
    };

    let mut experts = panel_template.generate_experts(expert_count);

    // Make sure we don't exceed requested count
    if experts.len() > expert_count {
        experts.truncate(expert_count);
    }

    // Create dialogue
    let mut dialogue = AlignmentDialogue::new(
        topic.to_string(),
        constraint.map(String::from),
        experts.clone(),
    );
    dialogue.convergence_threshold = convergence;
    dialogue.max_rounds = max_rounds;
    dialogue.rfc_title = rfc_title.map(String::from);

    // Completion options for expert responses
    let options = CompletionOptions {
        max_tokens: 2048,
        temperature: 0.8,
        stop_sequences: vec!["---".to_string()],
    };

    // Run rounds
    let mut round_num = 0;
    let mut previous_score = 0u32;

    loop {
        round_num += 1;

        // Check max rounds
        if round_num > max_rounds {
            dialogue.status = DialogueStatus::MaxRoundsReached;
            break;
        }

        // Run one round - need to pass copies/references that don't conflict
        let (round, new_perspectives, new_tensions) = run_round(
            &ollama,
            model,
            &options,
            &dialogue.topic,
            dialogue.constraint.as_deref(),
            &dialogue.experts,
            &dialogue.rounds,
            round_num,
            dialogue.perspectives.len(),
            dialogue.tensions.len(),
        )?;

        // Merge new perspectives and tensions
        dialogue.perspectives.extend(new_perspectives);
        for tension in new_tensions {
            dialogue.tensions.push(tension);
        }

        // Calculate velocity
        let velocity = (round.total_score as i32) - (previous_score as i32);
        previous_score = round.total_score;

        // Check convergence conditions:
        // 1. Convergence threshold met
        // 2. Velocity approaching zero (less than 2 points gained)
        // 3. All tensions resolved
        let tensions_resolved = dialogue.tensions.is_empty() || dialogue.tensions.iter().all(|t| t.status == TensionStatus::Resolved);
        let velocity_stable = velocity.abs() < 2 && round_num > 2;

        dialogue.rounds.push(round);

        if dialogue.rounds.last().map(|r| r.convergence).unwrap_or(0.0) >= convergence {
            dialogue.status = DialogueStatus::Converged;
            break;
        }

        if velocity_stable && tensions_resolved && round_num > 3 {
            dialogue.status = DialogueStatus::Converged;
            break;
        }
    }

    // Generate and save dialogue markdown
    let markdown = generate_dialogue_markdown(&dialogue);
    let dialogue_path = save_dialogue(state, &dialogue, &markdown)?;

    // Get final stats
    let final_convergence = dialogue.rounds.last().map(|r| r.convergence).unwrap_or(0.0);
    let total_rounds = dialogue.rounds.len();

    let hint = match dialogue.status {
        DialogueStatus::Converged => format!(
            "Reached {:.0}% convergence in {} rounds.",
            final_convergence * 100.0,
            total_rounds
        ),
        DialogueStatus::MaxRoundsReached => format!(
            "Stopped after {} rounds at {:.0}% convergence.",
            total_rounds,
            final_convergence * 100.0
        ),
        _ => "Dialogue interrupted.".to_string(),
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Alignment dialogue complete: {}", topic),
            Some(&hint)
        ),
        "dialogue": {
            "topic": topic,
            "constraint": constraint,
            "file": dialogue_path.display().to_string(),
            "rounds": total_rounds,
            "final_convergence": final_convergence,
            "status": format!("{:?}", dialogue.status).to_lowercase(),
            "expert_count": experts.len(),
            "perspectives_surfaced": dialogue.perspectives.len(),
            "tensions_resolved": dialogue.tensions.iter().filter(|t| t.status == TensionStatus::Resolved).count(),
            "linked_rfc": rfc_title,
        },
        "expert_panel": experts.iter().map(|e| json!({
            "id": e.id,
            "name": e.name,
            "tier": format!("{:?}", e.tier).to_lowercase(),
        })).collect::<Vec<_>>(),
    }))
}

/// Build a summary of previous rounds for the prompt
fn summarize_previous_rounds(rounds: &[Round]) -> String {
    if rounds.is_empty() {
        return String::new();
    }

    let mut summary = String::new();
    for round in rounds {
        summary.push_str(&format!("\n## Round {} Summary\n", round.number));
        summary.push_str(&format!("Convergence: {:.0}%\n", round.convergence * 100.0));

        for resp in &round.responses {
            summary.push_str(&format!(
                "\n**{}**: {} (confidence: {:.1})\n",
                resp.expert_id, resp.position, resp.confidence
            ));
        }
    }
    summary
}

/// Run a single round of dialogue
/// Returns (Round, new_perspectives, new_tensions)
fn run_round(
    ollama: &EmbeddedOllama,
    model: &str,
    options: &CompletionOptions,
    topic: &str,
    constraint: Option<&str>,
    experts: &[blue_core::Expert],
    previous_rounds: &[Round],
    round_num: u32,
    perspective_offset: usize,
    tension_offset: usize,
) -> Result<(Round, Vec<Perspective>, Vec<Tension>), ServerError> {
    let mut responses = Vec::new();
    let mut round_score = 0u32;
    let mut new_perspectives = Vec::new();
    let mut new_tensions = Vec::new();

    // Build summary of previous rounds
    let previous_summary = summarize_previous_rounds(previous_rounds);

    for expert in experts {
        // Build prompt for this expert
        let prompt = build_expert_prompt(
            expert,
            topic,
            constraint,
            round_num,
            &previous_summary,
        );

        // Generate response
        let result = ollama
            .generate(model, &prompt, options)
            .map_err(|e| ServerError::CommandFailed(format!("LLM generation failed: {}", e)))?;

        // Parse response
        let mut response = parse_expert_response(&expert.id, &result.text);

        // Track new perspectives
        let local_perspective_offset = perspective_offset + new_perspectives.len();
        for (i, p) in response.perspectives.iter_mut().enumerate() {
            p.id = format!("P{:02}", local_perspective_offset + i + 1);
            p.round = round_num;
            new_perspectives.push(p.clone());
        }

        // Track new tensions
        let local_tension_offset = tension_offset + new_tensions.len();
        for (i, t) in response.tensions.iter_mut().enumerate() {
            t.id = format!("T{}", local_tension_offset + i + 1);
            new_tensions.push(t.clone());
        }

        round_score += response.score.total();
        responses.push(response);
    }

    // Calculate convergence based on position similarity
    let convergence = calculate_convergence(&responses);

    // Calculate velocity
    let previous_total = previous_rounds.last().map(|r| r.total_score).unwrap_or(0);
    let velocity = (round_score as i32) - (previous_total as i32);

    Ok((
        Round {
            number: round_num,
            responses,
            total_score: round_score,
            velocity,
            convergence,
        },
        new_perspectives,
        new_tensions,
    ))
}

/// Calculate convergence based on position alignment
fn calculate_convergence(responses: &[ExpertResponse]) -> f64 {
    if responses.is_empty() {
        return 0.0;
    }

    // Use confidence-weighted position clustering
    // High confidence experts have more weight in determining convergence
    let high_confidence: Vec<_> = responses
        .iter()
        .filter(|r| r.confidence >= 0.7)
        .collect();

    if high_confidence.is_empty() {
        return 0.3; // Base convergence if no one is confident yet
    }

    // Group by position similarity using first 30 chars as key
    let mut position_groups: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for response in &high_confidence {
        let key: String = response.position.chars().take(30).collect::<String>().to_lowercase();
        *position_groups.entry(key).or_insert(0) += 1;
    }

    let largest_group = position_groups.values().max().copied().unwrap_or(0);
    largest_group as f64 / responses.len() as f64
}

/// Generate dialogue markdown
fn generate_dialogue_markdown(dialogue: &AlignmentDialogue) -> String {
    let mut md = String::new();

    // Title
    md.push_str(&format!("# Alignment Dialogue: {}\n\n", dialogue.topic));

    // Metadata
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!("| **Topic** | {} |\n", dialogue.topic));
    if let Some(ref c) = dialogue.constraint {
        md.push_str(&format!("| **Constraint** | {} |\n", c));
    }
    md.push_str(&format!(
        "| **Format** | {} experts, {} rounds |\n",
        dialogue.experts.len(),
        dialogue.rounds.len()
    ));
    let final_conv = dialogue.rounds.last().map(|r| r.convergence).unwrap_or(0.0);
    md.push_str(&format!(
        "| **Final Convergence** | {:.0}% |\n",
        final_conv * 100.0
    ));
    md.push_str(&format!(
        "| **Status** | {:?} |\n",
        dialogue.status
    ));
    if let Some(ref rfc) = dialogue.rfc_title {
        md.push_str(&format!("| **RFC** | {} |\n", rfc));
    }
    md.push_str("\n---\n\n");

    // Expert Panel
    md.push_str("## Expert Panel\n\n");
    md.push_str("| ID | Expert | Tier | Perspective |\n");
    md.push_str("|----|--------|------|-------------|\n");
    for e in &dialogue.experts {
        md.push_str(&format!(
            "| {} | **{}** | {:?} | {} |\n",
            e.id, e.name, e.tier, e.perspective
        ));
    }
    md.push_str("\n");

    // Perspectives Inventory
    if !dialogue.perspectives.is_empty() {
        md.push_str("## Perspectives Inventory\n\n");
        md.push_str("| ID | Description | Surfaced By | Round | Status |\n");
        md.push_str("|----|-------------|-------------|-------|--------|\n");
        for p in &dialogue.perspectives {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {:?} |\n",
                p.id, p.description, p.surfaced_by, p.round, p.status
            ));
        }
        md.push_str("\n");
    }

    // Tensions
    if !dialogue.tensions.is_empty() {
        md.push_str("## Tensions\n\n");
        md.push_str("| ID | Description | Status |\n");
        md.push_str("|----|-------------|--------|\n");
        for t in &dialogue.tensions {
            md.push_str(&format!(
                "| {} | {} | {:?} |\n",
                t.id, t.description, t.status
            ));
        }
        md.push_str("\n");
    }

    // Rounds
    for round in &dialogue.rounds {
        md.push_str(&format!("## Round {}\n\n", round.number));

        for resp in &round.responses {
            let expert = dialogue.experts.iter().find(|e| e.id == resp.expert_id);
            let name = expert.map(|e| e.name.as_str()).unwrap_or(&resp.expert_id);
            md.push_str(&format!("### {} ({})\n\n", name, resp.expert_id));
            md.push_str(&resp.content);
            md.push_str("\n\n");
        }

        // Round scoreboard
        md.push_str(&format!("### Round {} Scoreboard\n\n", round.number));
        md.push_str("| Expert | Position | Confidence | ALIGNMENT |\n");
        md.push_str("|--------|----------|------------|----------|\n");
        for resp in &round.responses {
            let position_display = if resp.position.len() > 40 {
                format!("{}...", &resp.position[..40])
            } else {
                resp.position.clone()
            };
            md.push_str(&format!(
                "| {} | {} | {:.1} | {} |\n",
                resp.expert_id,
                position_display,
                resp.confidence,
                resp.score.total()
            ));
        }
        md.push_str(&format!(
            "\n**Convergence:** {:.0}% | **Velocity:** {:+} | **Total ALIGNMENT:** {}\n\n",
            round.convergence * 100.0,
            round.velocity,
            round.total_score
        ));
    }

    // Recommendations (extracted from final round consensus)
    md.push_str("## Recommendations\n\n");
    if let Some(final_round) = dialogue.rounds.last() {
        // Take top 3 positions by confidence
        let mut sorted_responses = final_round.responses.clone();
        sorted_responses.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        for (i, resp) in sorted_responses.iter().take(3).enumerate() {
            md.push_str(&format!("{}. **{}**: {}\n", i + 1, resp.expert_id, resp.position));
        }
    } else {
        md.push_str("*No rounds completed.*\n");
    }

    md.push_str("\n---\n\n");
    md.push_str("*Generated by Blue Alignment Dialogue Orchestration (RFC 0012)*\n");

    md
}

/// Save dialogue to file and SQLite
fn save_dialogue(
    state: &mut ProjectState,
    dialogue: &AlignmentDialogue,
    markdown: &str,
) -> Result<PathBuf, ServerError> {
    // Get next dialogue number
    let dialogue_number = state
        .store
        .next_number(DocType::Dialogue)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Generate file path
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_name = format!(
        "{}-{}.dialogue.md",
        date,
        to_kebab_case(&dialogue.topic)
    );
    let file_path = PathBuf::from("dialogues").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let dialogue_path = docs_path.join(&file_path);

    // Create document in SQLite
    let mut doc = Document::new(DocType::Dialogue, &dialogue.topic, "recorded");
    doc.number = Some(dialogue_number);
    doc.file_path = Some(file_path.to_string_lossy().to_string());

    let dialogue_id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Link to RFC if provided
    if let Some(ref rfc_title) = dialogue.rfc_title {
        if let Ok(rfc_doc) = state.store.find_document(DocType::Rfc, rfc_title) {
            if let (Some(rfc_id), Some(did)) = (rfc_doc.id, Some(dialogue_id)) {
                let _ = state.store.link_documents(did, rfc_id, LinkType::DialogueToRfc);
            }
        }
    }

    // Create dialogues directory if needed
    if let Some(parent) = dialogue_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }

    // Write file
    fs::write(&dialogue_path, markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    Ok(dialogue_path)
}

/// Convert string to kebab-case
fn to_kebab_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use blue_core::AlignmentScore;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("API Versioning Strategy"), "api-versioning-strategy");
        assert_eq!(to_kebab_case("Cross-Account IAM"), "cross-account-iam");
    }

    #[test]
    fn test_calculate_convergence_single() {
        let responses = vec![ExpertResponse {
            expert_id: "DS".to_string(),
            content: String::new(),
            position: "Use semantic versioning".to_string(),
            confidence: 0.8,
            perspectives: Vec::new(),
            tensions: Vec::new(),
            refinements: Vec::new(),
            concessions: Vec::new(),
            resolved_tensions: Vec::new(),
            score: AlignmentScore::default(),
        }];

        let conv = calculate_convergence(&responses);
        assert!((conv - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_convergence_empty() {
        let responses: Vec<ExpertResponse> = Vec::new();
        let conv = calculate_convergence(&responses);
        assert!((conv - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_summarize_empty_rounds() {
        let rounds: Vec<Round> = Vec::new();
        let summary = summarize_previous_rounds(&rounds);
        assert!(summary.is_empty());
    }
}
