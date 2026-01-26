//! Dialogue lint tool handler
//!
//! Validates dialogue documents against the blue-dialogue-pattern.
//! Returns weighted consistency score with actionable remediation feedback.

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::error::ServerError;

/// Check severity levels with weights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical, // weight = 3
    Major,    // weight = 2
    Minor,    // weight = 1
}

impl Severity {
    fn weight(&self) -> u32 {
        match self {
            Severity::Critical => 3,
            Severity::Major => 2,
            Severity::Minor => 1,
        }
    }
}

/// Result of a single check
#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub name: &'static str,
    pub severity: Severity,
    pub pass: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_hint: Option<String>,
}

/// Parsed dialogue structure for validation
#[derive(Debug, Default)]
struct ParsedDialogue {
    // Header fields
    has_draft_link: bool,
    has_participants: bool,
    has_status: bool,
    status_value: Option<String>,

    // Scoreboard
    has_scoreboard: bool,
    scoreboard_agents: Vec<String>,
    scoreboard_totals: HashMap<String, u32>,
    claimed_total: Option<u32>,

    // Inventories
    has_perspectives_inventory: bool,
    has_tensions_tracker: bool,

    // Rounds
    rounds: Vec<u32>,

    // Markers
    perspective_ids: Vec<String>,
    tension_ids: Vec<String>,
    resolved_ids: Vec<String>,

    // For emoji consistency
    agent_emojis: HashMap<String, String>,

    // Expert panel (alignment dialogues)
    has_expert_panel: bool,
}

/// Handle blue_dialogue_lint
pub fn handle_dialogue_lint(args: &Value) -> Result<Value, ServerError> {
    let file_path_str = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let file_path = PathBuf::from(file_path_str);

    // Verify file exists
    if !file_path.exists() {
        return Err(ServerError::CommandFailed(format!(
            "Dialogue file not found: {}",
            file_path.display()
        )));
    }

    // Read file content
    let content = fs::read_to_string(&file_path)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to read file: {}", e)))?;

    // Parse dialogue structure
    let parsed = parse_dialogue(&content);

    // Run all checks
    let mut checks = Vec::new();

    // Critical checks
    checks.push(check_rounds_present(&parsed));
    checks.push(check_markers_parseable(&content));

    // Major checks
    checks.push(check_convergence_gate(&parsed));
    checks.push(check_scoreboard_present(&parsed));
    checks.push(check_inventories_present(&parsed));
    checks.push(check_id_uniqueness(&parsed));
    checks.push(check_round_sequencing(&parsed));

    // Minor checks
    checks.push(check_header_completeness(&parsed));
    checks.push(check_scoreboard_math(&parsed));
    checks.push(check_round_numbering(&parsed));
    checks.push(check_emoji_consistency(&parsed));
    checks.push(check_expert_panel(&parsed, &content));

    // Calculate weighted score
    let mut total_weight = 0u32;
    let mut earned_weight = 0u32;
    let mut checks_passed = 0usize;
    let mut checks_failed = 0usize;
    let mut critical_failures = Vec::new();

    for check in &checks {
        let weight = check.severity.weight();
        total_weight += weight;
        if check.pass {
            earned_weight += weight;
            checks_passed += 1;
        } else {
            checks_failed += 1;
            if check.severity == Severity::Critical {
                critical_failures.push(check.message.clone());
            }
        }
    }

    let score = if total_weight > 0 {
        (earned_weight as f64) / (total_weight as f64)
    } else {
        1.0
    };

    // Build hint
    let hint = if score >= 0.9 {
        format!(
            "Dialogue passes with score {:.1}% ({}/{} checks)",
            score * 100.0,
            checks_passed,
            checks_passed + checks_failed
        )
    } else if score >= 0.7 {
        format!(
            "Dialogue needs attention: {:.1}% ({} issues)",
            score * 100.0,
            checks_failed
        )
    } else if score >= 0.3 {
        format!(
            "Dialogue has significant issues: {:.1}% ({} failures)",
            score * 100.0,
            checks_failed
        )
    } else {
        format!(
            "Dialogue failing: {:.1}% - {} critical issues",
            score * 100.0,
            critical_failures.len()
        )
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Dialogue score: {:.1}%", score * 100.0),
            Some(&hint)
        ),
        "score": score,
        "checks_passed": checks_passed,
        "checks_failed": checks_failed,
        "details": checks.iter().map(|c| json!({
            "name": c.name,
            "severity": c.severity,
            "pass": c.pass,
            "message": c.message,
            "fix_hint": c.fix_hint
        })).collect::<Vec<_>>(),
        "critical_failures": critical_failures
    }))
}

/// Parse dialogue content into structured form
fn parse_dialogue(content: &str) -> ParsedDialogue {
    let mut parsed = ParsedDialogue::default();

    // Header patterns (case-insensitive, whitespace-tolerant)
    let draft_re = Regex::new(r"(?i)\*\*Draft\*\*:").unwrap();
    let participants_re = Regex::new(r"(?i)\*\*Participants\*\*:").unwrap();
    let status_re = Regex::new(r"(?i)\*\*Status\*\*:\s*(.+)").unwrap();

    parsed.has_draft_link = draft_re.is_match(content);
    parsed.has_participants = participants_re.is_match(content);

    if let Some(caps) = status_re.captures(content) {
        parsed.has_status = true;
        parsed.status_value = Some(caps[1].trim().to_string());
    }

    // Scoreboard detection
    let scoreboard_re = Regex::new(r"(?i)##\s*Alignment\s+Scoreboard").unwrap();
    parsed.has_scoreboard = scoreboard_re.is_match(content);

    // Parse scoreboard table for agents and totals
    let table_row_re =
        Regex::new(r"\|\s*([🧁💙]?\s*\w+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*\*\*(\d+)\*\*\s*\|").unwrap();
    for caps in table_row_re.captures_iter(content) {
        let agent = caps[1].trim().to_string();
        let w: u32 = caps[2].parse().unwrap_or(0);
        let c: u32 = caps[3].parse().unwrap_or(0);
        let t: u32 = caps[4].parse().unwrap_or(0);
        let r: u32 = caps[5].parse().unwrap_or(0);
        let total: u32 = caps[6].parse().unwrap_or(0);

        parsed.scoreboard_agents.push(agent.clone());
        parsed.scoreboard_totals.insert(agent, w + c + t + r);
        parsed.claimed_total = Some(total);
    }

    // Total ALIGNMENT line
    let total_alignment_re = Regex::new(r"(?i)\*\*Total\s+ALIGNMENT\*\*:\s*(\d+)").unwrap();
    if let Some(caps) = total_alignment_re.captures(content) {
        parsed.claimed_total = caps[1].parse().ok();
    }

    // Inventories
    let perspectives_re = Regex::new(r"(?i)##\s*Perspectives\s+Inventory").unwrap();
    let tensions_re = Regex::new(r"(?i)##\s*Tensions\s+Tracker").unwrap();
    parsed.has_perspectives_inventory = perspectives_re.is_match(content);
    parsed.has_tensions_tracker = tensions_re.is_match(content);

    // Expert panel
    let expert_panel_re = Regex::new(r"(?i)##\s*Expert\s+Panel").unwrap();
    parsed.has_expert_panel = expert_panel_re.is_match(content);

    // Rounds (case-insensitive)
    let round_re = Regex::new(r"(?i)##\s*Round\s+(\d+)").unwrap();
    for caps in round_re.captures_iter(content) {
        if let Ok(n) = caps[1].parse::<u32>() {
            parsed.rounds.push(n);
        }
    }
    parsed.rounds.sort();

    // Agent headers within rounds
    let agent_re = Regex::new(r"###\s*(\w+)\s*([🧁💙]?)").unwrap();
    for caps in agent_re.captures_iter(content) {
        let agent = caps[1].to_string();
        let emoji = caps
            .get(2)
            .map(|m: regex::Match| m.as_str().to_string())
            .unwrap_or_default();
        if !emoji.is_empty() {
            parsed.agent_emojis.insert(agent.clone(), emoji);
        }
    }

    // Perspective markers (case-insensitive, whitespace-tolerant)
    let perspective_marker_re = Regex::new(r"(?i)\[\s*PERSPECTIVE\s+P(\d{2})\s*:").unwrap();
    for caps in perspective_marker_re.captures_iter(content) {
        parsed.perspective_ids.push(format!("P{}", &caps[1]));
    }

    // Tension markers
    let tension_marker_re = Regex::new(r"(?i)\[\s*TENSION\s+T(\d+)\s*:").unwrap();
    for caps in tension_marker_re.captures_iter(content) {
        parsed.tension_ids.push(format!("T{}", &caps[1]));
    }

    // Resolved markers
    let resolved_marker_re = Regex::new(r"(?i)\[\s*RESOLVED\s+T(\d+)").unwrap();
    for caps in resolved_marker_re.captures_iter(content) {
        parsed.resolved_ids.push(format!("T{}", &caps[1]));
    }

    parsed
}

// ===== CRITICAL CHECKS =====

fn check_rounds_present(parsed: &ParsedDialogue) -> CheckResult {
    let pass = !parsed.rounds.is_empty();
    CheckResult {
        name: "rounds-present",
        severity: Severity::Critical,
        pass,
        message: if pass {
            format!("Found {} round(s)", parsed.rounds.len())
        } else {
            "No rounds found in dialogue".to_string()
        },
        fix_hint: if pass {
            None
        } else {
            Some("Add at least one '## Round N' section with agent responses".to_string())
        },
    }
}

fn check_markers_parseable(content: &str) -> CheckResult {
    // Check for malformed markers that might indicate parsing issues
    let malformed_perspective = Regex::new(r"\[PERSPECTIV[^E]").unwrap();
    let malformed_tension = Regex::new(r"\[TENSIO[^N]").unwrap();

    let has_malformed =
        malformed_perspective.is_match(content) || malformed_tension.is_match(content);

    CheckResult {
        name: "markers-parseable",
        severity: Severity::Critical,
        pass: !has_malformed,
        message: if has_malformed {
            "Found potentially malformed markers".to_string()
        } else {
            "All markers appear well-formed".to_string()
        },
        fix_hint: if has_malformed {
            Some("Check spelling: [PERSPECTIVE Pnn: ...] and [TENSION Tn: ...]".to_string())
        } else {
            None
        },
    }
}

// ===== MAJOR CHECKS =====

fn check_convergence_gate(parsed: &ParsedDialogue) -> CheckResult {
    // Only applies if status indicates convergence
    let is_converged = parsed
        .status_value
        .as_ref()
        .map(|s| s.to_lowercase().contains("converge"))
        .unwrap_or(false);

    if !is_converged {
        return CheckResult {
            name: "convergence-gate",
            severity: Severity::Major,
            pass: true,
            message: "Not converged yet, gate not applicable".to_string(),
            fix_hint: None,
        };
    }

    // Check all tensions have matching resolved
    let tension_set: HashSet<_> = parsed.tension_ids.iter().collect();
    let resolved_set: HashSet<_> = parsed.resolved_ids.iter().collect();

    let unresolved: Vec<_> = tension_set
        .difference(&resolved_set)
        .map(|s| s.as_str())
        .collect();

    let pass = unresolved.is_empty();

    CheckResult {
        name: "convergence-gate",
        severity: Severity::Major,
        pass,
        message: if pass {
            "All tensions resolved before convergence".to_string()
        } else {
            format!("Unresolved tensions: {}", unresolved.join(", "))
        },
        fix_hint: if pass {
            None
        } else {
            Some(format!(
                "Add [RESOLVED {}] markers for each unresolved tension",
                unresolved.join(", ")
            ))
        },
    }
}

fn check_scoreboard_present(parsed: &ParsedDialogue) -> CheckResult {
    CheckResult {
        name: "scoreboard-present",
        severity: Severity::Major,
        pass: parsed.has_scoreboard,
        message: if parsed.has_scoreboard {
            "Scoreboard section found".to_string()
        } else {
            "Missing '## Alignment Scoreboard' section".to_string()
        },
        fix_hint: if parsed.has_scoreboard {
            None
        } else {
            Some("Add '## Alignment Scoreboard' section with W/C/T/R columns".to_string())
        },
    }
}

fn check_inventories_present(parsed: &ParsedDialogue) -> CheckResult {
    let has_both = parsed.has_perspectives_inventory && parsed.has_tensions_tracker;
    let missing = match (
        parsed.has_perspectives_inventory,
        parsed.has_tensions_tracker,
    ) {
        (false, false) => "Perspectives Inventory, Tensions Tracker",
        (false, true) => "Perspectives Inventory",
        (true, false) => "Tensions Tracker",
        (true, true) => "",
    };

    CheckResult {
        name: "inventories-present",
        severity: Severity::Major,
        pass: has_both,
        message: if has_both {
            "Both inventory sections present".to_string()
        } else {
            format!("Missing: {}", missing)
        },
        fix_hint: if has_both {
            None
        } else {
            Some(format!("Add '## {}' section(s)", missing))
        },
    }
}

fn check_id_uniqueness(parsed: &ParsedDialogue) -> CheckResult {
    let mut perspective_seen: HashSet<String> = HashSet::new();
    let mut tension_seen: HashSet<String> = HashSet::new();
    let mut duplicates = Vec::new();

    for id in &parsed.perspective_ids {
        if !perspective_seen.insert(id.clone()) {
            duplicates.push(id.clone());
        }
    }
    for id in &parsed.tension_ids {
        if !tension_seen.insert(id.clone()) {
            duplicates.push(id.clone());
        }
    }

    let pass = duplicates.is_empty();

    CheckResult {
        name: "id-uniqueness",
        severity: Severity::Major,
        pass,
        message: if pass {
            "All perspective/tension IDs are unique".to_string()
        } else {
            format!("Duplicate IDs: {}", duplicates.join(", "))
        },
        fix_hint: if pass {
            None
        } else {
            Some("Renumber duplicate IDs to be unique".to_string())
        },
    }
}

fn check_round_sequencing(parsed: &ParsedDialogue) -> CheckResult {
    if parsed.rounds.is_empty() {
        return CheckResult {
            name: "round-sequencing",
            severity: Severity::Major,
            pass: false,
            message: "No rounds to check".to_string(),
            fix_hint: Some("Add '## Round 1' section".to_string()),
        };
    }

    // Check rounds are sequential starting from 1
    let expected: Vec<u32> = (1..=parsed.rounds.len() as u32).collect();
    let pass = parsed.rounds == expected;

    CheckResult {
        name: "round-sequencing",
        severity: Severity::Major,
        pass,
        message: if pass {
            format!("Rounds 1-{} sequential", parsed.rounds.len())
        } else {
            format!(
                "Round sequence gap: found {:?}, expected {:?}",
                parsed.rounds, expected
            )
        },
        fix_hint: if pass {
            None
        } else {
            Some("Renumber rounds sequentially starting from 1".to_string())
        },
    }
}

// ===== MINOR CHECKS =====

fn check_header_completeness(parsed: &ParsedDialogue) -> CheckResult {
    let missing: Vec<&str> = [
        (!parsed.has_draft_link, "Draft"),
        (!parsed.has_participants, "Participants"),
        (!parsed.has_status, "Status"),
    ]
    .iter()
    .filter_map(|(missing, name)| if *missing { Some(*name) } else { None })
    .collect();

    let pass = missing.is_empty();

    CheckResult {
        name: "header-completeness",
        severity: Severity::Minor,
        pass,
        message: if pass {
            "All header fields present".to_string()
        } else {
            format!("Missing header fields: {}", missing.join(", "))
        },
        fix_hint: if pass {
            None
        } else {
            Some(format!(
                "Add **{}**: fields to header",
                missing.join("**, **")
            ))
        },
    }
}

fn check_scoreboard_math(parsed: &ParsedDialogue) -> CheckResult {
    if !parsed.has_scoreboard || parsed.scoreboard_totals.is_empty() {
        return CheckResult {
            name: "scoreboard-math",
            severity: Severity::Minor,
            pass: true,
            message: "No scoreboard to verify".to_string(),
            fix_hint: None,
        };
    }

    // Sum up all agent totals
    let computed_total: u32 = parsed.scoreboard_totals.values().sum();
    let claimed = parsed.claimed_total.unwrap_or(0);

    // Allow some tolerance for parsing issues
    let pass = (computed_total as i32 - claimed as i32).abs() <= 2;

    CheckResult {
        name: "scoreboard-math",
        severity: Severity::Minor,
        pass,
        message: if pass {
            format!("Total ALIGNMENT: {}", claimed)
        } else {
            format!(
                "Math mismatch: claimed {}, computed {}",
                claimed, computed_total
            )
        },
        fix_hint: if pass {
            None
        } else {
            Some(format!(
                "Update **Total ALIGNMENT**: {} to match sum of agent scores",
                computed_total
            ))
        },
    }
}

fn check_round_numbering(parsed: &ParsedDialogue) -> CheckResult {
    if parsed.rounds.is_empty() {
        return CheckResult {
            name: "round-numbering",
            severity: Severity::Minor,
            pass: true,
            message: "No rounds to check".to_string(),
            fix_hint: None,
        };
    }

    let starts_at_one = parsed.rounds.first() == Some(&1);

    CheckResult {
        name: "round-numbering",
        severity: Severity::Minor,
        pass: starts_at_one,
        message: if starts_at_one {
            "Rounds start at 1".to_string()
        } else {
            format!("Rounds don't start at 1: {:?}", parsed.rounds)
        },
        fix_hint: if starts_at_one {
            None
        } else {
            Some("Start round numbering at 1".to_string())
        },
    }
}

fn check_emoji_consistency(parsed: &ParsedDialogue) -> CheckResult {
    let has_emojis = !parsed.agent_emojis.is_empty();

    CheckResult {
        name: "emoji-consistency",
        severity: Severity::Minor,
        pass: has_emojis,
        message: if has_emojis {
            format!("Found {} agents with emoji", parsed.agent_emojis.len())
        } else {
            "No agent emojis found".to_string()
        },
        fix_hint: if has_emojis {
            None
        } else {
            Some("Add emoji to agent headers: ### Muffin 🧁".to_string())
        },
    }
}

/// Check for Expert Panel section (alignment dialogues only).
/// Only fires when "Alignment Scoreboard" is present (indicating alignment mode).
fn check_expert_panel(parsed: &ParsedDialogue, content: &str) -> CheckResult {
    let is_alignment = content.contains("Alignment Scoreboard");

    if !is_alignment {
        return CheckResult {
            name: "expert-panel",
            severity: Severity::Minor,
            pass: true,
            message: "Not an alignment dialogue, expert panel not required".to_string(),
            fix_hint: None,
        };
    }

    CheckResult {
        name: "expert-panel",
        severity: Severity::Minor,
        pass: parsed.has_expert_panel,
        message: if parsed.has_expert_panel {
            "Expert Panel section present".to_string()
        } else {
            "Alignment dialogue missing '## Expert Panel' section".to_string()
        },
        fix_hint: if parsed.has_expert_panel {
            None
        } else {
            Some("Add '## Expert Panel' table with Agent/Role/Emoji columns".to_string())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dialogue_rounds() {
        let content = r#"
## Round 1
### Muffin 🧁
Some content
## Round 2
### Cupcake 🧁
More content
"#;
        let parsed = parse_dialogue(content);
        assert_eq!(parsed.rounds, vec![1, 2]);
    }

    #[test]
    fn test_check_rounds_present_pass() {
        let mut parsed = ParsedDialogue::default();
        parsed.rounds = vec![1, 2];
        let result = check_rounds_present(&parsed);
        assert!(result.pass);
    }

    #[test]
    fn test_check_rounds_present_fail() {
        let parsed = ParsedDialogue::default();
        let result = check_rounds_present(&parsed);
        assert!(!result.pass);
        assert!(result.fix_hint.is_some());
    }
}
