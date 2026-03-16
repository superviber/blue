//! Playwright verification handler
//!
//! Provides browser-based test verification using Playwright MCP.
//! Generates verification plans that Claude can execute via Playwright tools.

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

use crate::handler_error::HandlerError;

/// A single verification step for Playwright execution
#[derive(Debug, Clone, Serialize)]
pub struct VerificationStep {
    pub step: usize,
    pub action: VerificationAction,
    pub description: String,
    pub mcp_tool: String,
    pub assertion: String,
}

/// Types of verification actions
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationAction {
    Navigate,
    Snapshot,
    Screenshot,
    Click,
    Fill,
    Resize,
}

/// URL safety level
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlSafetyLevel {
    Localhost,
    Development,
    Staging,
    Production,
    Unknown,
}

/// Handle blue_playwright_verify
pub fn handle_verify(args: &Value) -> Result<Value, HandlerError> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let base_url = args
        .get("base_url")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let path = args.get("path").and_then(|v| v.as_str());
    let allow_staging = args
        .get("allow_staging")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let expected_outcomes: Vec<String> = args
        .get("expected_outcomes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Validate URL safety
    let safety_level = classify_url_safety(base_url);
    validate_url_safety(&safety_level, allow_staging)?;

    // Build full URL
    let target_url = if let Some(p) = path {
        format!("{}{}", base_url.trim_end_matches('/'), p)
    } else {
        base_url.to_string()
    };

    // Generate verification steps from task description
    let steps = generate_verification_steps(task, &target_url);

    // Generate the Playwright MCP tool sequence
    let playwright_sequence = generate_playwright_sequence(&steps, &target_url);

    let hint = format!(
        "Generated {} verification steps for '{}'. Execute the playwright_sequence in order.",
        steps.len(),
        task
    );

    Ok(json!({
        "status": "success",
        "message": crate::voice::info(
            &format!("{} verification steps generated", steps.len()),
            Some(&hint)
        ),
        "verification_plan": {
            "task": task,
            "target_url": target_url,
            "safety_level": safety_level,
            "steps": steps,
            "expected_outcomes": expected_outcomes
        },
        "playwright_sequence": playwright_sequence,
        "safety": {
            "url_safety_level": safety_level,
            "requires_approval": safety_level != UrlSafetyLevel::Localhost,
            "blocked": safety_level == UrlSafetyLevel::Production
        },
        "evidence_guidance": {
            "screenshot_tool": "mcp__playwright__browser_take_screenshot",
            "snapshot_tool": "mcp__playwright__browser_snapshot",
            "recommended": "Take screenshots before and after key actions"
        },
        "suggested_tools": [
            "mcp__playwright__browser_navigate",
            "mcp__playwright__browser_snapshot",
            "mcp__playwright__browser_take_screenshot"
        ]
    }))
}

/// Classify URL safety level
fn classify_url_safety(url: &str) -> UrlSafetyLevel {
    let lower = url.to_lowercase();

    // Check for localhost first
    if lower.contains("localhost") || lower.contains("127.0.0.1") || lower.contains("[::1]") {
        return UrlSafetyLevel::Localhost;
    }

    // Production patterns - blocked
    let production_patterns = [
        "prod.",
        ".prod",
        "production.",
        ".production",
        "live.",
        ".live",
        "www.",
    ];
    if production_patterns.iter().any(|p| lower.contains(p)) {
        return UrlSafetyLevel::Production;
    }

    // Development patterns
    let dev_patterns = [
        "dev.",
        ".dev",
        "development.",
        ".development",
        "local.",
        ".local",
        ":3000",
        ":3001",
        ":5173",
        ":5174",
        ":8080",
        ":8000",
        ":4200",
    ];
    if dev_patterns.iter().any(|p| lower.contains(p)) {
        return UrlSafetyLevel::Development;
    }

    // Staging patterns
    let staging_patterns = [
        "staging.", ".staging", "stage.", ".stage", "test.", ".test", "qa.", ".qa", "uat.", ".uat",
        "preview.", ".preview",
    ];
    if staging_patterns.iter().any(|p| lower.contains(p)) {
        return UrlSafetyLevel::Staging;
    }

    // If it looks like an IP address with a port, likely development
    if let Ok(re) = Regex::new(r"\d+\.\d+\.\d+\.\d+:\d+") {
        if re.is_match(&lower) {
            return UrlSafetyLevel::Development;
        }
    }

    UrlSafetyLevel::Unknown
}

/// Validate URL safety and return error if blocked
fn validate_url_safety(
    safety_level: &UrlSafetyLevel,
    allow_staging: bool,
) -> Result<(), HandlerError> {
    match safety_level {
        UrlSafetyLevel::Localhost | UrlSafetyLevel::Development => Ok(()),
        UrlSafetyLevel::Staging => {
            if allow_staging {
                Ok(())
            } else {
                Err(HandlerError::CommandFailed(
                    "Staging URLs require explicit approval. Pass allow_staging=true to proceed."
                        .to_string(),
                ))
            }
        }
        UrlSafetyLevel::Production => Err(HandlerError::CommandFailed(
            "Cannot run Playwright verification against production URLs. Use localhost or staging."
                .to_string(),
        )),
        UrlSafetyLevel::Unknown => Err(HandlerError::CommandFailed(
            "Unknown URL safety level. Use localhost for testing or explicitly allow staging."
                .to_string(),
        )),
    }
}

/// Generate verification steps based on task description
fn generate_verification_steps(task: &str, target_url: &str) -> Vec<VerificationStep> {
    let lower = task.to_lowercase();
    let mut steps = Vec::new();
    let mut step_num = 0;

    // Always start with navigation
    step_num += 1;
    steps.push(VerificationStep {
        step: step_num,
        action: VerificationAction::Navigate,
        description: format!("Navigate to {}", target_url),
        mcp_tool: "mcp__playwright__browser_navigate".to_string(),
        assertion: "Page loads successfully".to_string(),
    });

    // Always take initial snapshot
    step_num += 1;
    steps.push(VerificationStep {
        step: step_num,
        action: VerificationAction::Snapshot,
        description: "Capture initial page state".to_string(),
        mcp_tool: "mcp__playwright__browser_snapshot".to_string(),
        assertion: "Page structure is visible".to_string(),
    });

    // Page load verification
    if lower.contains("page load")
        || lower.contains("loads correctly")
        || lower.contains("displays")
    {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Screenshot,
            description: "Capture screenshot as page load evidence".to_string(),
            mcp_tool: "mcp__playwright__browser_take_screenshot".to_string(),
            assertion: "Page rendered correctly".to_string(),
        });
    }

    // Form interactions
    if lower.contains("form") || lower.contains("input") || lower.contains("fill") {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Snapshot,
            description: "Identify form fields in page structure".to_string(),
            mcp_tool: "mcp__playwright__browser_snapshot".to_string(),
            assertion: "Form fields are accessible".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Fill,
            description: "Fill form fields with test data".to_string(),
            mcp_tool: "mcp__playwright__browser_fill".to_string(),
            assertion: "Form accepts input".to_string(),
        });
    }

    // Click interactions
    if lower.contains("click") || lower.contains("button") {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Click,
            description: "Click the target element".to_string(),
            mcp_tool: "mcp__playwright__browser_click".to_string(),
            assertion: "Element responds to click".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Snapshot,
            description: "Capture state after click".to_string(),
            mcp_tool: "mcp__playwright__browser_snapshot".to_string(),
            assertion: "Expected state change occurred".to_string(),
        });
    }

    // Modal / dialog testing
    if lower.contains("modal") || lower.contains("dialog") || lower.contains("popup") {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Click,
            description: "Open modal/dialog".to_string(),
            mcp_tool: "mcp__playwright__browser_click".to_string(),
            assertion: "Modal opens".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Screenshot,
            description: "Screenshot modal for evidence".to_string(),
            mcp_tool: "mcp__playwright__browser_take_screenshot".to_string(),
            assertion: "Modal state captured".to_string(),
        });
    }

    // Responsive / mobile testing
    if lower.contains("responsive") || lower.contains("mobile") || lower.contains("viewport") {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Resize,
            description: "Resize to mobile viewport (375x667)".to_string(),
            mcp_tool: "mcp__playwright__browser_resize".to_string(),
            assertion: "Viewport resized to mobile".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Screenshot,
            description: "Screenshot mobile layout".to_string(),
            mcp_tool: "mcp__playwright__browser_take_screenshot".to_string(),
            assertion: "Mobile layout captured".to_string(),
        });
    }

    // Login testing
    if lower.contains("login") || lower.contains("sign in") || lower.contains("authentication") {
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Fill,
            description: "Fill login credentials".to_string(),
            mcp_tool: "mcp__playwright__browser_fill".to_string(),
            assertion: "Credentials entered".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Click,
            description: "Submit login form".to_string(),
            mcp_tool: "mcp__playwright__browser_click".to_string(),
            assertion: "Login submitted".to_string(),
        });
        step_num += 1;
        steps.push(VerificationStep {
            step: step_num,
            action: VerificationAction::Snapshot,
            description: "Capture post-login state".to_string(),
            mcp_tool: "mcp__playwright__browser_snapshot".to_string(),
            assertion: "Login result visible".to_string(),
        });
    }

    // Always end with a final screenshot for evidence
    step_num += 1;
    steps.push(VerificationStep {
        step: step_num,
        action: VerificationAction::Screenshot,
        description: "Final screenshot for verification evidence".to_string(),
        mcp_tool: "mcp__playwright__browser_take_screenshot".to_string(),
        assertion: "Final state captured".to_string(),
    });

    steps
}

/// Generate the Playwright MCP tool sequence for Claude to execute
fn generate_playwright_sequence(steps: &[VerificationStep], target_url: &str) -> Vec<Value> {
    steps
        .iter()
        .map(|step| {
            let params = match step.action {
                VerificationAction::Navigate => json!({
                    "url": target_url
                }),
                VerificationAction::Resize => json!({
                    "width": 375,
                    "height": 667
                }),
                VerificationAction::Fill => json!({
                    "selector": "[element selector - identify from snapshot]",
                    "value": "[test value]"
                }),
                VerificationAction::Click => json!({
                    "selector": "[element selector - identify from snapshot]"
                }),
                _ => json!({}),
            };

            json!({
                "step": step.step,
                "tool": step.mcp_tool,
                "description": step.description,
                "params": params,
                "assertion": step.assertion
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_url_safety_localhost() {
        assert_eq!(
            classify_url_safety("http://localhost:3000"),
            UrlSafetyLevel::Localhost
        );
        assert_eq!(
            classify_url_safety("http://127.0.0.1:8080"),
            UrlSafetyLevel::Localhost
        );
    }

    #[test]
    fn test_classify_url_safety_development() {
        assert_eq!(
            classify_url_safety("http://dev.example.com"),
            UrlSafetyLevel::Development
        );
        assert_eq!(
            classify_url_safety("http://192.168.1.100:3000"),
            UrlSafetyLevel::Development
        );
    }

    #[test]
    fn test_classify_url_safety_staging() {
        assert_eq!(
            classify_url_safety("https://staging.example.com"),
            UrlSafetyLevel::Staging
        );
    }

    #[test]
    fn test_classify_url_safety_production() {
        assert_eq!(
            classify_url_safety("https://www.example.com"),
            UrlSafetyLevel::Production
        );
    }

    #[test]
    fn test_validate_url_safety() {
        assert!(validate_url_safety(&UrlSafetyLevel::Localhost, false).is_ok());
        assert!(validate_url_safety(&UrlSafetyLevel::Staging, false).is_err());
        assert!(validate_url_safety(&UrlSafetyLevel::Staging, true).is_ok());
        assert!(validate_url_safety(&UrlSafetyLevel::Production, true).is_err());
    }

    #[test]
    fn test_generate_verification_steps() {
        let steps = generate_verification_steps(
            "Verify the login page loads correctly",
            "http://localhost:3000/login",
        );
        assert!(steps.len() >= 3);
        assert!(matches!(steps[0].action, VerificationAction::Navigate));
    }
}
