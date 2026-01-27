//! Interactive Onboarding Guide
//!
//! Provides an interactive tutorial for new Blue users.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Guide progress tracking
#[derive(Debug, Serialize, Deserialize, Default)]
struct GuideProgress {
    started_at: Option<String>,
    completed_sections: Vec<String>,
    current_section: String,
    completed_at: Option<String>,
    skipped: bool,
}

/// Guide sections in order
const SECTIONS: &[&str] = &[
    "intro",
    "workflow",
    "documents",
    "implementation",
    "ready",
];

/// Handle blue_guide
pub fn handle_guide(args: &Value, blue_path: &Path) -> Result<Value, ServerError> {
    let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("resume");

    let progress_path = blue_path.join("onboarding.json");
    let mut progress = load_progress(&progress_path);

    match action {
        "start" => {
            progress = GuideProgress {
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                completed_sections: vec![],
                current_section: "intro".to_string(),
                completed_at: None,
                skipped: false,
            };
            save_progress(&progress_path, &progress)?;
            render_section("intro", &progress)
        }
        "resume" => {
            if progress.started_at.is_none() {
                progress = GuideProgress {
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    completed_sections: vec![],
                    current_section: "intro".to_string(),
                    completed_at: None,
                    skipped: false,
                };
                save_progress(&progress_path, &progress)?;
            }
            render_section(&progress.current_section.clone(), &progress)
        }
        "next" => {
            if !progress.completed_sections.contains(&progress.current_section) {
                progress.completed_sections.push(progress.current_section.clone());
            }

            let current_idx = SECTIONS
                .iter()
                .position(|&s| s == progress.current_section)
                .unwrap_or(0);

            if current_idx + 1 < SECTIONS.len() {
                progress.current_section = SECTIONS[current_idx + 1].to_string();
                save_progress(&progress_path, &progress)?;
                render_section(&progress.current_section.clone(), &progress)
            } else {
                progress.completed_at = Some(chrono::Utc::now().to_rfc3339());
                save_progress(&progress_path, &progress)?;
                render_completion()
            }
        }
        "skip" => {
            progress.skipped = true;
            progress.completed_at = Some(chrono::Utc::now().to_rfc3339());
            save_progress(&progress_path, &progress)?;

            Ok(json!({
                "status": "success",
                "message": blue_core::voice::info(
                    "Guide skipped",
                    Some("Use blue_guide action='start' anytime to restart")
                ),
                "skipped": true
            }))
        }
        "reset" => {
            if progress_path.exists() {
                fs::remove_file(&progress_path).ok();
            }

            Ok(json!({
                "status": "success",
                "message": blue_core::voice::success(
                    "Guide reset",
                    Some("Use blue_guide action='start' to begin fresh")
                )
            }))
        }
        "status" => {
            let total = SECTIONS.len();
            let completed = progress.completed_sections.len();
            let percentage = if total > 0 { (completed * 100) / total } else { 0 };

            Ok(json!({
                "status": "success",
                "message": blue_core::voice::info(
                    &format!("Guide {}% complete", percentage),
                    if progress.completed_at.is_some() {
                        Some("Guide completed!")
                    } else if progress.started_at.is_some() {
                        Some("Use blue_guide action='resume' to continue")
                    } else {
                        Some("Use blue_guide action='start' to begin")
                    }
                ),
                "started": progress.started_at.is_some(),
                "completed": progress.completed_at.is_some(),
                "skipped": progress.skipped,
                "current_section": progress.current_section,
                "progress": {
                    "completed": completed,
                    "total": total,
                    "percentage": percentage
                }
            }))
        }
        _ => Err(ServerError::InvalidParams),
    }
}

fn load_progress(path: &Path) -> GuideProgress {
    if path.exists() {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        GuideProgress::default()
    }
}

fn save_progress(path: &Path, progress: &GuideProgress) -> Result<(), ServerError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(progress)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    fs::write(path, json).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    Ok(())
}

fn render_section(section: &str, progress: &GuideProgress) -> Result<Value, ServerError> {
    let content = get_section_content(section);
    let current_idx = SECTIONS.iter().position(|&s| s == section).unwrap_or(0);
    let total = SECTIONS.len();

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Guide section {}/{}", current_idx + 1, total),
            Some("Read the content and use blue_guide action='next' to continue")
        ),
        "section": section,
        "content": content,
        "progress": {
            "current": current_idx + 1,
            "total": total,
            "completed_sections": progress.completed_sections
        }
    }))
}

fn render_completion() -> Result<Value, ServerError> {
    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            "Guide complete!",
            Some("You're ready to start working with Blue")
        ),
        "section": "complete",
        "content": r#"
═══════════════════════════════════════════════════════════════════════════
                         🎓 ONBOARDING COMPLETE
═══════════════════════════════════════════════════════════════════════════

Right then. You've got the basics down.

Quick reference:
  • "What's the status?" → blue_status
  • "What should I work on?" → blue_next
  • "I have an idea" → Describe it, I'll suggest PRD/RFC/Spike

Remember:
  • PRDs define WHAT to build (requirements)
  • RFCs define HOW to build it (design)
  • Spikes investigate questions (research)

Off you go! 🐑

— Blue
"#
    }))
}

fn get_section_content(section: &str) -> &'static str {
    match section {
        "intro" => r#"
═══════════════════════════════════════════════════════════════════════════
                         👋 HELLO THERE
═══════════════════════════════════════════════════════════════════════════

I'm Blue, your project management companion.

I help you:
  📝  Track what needs to be built (PRDs)
  📋  Design how to build it (RFCs)
  🔍  Investigate questions (Spikes)
  ✅  Verify work is complete (PRs)

Think of me as your friendly project shepherd. I keep track of everything
so nothing falls through the cracks.

Ready to learn the workflow?
"#,
        "workflow" => r#"
═══════════════════════════════════════════════════════════════════════════
                         📋 THE WORKFLOW
═══════════════════════════════════════════════════════════════════════════

Here's how features flow from idea to implementation:

    💡 IDEA
       │
       ├─ User-facing? ─────────────────┐
       │                                ▼
       │                         📝 PRD (What & Why)
       │                                │
    ┌──▼────────────────────────────────┘
    │  🔍 SPIKE (Investigation)
    │     └─ Recommends implementation?
    │                │
    ┌────────────────▼─────────────────┐
    │  📋 RFC (Design)                 │
    │     └─ Accepted?                 │
    │            │                     │
    │     ┌──────▼──────┐              │
    │     │  📝 PLAN    │              │
    │     │  (Tasks)    │              │
    │     └──────┬──────┘              │
    │            │                     │
    │     ┌──────▼──────┐              │
    │     │  🔨 BUILD   │              │
    │     └──────┬──────┘              │
    │            │                     │
    │     ┌──────▼──────┐              │
    │     │  🔍 REVIEW  │              │
    │     │  (PR)       │              │
    │     └──────┬──────┘              │
    │            │                     │
    │     ┌──────▼──────┐              │
    │     │  🚀 SHIP    │              │
    │     └─────────────┘              │
    └──────────────────────────────────┘

Don't worry about memorizing this. I'll guide you through each step.
"#,
        "documents" => r#"
═══════════════════════════════════════════════════════════════════════════
                         📄 DOCUMENT TYPES
═══════════════════════════════════════════════════════════════════════════

PRD (Product Requirements Document)
  • Captures WHAT you're building and WHY
  • Includes user stories and acceptance criteria
  • Use when: User-facing features, stakeholder sign-off needed
  • Command: blue_prd_create

RFC (Request for Comments)
  • Captures HOW you'll build something
  • Includes problem, goals, proposal, alternatives
  • Use when: Non-trivial implementation decisions
  • Command: blue_rfc_create

Spike
  • Time-boxed investigation
  • Ends with: no-action, decision-made, or recommends-implementation
  • Use when: Unsure if something is feasible or which approach to take
  • Command: blue_spike_create

ADR (Architecture Decision Record)
  • Documents significant architectural decisions
  • Created after implementing important patterns
  • Command: blue_adr_create

Decision Note
  • Lightweight alternative to RFC/ADR
  • For simple choices between options
  • Command: blue_decision_create
"#,
        "implementation" => r#"
═══════════════════════════════════════════════════════════════════════════
                         🔨 IMPLEMENTATION
═══════════════════════════════════════════════════════════════════════════

Once an RFC is accepted, here's the implementation flow:

1. CREATE WORKTREE
   blue_worktree_create title="my-feature"
   - Creates isolated git worktree
   - Keeps your work separate from main

2. TRACK PROGRESS
   blue_rfc_task_complete title="my-feature" task="1"
   - Check off tasks as you complete them
   - I'll track your progress automatically

3. MARK COMPLETE
   blue_rfc_complete title="my-feature"
   - Requires 70%+ task completion
   - I'll suggest if ADR is needed

4. CREATE PR
   blue_pr_create title="Add my feature"
   - I'll generate a summary and test plan
   - Links back to the RFC

5. VERIFY & MERGE
   blue_pr_verify → blue_pr_merge
   - Check test plan items
   - Squash merge when approved

6. CLEANUP
   blue_worktree_cleanup title="my-feature"
   - Removes worktree and branch
   - Syncs with develop
"#,
        "ready" => r#"
═══════════════════════════════════════════════════════════════════════════
                         🚀 READY TO START
═══════════════════════════════════════════════════════════════════════════

You're all set! Here are the commands you'll use most:

STATUS & NAVIGATION
  blue_status        - What's the current project state?
  blue_next          - What should I work on next?
  blue_audit         - Any issues I should know about?

DOCUMENTS
  blue_prd_create    - Start defining requirements
  blue_rfc_create    - Start designing a feature
  blue_spike_create  - Start investigating a question

SEARCH
  blue_search query="..." - Find documents by keyword

WORKFLOW
  blue_worktree_create    - Start implementing
  blue_rfc_complete       - Mark RFC as done
  blue_pr_create          - Create pull request

Just describe what you want to do, and I'll suggest the right tool.

Off you go! 🐑
"#,
        _ => "Unknown section",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sections_have_content() {
        for section in SECTIONS {
            let content = get_section_content(section);
            assert!(!content.is_empty());
            assert_ne!(content, "Unknown section");
        }
    }
}
