//! Document types for Blue
//!
//! RFCs, ADRs, Spikes, and other document structures with markdown generation.

use serde::{Deserialize, Serialize};

/// Document status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Accepted,
    InProgress,
    Implemented,
    Superseded,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Accepted => "accepted",
            Status::InProgress => "in-progress",
            Status::Implemented => "implemented",
            Status::Superseded => "superseded",
        }
    }
}

/// An RFC (Request for Comments) - a design document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rfc {
    pub title: String,
    pub status: Status,
    pub date: Option<String>,
    pub source_spike: Option<String>,
    pub source_prd: Option<String>,
    pub problem: Option<String>,
    pub proposal: Option<String>,
    pub goals: Vec<String>,
    pub non_goals: Vec<String>,
    pub plan: Vec<Task>,
}

/// A task within an RFC plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub description: String,
    pub completed: bool,
}

/// A Spike - a time-boxed investigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spike {
    pub title: String,
    pub status: String,
    pub date: String,
    pub time_box: Option<String>,
    pub question: String,
    pub outcome: Option<SpikeOutcome>,
    pub findings: Option<String>,
    pub recommendation: Option<String>,
}

/// Outcome of a spike investigation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeOutcome {
    NoAction,
    DecisionMade,
    RecommendsImplementation,
}

impl SpikeOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpikeOutcome::NoAction => "no-action",
            SpikeOutcome::DecisionMade => "decision-made",
            SpikeOutcome::RecommendsImplementation => "recommends-implementation",
        }
    }
}

/// A Decision Note - lightweight choice documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub title: String,
    pub date: String,
    pub decision: String,
    pub rationale: Option<String>,
    pub alternatives: Vec<String>,
}

/// An ADR (Architecture Decision Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
    pub title: String,
    pub status: String,
    pub date: String,
    pub source_rfc: Option<String>,
    pub context: String,
    pub decision: String,
    pub consequences: Vec<String>,
}

/// An Audit document - formal findings report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audit {
    pub title: String,
    pub status: String,
    pub date: String,
    pub audit_type: AuditType,
    pub scope: String,
    pub summary: Option<String>,
    pub findings: Vec<AuditFinding>,
    pub recommendations: Vec<String>,
}

/// Types of audits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditType {
    Repository,
    Security,
    RfcVerification,
    AdrAdherence,
    Custom,
}

impl AuditType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditType::Repository => "repository",
            AuditType::Security => "security",
            AuditType::RfcVerification => "rfc-verification",
            AuditType::AdrAdherence => "adr-adherence",
            AuditType::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "repository" => Some(AuditType::Repository),
            "security" => Some(AuditType::Security),
            "rfc-verification" => Some(AuditType::RfcVerification),
            "adr-adherence" => Some(AuditType::AdrAdherence),
            "custom" => Some(AuditType::Custom),
            _ => None,
        }
    }
}

/// A finding within an audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub category: String,
    pub title: String,
    pub description: String,
    pub severity: AuditSeverity,
}

/// Severity of an audit finding
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Error,
    Warning,
    Info,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditSeverity::Error => "error",
            AuditSeverity::Warning => "warning",
            AuditSeverity::Info => "info",
        }
    }
}

impl Rfc {
    /// Create a new RFC in draft status
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: Status::Draft,
            date: Some(today()),
            source_spike: None,
            source_prd: None,
            problem: None,
            proposal: None,
            goals: Vec::new(),
            non_goals: Vec::new(),
            plan: Vec::new(),
        }
    }

    /// Calculate completion percentage of the plan
    pub fn progress(&self) -> f64 {
        if self.plan.is_empty() {
            return 0.0;
        }
        let completed = self.plan.iter().filter(|t| t.completed).count();
        (completed as f64 / self.plan.len() as f64) * 100.0
    }

    /// Generate markdown content
    pub fn to_markdown(&self, number: u32) -> String {
        let mut md = String::new();

        // Title
        md.push_str(&format!(
            "# RFC {:04}: {}\n\n",
            number,
            to_title_case(&self.title)
        ));

        // Metadata table
        md.push_str("| | |\n|---|---|\n");
        md.push_str(&format!(
            "| **Status** | {} |\n",
            to_title_case(self.status.as_str())
        ));
        if let Some(ref date) = self.date {
            md.push_str(&format!("| **Date** | {} |\n", date));
        }
        if let Some(ref spike) = self.source_spike {
            md.push_str(&format!("| **Source Spike** | {} |\n", spike));
        }
        if let Some(ref prd) = self.source_prd {
            md.push_str(&format!("| **Source PRD** | {} |\n", prd));
        }
        md.push_str("\n---\n\n");

        // Summary (problem)
        if let Some(ref problem) = self.problem {
            md.push_str("## Summary\n\n");
            md.push_str(problem);
            md.push_str("\n\n");
        }

        // Proposal
        if let Some(ref proposal) = self.proposal {
            md.push_str("## Proposal\n\n");
            md.push_str(proposal);
            md.push_str("\n\n");
        }

        // Goals
        if !self.goals.is_empty() {
            md.push_str("## Goals\n\n");
            for goal in &self.goals {
                md.push_str(&format!("- {}\n", goal));
            }
            md.push('\n');
        }

        // Non-Goals
        if !self.non_goals.is_empty() {
            md.push_str("## Non-Goals\n\n");
            for ng in &self.non_goals {
                md.push_str(&format!("- {}\n", ng));
            }
            md.push('\n');
        }

        // Test Plan (empty checkboxes)
        md.push_str("## Test Plan\n\n");
        md.push_str("- [ ] TBD\n\n");

        // Blue's signature
        md.push_str("---\n\n");
        md.push_str("*\"Right then. Let's get to it.\"*\n\n");
        md.push_str("— Blue\n");

        md
    }
}

impl Spike {
    /// Create a new spike
    pub fn new(title: impl Into<String>, question: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: "in-progress".to_string(),
            date: today(),
            time_box: None,
            question: question.into(),
            outcome: None,
            findings: None,
            recommendation: None,
        }
    }

    /// Generate markdown content
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Spike: {}\n\n", to_title_case(&self.title)));

        md.push_str("| | |\n|---|---|\n");
        md.push_str(&format!(
            "| **Status** | {} |\n",
            to_title_case(&self.status)
        ));
        md.push_str(&format!("| **Date** | {} |\n", self.date));
        if let Some(ref tb) = self.time_box {
            md.push_str(&format!("| **Time Box** | {} |\n", tb));
        }
        if let Some(ref outcome) = self.outcome {
            md.push_str(&format!("| **Outcome** | {} |\n", outcome.as_str()));
        }
        md.push_str("\n---\n\n");

        md.push_str("## Question\n\n");
        md.push_str(&self.question);
        md.push_str("\n\n");

        if let Some(ref findings) = self.findings {
            md.push_str("## Findings\n\n");
            md.push_str(findings);
            md.push_str("\n\n");
        }

        if let Some(ref rec) = self.recommendation {
            md.push_str("## Recommendation\n\n");
            md.push_str(rec);
            md.push_str("\n\n");
        }

        md.push_str("---\n\n");
        md.push_str("*Investigation notes by Blue*\n");

        md
    }
}

impl Adr {
    /// Create a new ADR
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: "accepted".to_string(),
            date: today(),
            source_rfc: None,
            context: String::new(),
            decision: String::new(),
            consequences: Vec::new(),
        }
    }

    /// Generate markdown content
    pub fn to_markdown(&self, number: u32) -> String {
        let mut md = String::new();

        md.push_str(&format!(
            "# ADR {:04}: {}\n\n",
            number,
            to_title_case(&self.title)
        ));

        md.push_str("| | |\n|---|---|\n");
        md.push_str(&format!(
            "| **Status** | {} |\n",
            to_title_case(&self.status)
        ));
        md.push_str(&format!("| **Date** | {} |\n", self.date));
        if let Some(ref rfc) = self.source_rfc {
            md.push_str(&format!("| **RFC** | {} |\n", rfc));
        }
        md.push_str("\n---\n\n");

        md.push_str("## Context\n\n");
        md.push_str(&self.context);
        md.push_str("\n\n");

        md.push_str("## Decision\n\n");
        md.push_str(&self.decision);
        md.push_str("\n\n");

        if !self.consequences.is_empty() {
            md.push_str("## Consequences\n\n");
            for c in &self.consequences {
                md.push_str(&format!("- {}\n", c));
            }
            md.push('\n');
        }

        md.push_str("---\n\n");
        md.push_str("*Recorded by Blue*\n");

        md
    }
}

impl Decision {
    /// Create a new Decision
    pub fn new(title: impl Into<String>, decision: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            date: today(),
            decision: decision.into(),
            rationale: None,
            alternatives: Vec::new(),
        }
    }

    /// Generate markdown content
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Decision: {}\n\n", to_title_case(&self.title)));
        md.push_str(&format!("**Date:** {}\n\n", self.date));

        md.push_str("## Decision\n\n");
        md.push_str(&self.decision);
        md.push_str("\n\n");

        if let Some(ref rationale) = self.rationale {
            md.push_str("## Rationale\n\n");
            md.push_str(rationale);
            md.push_str("\n\n");
        }

        if !self.alternatives.is_empty() {
            md.push_str("## Alternatives Considered\n\n");
            for alt in &self.alternatives {
                md.push_str(&format!("- {}\n", alt));
            }
            md.push('\n');
        }

        md.push_str("---\n\n");
        md.push_str("*Noted by Blue*\n");

        md
    }
}

impl Audit {
    /// Create a new Audit
    pub fn new(title: impl Into<String>, audit_type: AuditType, scope: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: "in-progress".to_string(),
            date: today(),
            audit_type,
            scope: scope.into(),
            summary: None,
            findings: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    /// Generate markdown content
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Audit: {}\n\n", to_title_case(&self.title)));

        md.push_str("| | |\n|---|---|\n");
        md.push_str(&format!(
            "| **Status** | {} |\n",
            to_title_case(&self.status)
        ));
        md.push_str(&format!("| **Date** | {} |\n", self.date));
        md.push_str(&format!(
            "| **Type** | {} |\n",
            to_title_case(self.audit_type.as_str())
        ));
        md.push_str(&format!("| **Scope** | {} |\n", self.scope));
        md.push_str("\n---\n\n");

        if let Some(ref summary) = self.summary {
            md.push_str("## Executive Summary\n\n");
            md.push_str(summary);
            md.push_str("\n\n");
        }

        if !self.findings.is_empty() {
            md.push_str("## Findings\n\n");
            for finding in &self.findings {
                md.push_str(&format!(
                    "### {} ({})\n\n",
                    finding.title,
                    finding.severity.as_str()
                ));
                md.push_str(&format!("**Category:** {}\n\n", finding.category));
                md.push_str(&finding.description);
                md.push_str("\n\n");
            }
        }

        if !self.recommendations.is_empty() {
            md.push_str("## Recommendations\n\n");
            for rec in &self.recommendations {
                md.push_str(&format!("- {}\n", rec));
            }
            md.push('\n');
        }

        md.push_str("---\n\n");
        md.push_str("*Audited by Blue*\n");

        md
    }
}

/// Get current date in YYYY-MM-DD format
fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Convert kebab-case to Title Case
fn to_title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Update status in a markdown file
///
/// Handles common status patterns:
/// - `| **Status** | Draft |` (table format)
/// - `**Status:** Draft` (inline format)
///
/// Returns Ok(true) if status was updated, Ok(false) if no match found.
pub fn update_markdown_status(
    file_path: &std::path::Path,
    new_status: &str,
) -> Result<bool, std::io::Error> {
    use std::fs;

    if !file_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(file_path)?;
    let display_status = to_title_case(new_status);

    // Try table format: | **Status** | <anything> |
    let table_pattern = regex::Regex::new(r"\| \*\*Status\*\* \| [^|]+ \|").unwrap();
    let mut updated = table_pattern
        .replace(&content, format!("| **Status** | {} |", display_status).as_str())
        .to_string();

    // Also try inline format: **Status:** <word>
    let inline_pattern = regex::Regex::new(r"\*\*Status:\*\* \S+").unwrap();
    updated = inline_pattern
        .replace(&updated, format!("**Status:** {}", display_status).as_str())
        .to_string();

    let changed = updated != content;
    if changed {
        fs::write(file_path, updated)?;
    }

    Ok(changed)
}

/// RFC header format types (RFC 0017)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFormat {
    /// Table format: `| **Status** | Draft |`
    Table,
    /// Inline format: `**Status:** Draft`
    Inline,
    /// No recognizable header format
    Missing,
}

/// Validate RFC header format
///
/// Returns the detected header format:
/// - Table: canonical format `| **Status** | Draft |`
/// - Inline: non-canonical format `**Status:** Draft`
/// - Missing: no status header found
pub fn validate_rfc_header(content: &str) -> HeaderFormat {
    let table_pattern = regex::Regex::new(r"\| \*\*Status\*\* \| [^|]+ \|").unwrap();
    let inline_pattern = regex::Regex::new(r"\*\*Status:\*\*\s+\S+").unwrap();

    if table_pattern.is_match(content) {
        HeaderFormat::Table
    } else if inline_pattern.is_match(content) {
        HeaderFormat::Inline
    } else {
        HeaderFormat::Missing
    }
}

/// Convert inline header format to table format
///
/// Converts patterns like:
/// ```text
/// **Status:** Draft
/// **Created:** 2026-01-25
/// **Author:** Claude
/// ```
///
/// To:
/// ```text
/// | | |
/// |---|---|
/// | **Status** | Draft |
/// | **Created** | 2026-01-25 |
/// | **Author** | Claude |
/// ```
pub fn convert_inline_to_table_header(content: &str) -> String {
    // Match inline metadata patterns: **Key:** Value
    let inline_re = regex::Regex::new(r"\*\*([^:*]+):\*\*\s*(.+)").unwrap();

    let mut metadata_lines: Vec<(String, String)> = Vec::new();
    let mut other_lines: Vec<String> = Vec::new();
    let mut in_header_section = false;
    let mut header_ended = false;

    for line in content.lines() {
        // Skip title line
        if line.starts_with("# ") {
            other_lines.push(line.to_string());
            in_header_section = true;
            continue;
        }

        // Check for inline metadata
        if in_header_section && !header_ended {
            if let Some(caps) = inline_re.captures(line) {
                let key = caps.get(1).unwrap().as_str().trim();
                let value = caps.get(2).unwrap().as_str().trim();
                metadata_lines.push((key.to_string(), value.to_string()));
                continue;
            }

            // Empty line in header section is ok
            if line.trim().is_empty() && !metadata_lines.is_empty() {
                continue;
            }

            // If we had metadata and hit something else, header section ended
            if !metadata_lines.is_empty() {
                header_ended = true;
            }
        }

        other_lines.push(line.to_string());
    }

    if metadata_lines.is_empty() {
        return content.to_string();
    }

    // Reconstruct content with table format
    let mut result = String::new();

    // Find and add title
    if let Some(title_pos) = other_lines.iter().position(|l| l.starts_with("# ")) {
        result.push_str(&other_lines[title_pos]);
        result.push_str("\n\n");

        // Add table header
        result.push_str("| | |\n");
        result.push_str("|---|---|\n");

        // Add metadata rows
        for (key, value) in &metadata_lines {
            result.push_str(&format!("| **{}** | {} |\n", key, value));
        }

        // Add remaining content
        let remaining = other_lines[title_pos + 1..].join("\n");
        let trimmed = remaining.trim_start();
        if !trimmed.is_empty() {
            result.push('\n');
            result.push_str(trimmed);
        }
    } else {
        // No title found, return original
        return content.to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc_to_markdown() {
        let mut rfc = Rfc::new("my-feature");
        rfc.problem = Some("Things are slow".to_string());
        rfc.goals = vec!["Make it fast".to_string()];

        let md = rfc.to_markdown(1);
        assert!(md.contains("# RFC 0001: My Feature"));
        assert!(md.contains("Things are slow"));
        assert!(md.contains("Make it fast"));
        assert!(md.contains("— Blue"));
    }

    #[test]
    fn test_title_case() {
        assert_eq!(to_title_case("my-feature"), "My Feature");
        assert_eq!(to_title_case("in-progress"), "In Progress");
    }

    #[test]
    fn test_spike_to_markdown() {
        let spike = Spike::new("test-investigation", "What should we do?");
        let md = spike.to_markdown();
        assert!(md.contains("# Spike: Test Investigation"));
        assert!(md.contains("What should we do?"));
    }

    #[test]
    fn test_update_markdown_status_table_format() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");

        let content = "# RFC\n\n| | |\n|---|---|\n| **Status** | Draft |\n| **Date** | 2026-01-24 |\n";
        fs::write(&file, content).unwrap();

        let changed = update_markdown_status(&file, "implemented").unwrap();
        assert!(changed);

        let updated = fs::read_to_string(&file).unwrap();
        assert!(updated.contains("| **Status** | Implemented |"));
        assert!(!updated.contains("Draft"));
    }

    #[test]
    fn test_update_markdown_status_no_file() {
        let path = std::path::Path::new("/nonexistent/file.md");
        let changed = update_markdown_status(path, "implemented").unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_update_markdown_status_no_status_field() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.md");

        let content = "# Just a document\n\nNo status here.\n";
        fs::write(&file, content).unwrap();

        let changed = update_markdown_status(&file, "implemented").unwrap();
        assert!(!changed);
    }

    #[test]
    fn test_validate_rfc_header_table_format() {
        let content = "# RFC 0001: Test\n\n| | |\n|---|---|\n| **Status** | Draft |\n| **Date** | 2026-01-24 |\n";
        assert_eq!(validate_rfc_header(content), HeaderFormat::Table);
    }

    #[test]
    fn test_validate_rfc_header_inline_format() {
        let content = "# RFC 0001: Test\n\n**Status:** Draft\n**Date:** 2026-01-24\n";
        assert_eq!(validate_rfc_header(content), HeaderFormat::Inline);
    }

    #[test]
    fn test_validate_rfc_header_missing() {
        let content = "# RFC 0001: Test\n\nJust some content without status.\n";
        assert_eq!(validate_rfc_header(content), HeaderFormat::Missing);
    }

    #[test]
    fn test_convert_inline_to_table_header() {
        let content = "# RFC 0001: Test\n\n**Status:** Draft\n**Created:** 2026-01-25\n**Author:** Claude\n\n## Problem\n\nSomething is wrong.\n";

        let converted = convert_inline_to_table_header(content);

        assert!(converted.contains("| | |"));
        assert!(converted.contains("|---|---|"));
        assert!(converted.contains("| **Status** | Draft |"));
        assert!(converted.contains("| **Created** | 2026-01-25 |"));
        assert!(converted.contains("| **Author** | Claude |"));
        assert!(converted.contains("## Problem"));
        assert!(converted.contains("Something is wrong."));
        assert!(!converted.contains("**Status:**"));
    }

    #[test]
    fn test_convert_inline_to_table_header_no_change() {
        let content = "# RFC 0001: Test\n\n| | |\n|---|---|\n| **Status** | Draft |\n\n## Problem\n";
        let converted = convert_inline_to_table_header(content);
        // Should not change already-table-formatted content
        assert_eq!(converted, content);
    }
}
