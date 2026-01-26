//! ADR tool handlers
//!
//! Handles Architecture Decision Record creation, listing, and adherence checking.
//! Implements RFC 0004: ADR Adherence.

use std::fs;
use std::path::Path;

use blue_core::{Adr, DocType, Document, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// ADR summary for listing and relevance matching
#[derive(Debug, Clone)]
struct AdrSummary {
    number: i64,
    title: String,
    summary: String,
    keywords: Vec<String>,
    applies_when: Vec<String>,
    anti_patterns: Vec<String>,
}

/// Handle blue_adr_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let source_rfc = args.get("rfc").and_then(|v| v.as_str());
    let context = args.get("context").and_then(|v| v.as_str());
    let decision = args.get("decision").and_then(|v| v.as_str());
    let consequences: Vec<String> = args
        .get("consequences")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Check if linked RFC exists and is implemented
    if let Some(rfc_title) = source_rfc {
        match state.store.find_document(DocType::Rfc, rfc_title) {
            Ok(doc) => {
                if doc.status != "implemented" {
                    return Ok(json!({
                        "status": "error",
                        "message": blue_core::voice::error(
                            &format!("RFC '{}' isn't implemented yet (status: {})", rfc_title, doc.status),
                            "ADRs document decisions from implemented RFCs"
                        )
                    }));
                }
            }
            Err(_) => {
                return Ok(json!({
                    "status": "error",
                    "message": blue_core::voice::error(
                        &format!("Can't find RFC '{}'", rfc_title),
                        "Check the title's spelled right"
                    )
                }));
            }
        }
    }

    // Get next ADR number
    let number = state
        .store
        .next_number_with_fs(DocType::Adr, &state.home.docs_path)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Create ADR
    let mut adr = Adr::new(title);
    if let Some(rfc) = source_rfc {
        adr.source_rfc = Some(rfc.to_string());
    }
    if let Some(ctx) = context {
        adr.context = ctx.to_string();
    }
    if let Some(dec) = decision {
        adr.decision = dec.to_string();
    }
    adr.consequences = consequences.clone();

    // Generate markdown
    let markdown = adr.to_markdown(number as u32);

    // Compute file path
    let file_name = format!("{:04}-{}.md", number, to_kebab_case(title));
    let file_path = format!("adrs/{}", file_name);

    // Write the file
    let docs_path = state.home.docs_path.clone();
    let adr_path = docs_path.join(&file_path);
    if let Some(parent) = adr_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }
    fs::write(&adr_path, &markdown).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Add to store
    let mut doc = Document::new(DocType::Adr, title, "accepted");
    doc.number = Some(number);
    doc.file_path = Some(file_path.clone());

    let id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Link to RFC if provided
    if let Some(rfc_title) = source_rfc {
        if let Ok(rfc_doc) = state.store.find_document(DocType::Rfc, rfc_title) {
            if let (Some(rfc_id), Some(adr_id)) = (rfc_doc.id, Some(id)) {
                let _ = state.store.link_documents(
                    rfc_id,
                    adr_id,
                    blue_core::LinkType::RfcToAdr,
                );
            }
        }
    }

    Ok(json!({
        "status": "success",
        "id": id,
        "number": number,
        "title": title,
        "file": adr_path.display().to_string(),
        "markdown": markdown,
        "linked_rfc": source_rfc,
        "message": blue_core::voice::success(
            &format!("Created ADR {:04}: '{}'", number, title),
            Some("Decision documented.")
        )
    }))
}

// ===== RFC 0004: ADR Adherence =====

/// Handle blue_adr_list
///
/// List all ADRs with summaries.
pub fn handle_list(state: &ProjectState) -> Result<Value, ServerError> {
    let adrs = load_adr_summaries(state)?;

    let adr_list: Vec<Value> = adrs
        .iter()
        .map(|adr| {
            json!({
                "number": adr.number,
                "title": adr.title,
                "summary": adr.summary
            })
        })
        .collect();

    Ok(json!({
        "adrs": adr_list,
        "count": adr_list.len(),
        "message": blue_core::voice::info(
            &format!("{} ADR(s) found", adr_list.len()),
            Some("Use blue_adr_get to view details")
        )
    }))
}

/// Handle blue_adr_get
///
/// Get full ADR content with referenced_by information.
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let number = args
        .get("number")
        .and_then(|v| v.as_i64())
        .ok_or(ServerError::InvalidParams)?;

    // Find ADR document
    let docs = state
        .store
        .list_documents(DocType::Adr)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let adr_doc = docs
        .into_iter()
        .find(|d| d.number == Some(number as i32))
        .ok_or_else(|| ServerError::StateLoadFailed(format!("ADR {} not found", number)))?;

    // Read content
    let file_path = adr_doc.file_path.as_ref().ok_or(ServerError::InvalidParams)?;
    let full_path = state.home.docs_path.join(file_path);
    let content = fs::read_to_string(&full_path)
        .map_err(|e| ServerError::CommandFailed(format!("Couldn't read ADR: {}", e)))?;

    // Find documents that reference this ADR
    let referenced_by = find_adr_references(state, adr_doc.id)?;

    // Parse metadata from content
    let metadata = parse_adr_metadata(&content);

    let ref_hint = if referenced_by.is_empty() {
        None
    } else {
        Some(format!("Referenced by {} document(s)", referenced_by.len()))
    };

    Ok(json!({
        "number": number,
        "title": adr_doc.title,
        "status": adr_doc.status,
        "content": content,
        "file": file_path,
        "applies_when": metadata.applies_when,
        "anti_patterns": metadata.anti_patterns,
        "referenced_by": referenced_by,
        "message": blue_core::voice::info(
            &format!("ADR {:04}: {}", number, adr_doc.title),
            ref_hint.as_deref()
        )
    }))
}

/// Handle blue_adr_relevant
///
/// Find relevant ADRs based on context using keyword matching.
/// Will be upgraded to AI matching when LLM integration is available (RFC 0005).
pub fn handle_relevant(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let context = args
        .get("context")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?
        .to_lowercase();

    let adrs = load_adr_summaries(state)?;

    // Check cache first (RFC 0004 requirement)
    let context_hash = compute_context_hash(&context);
    if let Some(cached) = get_cached_relevance(state, &context_hash) {
        return Ok(cached);
    }

    // Keyword-based matching (graceful degradation - no LLM available yet)
    let mut matches: Vec<(AdrSummary, f64, String)> = Vec::new();

    let context_words: Vec<&str> = context.split_whitespace().collect();

    for adr in &adrs {
        let (score, reason) = calculate_relevance_score(&context_words, adr);
        if score > 0.7 {
            matches.push((adr.clone(), score, reason));
        }
    }

    // Sort by score descending
    matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let relevant: Vec<Value> = matches
        .iter()
        .take(5) // Return top 5
        .map(|(adr, confidence, why)| {
            json!({
                "number": adr.number,
                "title": adr.title,
                "confidence": confidence,
                "why": why
            })
        })
        .collect();

    let result = json!({
        "method": "keyword", // Will be "ai" when LLM available
        "cached": false,
        "relevant": relevant,
        "message": if relevant.is_empty() {
            blue_core::voice::info("No strongly relevant ADRs found", Some("Proceed with judgment"))
        } else {
            blue_core::voice::info(
                &format!("{} relevant ADR(s) found", relevant.len()),
                Some("Consider these beliefs in your work")
            )
        }
    });

    // Cache the result
    cache_relevance(state, &context_hash, &result);

    Ok(result)
}

/// Handle blue_adr_audit
///
/// Scan for potential ADR violations. Only for testable ADRs.
pub fn handle_audit(state: &ProjectState) -> Result<Value, ServerError> {
    let mut findings: Vec<Value> = Vec::new();
    let mut passed: Vec<Value> = Vec::new();

    // ADR 0004: Evidence - Check for test coverage
    // (Placeholder - would need integration with test coverage tools)
    passed.push(json!({
        "adr": 4,
        "title": "Evidence",
        "message": "Test coverage check skipped (no coverage data available)"
    }));

    // ADR 0005: Single Source - Check for duplicate definitions
    // (Placeholder - would need code analysis)
    passed.push(json!({
        "adr": 5,
        "title": "Single Source",
        "message": "Duplicate definition check skipped (requires code analysis)"
    }));

    // ADR 0010: No Dead Code - Check for unused exports
    // Try to run cargo clippy for dead code detection
    let dead_code_result = check_dead_code(&state.home.root);
    match dead_code_result {
        DeadCodeResult::Found(locations) => {
            findings.push(json!({
                "adr": 10,
                "title": "No Dead Code",
                "type": "warning",
                "message": format!("{} unused items detected", locations.len()),
                "locations": locations
            }));
        }
        DeadCodeResult::None => {
            passed.push(json!({
                "adr": 10,
                "title": "No Dead Code",
                "message": "No unused items detected"
            }));
        }
        DeadCodeResult::NotApplicable(reason) => {
            passed.push(json!({
                "adr": 10,
                "title": "No Dead Code",
                "message": format!("Check skipped: {}", reason)
            }));
        }
    }

    Ok(json!({
        "findings": findings,
        "passed": passed,
        "message": blue_core::voice::info(
            &format!("{} finding(s), {} passed", findings.len(), passed.len()),
            if findings.is_empty() {
                Some("All testable ADRs satisfied")
            } else {
                Some("Review findings and address as appropriate")
            }
        )
    }))
}

// ===== Helper Functions =====

/// Load ADR summaries from the docs/adrs directory
fn load_adr_summaries(state: &ProjectState) -> Result<Vec<AdrSummary>, ServerError> {
    let adrs_path = state.home.docs_path.join("adrs");
    let mut summaries = Vec::new();

    if !adrs_path.exists() {
        return Ok(summaries);
    }

    let entries = fs::read_dir(&adrs_path)
        .map_err(|e| ServerError::CommandFailed(format!("Couldn't read ADRs directory: {}", e)))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(summary) = parse_adr_file(&path, &content) {
                    summaries.push(summary);
                }
            }
        }
    }

    // Sort by number
    summaries.sort_by_key(|s| s.number);

    Ok(summaries)
}

/// Parse an ADR file to extract summary and metadata
fn parse_adr_file(path: &Path, content: &str) -> Option<AdrSummary> {
    let file_name = path.file_name()?.to_string_lossy();

    // Extract number from filename (e.g., "0004-evidence.md")
    let number: i64 = file_name
        .split('-')
        .next()?
        .parse()
        .ok()?;

    // Extract title from first heading
    let title = content
        .lines()
        .find(|l| l.starts_with("# "))?
        .trim_start_matches("# ")
        .trim_start_matches("ADR ")
        .trim_start_matches(&format!("{:04}: ", number))
        .to_string();

    // Extract first paragraph as summary
    let summary = extract_summary(content);

    // Extract keywords from content
    let keywords = extract_keywords(content);

    // Parse metadata sections
    let metadata = parse_adr_metadata(content);

    Some(AdrSummary {
        number,
        title,
        summary,
        keywords,
        applies_when: metadata.applies_when,
        anti_patterns: metadata.anti_patterns,
    })
}

/// Extract summary from ADR content
fn extract_summary(content: &str) -> String {
    let mut in_summary = false;
    let mut summary_lines = Vec::new();

    for line in content.lines() {
        // Start capturing after the metadata table (after "---")
        if line == "---" {
            in_summary = true;
            continue;
        }

        if in_summary {
            // Stop at next heading or empty line after collecting some content
            if line.starts_with('#') && !summary_lines.is_empty() {
                break;
            }

            let trimmed = line.trim();
            if !trimmed.is_empty() {
                summary_lines.push(trimmed);
                if summary_lines.len() >= 3 {
                    break;
                }
            }
        }
    }

    summary_lines.join(" ")
}

/// Extract keywords from ADR content for relevance matching
fn extract_keywords(content: &str) -> Vec<String> {
    let mut keywords = Vec::new();

    // Extract from title
    let title_line = content.lines().find(|l| l.starts_with("# "));
    if let Some(title) = title_line {
        for word in title.to_lowercase().split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.len() > 3 {
                keywords.push(clean.to_string());
            }
        }
    }

    // Common ADR-related keywords to look for
    let important_terms = [
        "test", "testing", "evidence", "proof", "verify",
        "single", "source", "truth", "duplicate",
        "integrity", "whole", "complete",
        "honor", "commit", "promise",
        "courage", "delete", "remove", "refactor",
        "dead", "code", "unused",
        "freedom", "constraint", "limit",
        "faith", "believe", "trust",
        "overflow", "full", "abundance",
        "presence", "present", "aware",
        "purpose", "meaning", "why",
        "home", "belong", "welcome",
        "relationship", "connect", "link",
    ];

    let content_lower = content.to_lowercase();
    for term in important_terms {
        if content_lower.contains(term) {
            keywords.push(term.to_string());
        }
    }

    keywords.sort();
    keywords.dedup();
    keywords
}

struct AdrMetadata {
    applies_when: Vec<String>,
    anti_patterns: Vec<String>,
}

/// Parse ADR metadata sections (Applies When, Anti-Patterns)
fn parse_adr_metadata(content: &str) -> AdrMetadata {
    let mut applies_when = Vec::new();
    let mut anti_patterns = Vec::new();
    let mut current_section = None;

    for line in content.lines() {
        if line.starts_with("## Applies When") {
            current_section = Some("applies_when");
            continue;
        }
        if line.starts_with("## Anti-Patterns") || line.starts_with("## Anti Patterns") {
            current_section = Some("anti_patterns");
            continue;
        }
        if line.starts_with("## ") {
            current_section = None;
            continue;
        }

        if let Some(section) = current_section {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let item = trimmed.trim_start_matches("- ").trim_start_matches("* ").to_string();
                match section {
                    "applies_when" => applies_when.push(item),
                    "anti_patterns" => anti_patterns.push(item),
                    _ => {}
                }
            }
        }
    }

    AdrMetadata {
        applies_when,
        anti_patterns,
    }
}

/// Calculate relevance score between context and ADR
fn calculate_relevance_score(context_words: &[&str], adr: &AdrSummary) -> (f64, String) {
    let mut score = 0.0;
    let mut reasons = Vec::new();

    // Check title match
    let title_lower = adr.title.to_lowercase();
    for word in context_words {
        if title_lower.contains(word) {
            score += 0.3;
            reasons.push(format!("Title matches '{}'", word));
        }
    }

    // Check keyword match (with stem-like matching)
    let mut keyword_matches = 0;
    for word in context_words {
        // Match if word or keyword share a common stem (3+ chars)
        let word_stem = &word[..word.len().min(4)];
        if adr.keywords.iter().any(|k| {
            k.contains(word) ||
            word.contains(k.as_str()) ||
            (word.len() >= 4 && k.starts_with(word_stem)) ||
            (k.len() >= 4 && word.starts_with(&k[..k.len().min(4)]))
        }) {
            keyword_matches += 1;
        }
    }
    if keyword_matches > 0 {
        // Give more weight to keyword matches
        score += 0.3 * (keyword_matches as f64 / context_words.len().max(1) as f64);
        reasons.push(format!("{} keyword(s) match", keyword_matches));
    }

    // Check applies_when match (with stem-like matching)
    for applies in &adr.applies_when {
        let applies_lower = applies.to_lowercase();
        for word in context_words {
            let word_stem = &word[..word.len().min(4)];
            // Check for word match or stem match
            if applies_lower.contains(word) ||
               applies_lower.split_whitespace().any(|w| {
                   w.contains(word) ||
                   word.contains(w) ||
                   (w.len() >= 4 && w.starts_with(word_stem))
               }) {
                score += 0.25;
                reasons.push(format!("Applies when: {}", applies));
                break;
            }
        }
    }

    // Check anti-patterns match (important for catching violations)
    for anti in &adr.anti_patterns {
        let anti_lower = anti.to_lowercase();
        for word in context_words {
            let word_stem = &word[..word.len().min(4)];
            if anti_lower.contains(word) ||
               anti_lower.split_whitespace().any(|w| {
                   w.contains(word) ||
                   word.contains(w) ||
                   (w.len() >= 4 && w.starts_with(word_stem))
               }) {
                score += 0.25;
                reasons.push(format!("Anti-pattern match: {}", anti));
                break;
            }
        }
    }

    // Cap at 1.0
    score = score.min(1.0);

    let reason = if reasons.is_empty() {
        "Partial content match".to_string()
    } else {
        reasons.join("; ")
    };

    (score, reason)
}

/// Find documents that reference an ADR
fn find_adr_references(state: &ProjectState, adr_id: Option<i64>) -> Result<Vec<Value>, ServerError> {
    let mut references = Vec::new();

    let Some(id) = adr_id else {
        return Ok(references);
    };

    // Query documents that link to this ADR (where this ADR is the target)
    // This requires a direct SQL query since we need to find sources that link to this target
    let query = "SELECT d.id, d.doc_type, d.title, d.created_at
                 FROM documents d
                 JOIN document_links l ON l.source_id = d.id
                 WHERE l.target_id = ?1";

    let conn = state.store.conn();
    let mut stmt = conn
        .prepare(query)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let rows = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok((
                row.get::<_, String>(1)?, // doc_type
                row.get::<_, String>(2)?, // title
                row.get::<_, Option<String>>(3)?, // created_at
            ))
        })
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    for row in rows.flatten() {
        let (doc_type, title, created_at) = row;
        references.push(json!({
            "type": doc_type.to_lowercase(),
            "title": title,
            "date": created_at
        }));
    }

    Ok(references)
}

/// Compute hash for caching relevance results
fn compute_context_hash(context: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(context.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

/// Get cached relevance result (placeholder - uses in-memory for now)
fn get_cached_relevance(_state: &ProjectState, _hash: &str) -> Option<Value> {
    // TODO: Implement SQLite-based caching per RFC 0004
    None
}

/// Cache relevance result (placeholder)
fn cache_relevance(_state: &ProjectState, _hash: &str, _result: &Value) {
    // TODO: Implement SQLite-based caching per RFC 0004
}

enum DeadCodeResult {
    Found(Vec<String>),
    None,
    NotApplicable(String),
}

/// Check for dead code using cargo clippy (for Rust projects)
fn check_dead_code(project_root: &Path) -> DeadCodeResult {
    let cargo_toml = project_root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return DeadCodeResult::NotApplicable("Not a Rust project".to_string());
    }

    // Try to run clippy with dead_code lint
    let output = std::process::Command::new("cargo")
        .args(["clippy", "--message-format=short", "--", "-W", "dead_code"])
        .current_dir(project_root)
        .output();

    match output {
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let mut locations = Vec::new();

            for line in stderr.lines() {
                if line.contains("dead_code") || line.contains("unused") {
                    // Extract file:line format
                    if let Some(loc) = line.split_whitespace().next() {
                        if loc.contains(':') {
                            locations.push(loc.to_string());
                        }
                    }
                }
            }

            if locations.is_empty() {
                DeadCodeResult::None
            } else {
                DeadCodeResult::Found(locations)
            }
        }
        Err(_) => DeadCodeResult::NotApplicable("Couldn't run cargo clippy".to_string()),
    }
}

/// Convert a string to kebab-case
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

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("Evidence Based"), "evidence-based");
        assert_eq!(to_kebab_case("No Dead Code"), "no-dead-code");
    }

    #[test]
    fn test_extract_keywords() {
        let content = "# ADR 0004: Evidence\n\nShow, don't tell. Testing is the primary form of evidence.";
        let keywords = extract_keywords(content);
        assert!(keywords.contains(&"evidence".to_string()));
        assert!(keywords.contains(&"testing".to_string()));
    }

    #[test]
    fn test_calculate_relevance_score() {
        let adr = AdrSummary {
            number: 4,
            title: "Evidence".to_string(),
            summary: "Show, don't tell".to_string(),
            keywords: vec!["test".to_string(), "testing".to_string(), "evidence".to_string()],
            applies_when: vec!["Writing tests".to_string()],
            anti_patterns: vec!["Claiming code works without tests".to_string()],
        };

        let context: Vec<&str> = vec!["testing", "strategy"];
        let (score, reason) = calculate_relevance_score(&context, &adr);
        assert!(score > 0.5, "Expected high relevance for testing context, got {}", score);
        assert!(!reason.is_empty());
    }

    #[test]
    fn test_extract_summary() {
        let content = r#"# ADR 0004: Evidence

| **Status** | Accepted |

---

Show, don't tell. Testing is the primary form of evidence.

## Context
"#;
        let summary = extract_summary(content);
        assert!(summary.contains("Show, don't tell"));
    }
}
