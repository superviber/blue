//! MCP Resources handlers for Blue
//!
//! Implements resources/list and resources/read for blue:// URIs.
//! See RFC 0016 for the context injection architecture.
//! See RFC 0017 for dynamic context activation.

use std::sync::OnceLock;

use blue_core::{BlueUri, ContextManifest, ProjectState, estimate_tokens, read_uri_content};
use rand::Rng;
use serde_json::{json, Value};

use crate::error::ServerError;

/// Session ID for this MCP server lifecycle
/// Format: {repo}-{realm}-{random12} per RFC 0017
static SESSION_ID: OnceLock<String> = OnceLock::new();

/// Handle resources/list request
///
/// Returns a list of available blue:// URIs that can be read.
pub fn handle_resources_list(state: &ProjectState) -> Result<Value, ServerError> {
    let project_root = &state.home.root;
    let manifest = ContextManifest::load_or_default(project_root)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let mut resources = Vec::new();

    // Add identity tier sources
    for source in &manifest.identity.sources {
        let uri = &source.uri;
        let description = source.label.clone().unwrap_or_else(|| {
            format!("Identity context from {}", uri)
        });

        resources.push(json!({
            "uri": uri,
            "name": uri_to_name(uri),
            "description": description,
            "mimeType": "text/markdown"
        }));
    }

    // Add workflow tier sources
    for source in &manifest.workflow.sources {
        let uri = &source.uri;
        let description = source.label.clone().unwrap_or_else(|| {
            format!("Workflow context from {}", uri)
        });

        resources.push(json!({
            "uri": uri,
            "name": uri_to_name(uri),
            "description": description,
            "mimeType": "text/markdown"
        }));
    }

    // Add standard document types
    let doc_types = [
        ("blue://docs/adrs/", "All Architecture Decision Records"),
        ("blue://docs/rfcs/", "All RFCs"),
        ("blue://docs/spikes/", "All Spikes"),
        ("blue://docs/dialogues/", "All Dialogues"),
        ("blue://docs/runbooks/", "All Runbooks"),
        ("blue://docs/patterns/", "All Patterns"),
        ("blue://context/voice", "Voice patterns and tone"),
    ];

    for (uri, description) in doc_types {
        // Only add if not already in manifest sources
        let already_listed = manifest.identity.sources.iter().any(|s| s.uri == uri)
            || manifest.workflow.sources.iter().any(|s| s.uri == uri);

        if !already_listed {
            resources.push(json!({
                "uri": uri,
                "name": uri_to_name(uri),
                "description": description,
                "mimeType": "text/markdown"
            }));
        }
    }

    // Add state URIs
    resources.push(json!({
        "uri": "blue://state/current-rfc",
        "name": "Current RFC",
        "description": "The currently active RFC being worked on",
        "mimeType": "text/markdown"
    }));

    resources.push(json!({
        "uri": "blue://state/active-tasks",
        "name": "Active Tasks",
        "description": "Tasks from the current RFC that are not yet completed",
        "mimeType": "text/markdown"
    }));

    // Add plan files for in-progress RFCs (RFC 0019)
    if let Ok(docs) = state.store.list_documents(blue_core::DocType::Rfc) {
        for doc in docs.iter().filter(|d| d.status == "in-progress") {
            if let Some(num) = doc.number {
                let plan_path = blue_core::plan_file_path(&state.home.docs_path, &doc.title, num);
                if plan_path.exists() {
                    resources.push(json!({
                        "uri": format!("blue://docs/rfcs/{}/plan", num),
                        "name": format!("💙 Plan: {}", doc.title),
                        "description": format!("Task plan for RFC {:04}", num),
                        "mimeType": "text/markdown"
                    }));
                }
            }
        }
    }

    Ok(json!({
        "resources": resources
    }))
}

/// Handle resources/read request
///
/// Reads the content of a blue:// URI and returns it.
/// Implements staleness detection and rate limiting per RFC 0017.
pub fn handle_resources_read(state: &ProjectState, uri: &str) -> Result<Value, ServerError> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let project_root = &state.home.root;

    // Parse the URI
    let blue_uri = BlueUri::parse(uri)
        .map_err(|_| ServerError::InvalidParams)?;

    // Handle dynamic state URIs specially
    if blue_uri.is_dynamic() {
        return handle_state_uri(state, &blue_uri);
    }

    // Resolve to file paths
    let paths = blue_uri.resolve(project_root)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if paths.is_empty() {
        return Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/markdown",
                "text": format!("No content found for URI: {}", uri)
            }]
        }));
    }

    // Read and concatenate content
    let content = read_uri_content(&paths)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let tokens = estimate_tokens(&content);

    // Compute content hash for staleness detection
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let content_hash = format!("{:016x}", hasher.finish());

    // Check staleness and rate limiting
    let refresh_policy = get_refresh_policy(uri, None);
    let is_stale = should_refresh(state, uri, &content_hash);
    let refresh_allowed = is_refresh_allowed(state);

    // Determine if we should log this injection
    let should_log = match refresh_policy {
        blue_core::RefreshPolicy::Never => false,
        blue_core::RefreshPolicy::SessionStart => {
            // Only log if never injected in this session
            state.store.get_last_injection(get_session_id(state), uri)
                .map(|opt| opt.is_none())
                .unwrap_or(true)
        }
        _ => is_stale && refresh_allowed,
    };

    // Log the injection if appropriate
    if should_log {
        let _ = log_injection(state, uri, &content_hash, tokens);
    }

    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "text/markdown",
            "text": content
        }],
        "_meta": {
            "tokens": tokens,
            "is_stale": is_stale,
            "refresh_policy": format!("{:?}", refresh_policy),
            "session_id": get_session_id(state)
        }
    }))
}

/// Handle state URIs which require database queries
fn handle_state_uri(state: &ProjectState, blue_uri: &BlueUri) -> Result<Value, ServerError> {
    match blue_uri {
        BlueUri::State { entity } => {
            match entity.as_str() {
                "current-rfc" => {
                    // Get the current RFC from active session or most recent in-progress
                    let content = get_current_rfc_content(state)?;
                    Ok(json!({
                        "contents": [{
                            "uri": blue_uri.to_uri_string(),
                            "mimeType": "text/markdown",
                            "text": content
                        }]
                    }))
                }
                "active-tasks" => {
                    let content = get_active_tasks_content(state)?;
                    Ok(json!({
                        "contents": [{
                            "uri": blue_uri.to_uri_string(),
                            "mimeType": "text/markdown",
                            "text": content
                        }]
                    }))
                }
                _ => {
                    Ok(json!({
                        "contents": [{
                            "uri": blue_uri.to_uri_string(),
                            "mimeType": "text/markdown",
                            "text": format!("Unknown state entity: {}", entity)
                        }]
                    }))
                }
            }
        }
        _ => Err(ServerError::InvalidParams),
    }
}

/// Get the current RFC content
fn get_current_rfc_content(state: &ProjectState) -> Result<String, ServerError> {
    use blue_core::DocType;

    // Try to find an in-progress RFC
    let docs = state.store.list_documents(DocType::Rfc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let in_progress = docs.iter().find(|d| d.status == "in-progress");

    match in_progress {
        Some(doc) => {
            // Read the RFC file content
            if let Some(path) = &doc.file_path {
                let full_path = state.home.root.join(path);
                if full_path.exists() {
                    return std::fs::read_to_string(&full_path)
                        .map_err(|e| ServerError::StateLoadFailed(e.to_string()));
                }
            }

            // Fall back to generating summary
            Ok(format!(
                "# Current RFC: {}\n\nStatus: {}\n",
                doc.title, doc.status
            ))
        }
        None => {
            Ok("No RFC is currently in progress.\n\nUse `blue_rfc_create` to create a new RFC or `blue_rfc_update_status` to set one as in-progress.".to_string())
        }
    }
}

/// Get active tasks from the current RFC
fn get_active_tasks_content(state: &ProjectState) -> Result<String, ServerError> {
    use blue_core::DocType;

    // Find in-progress RFC
    let docs = state.store.list_documents(DocType::Rfc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let in_progress = docs.iter().find(|d| d.status == "in-progress");

    match in_progress {
        Some(doc) => {
            let doc_id = doc.id.ok_or(ServerError::StateLoadFailed("No document ID".to_string()))?;

            let tasks = state.store.get_tasks(doc_id)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            let incomplete: Vec<_> = tasks.iter()
                .filter(|t| !t.completed)
                .collect();

            if incomplete.is_empty() {
                return Ok(format!(
                    "# Active Tasks for: {}\n\nAll tasks are complete!\n",
                    doc.title
                ));
            }

            let mut content = format!("# Active Tasks for: {}\n\n", doc.title);
            for (i, task) in incomplete.iter().enumerate() {
                content.push_str(&format!("{}. [ ] {}\n", i + 1, task.description));
            }

            Ok(content)
        }
        None => {
            Ok("No RFC is currently in progress. No active tasks.".to_string())
        }
    }
}

/// Generate a composite session ID per RFC 0017
///
/// Format: {repo}-{realm}-{random12}
/// - repo: Project name from BlueHome
/// - realm: Realm name or "default"
/// - random12: 12 alphanumeric characters for uniqueness
fn generate_session_id(state: &ProjectState) -> String {
    const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    // Get repo name from project state
    let repo = state.home.project_name
        .as_ref()
        .map(|s| sanitize_id_component(s))
        .unwrap_or_else(|| "unknown".to_string());

    // For now, use "default" realm. Future: load from realm config
    let realm = "default";

    // Generate 12-character random suffix
    let mut rng = rand::thread_rng();
    let suffix: String = (0..12)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();

    format!("{}-{}-{}", repo, realm, suffix)
}

/// Sanitize a string for use in session ID (lowercase, alphanumeric, max 32 chars)
fn sanitize_id_component(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .take(32)
        .collect::<String>()
        .to_lowercase()
}

/// Get or initialize the session ID for this MCP lifecycle
pub fn get_session_id(state: &ProjectState) -> &str {
    SESSION_ID.get_or_init(|| generate_session_id(state))
}

/// Refresh rate limit in seconds (RFC 0017)
const REFRESH_COOLDOWN_SECS: u64 = 30;

/// Check if a refresh is allowed based on rate limiting
fn is_refresh_allowed(state: &ProjectState) -> bool {
    use chrono::{DateTime, Utc};

    let session_id = get_session_id(state);

    match state.store.get_last_refresh_time(session_id) {
        Ok(Some(timestamp)) => {
            // Parse the timestamp and check if cooldown has elapsed
            if let Ok(last_refresh) = DateTime::parse_from_rfc3339(&timestamp) {
                let elapsed = Utc::now().signed_duration_since(last_refresh.with_timezone(&Utc));
                elapsed.num_seconds() >= REFRESH_COOLDOWN_SECS as i64
            } else {
                true // Invalid timestamp, allow refresh
            }
        }
        Ok(None) => true, // No previous refresh, allow
        Err(_) => true,   // Error checking, allow refresh
    }
}

/// Check if content has changed since last injection (staleness detection)
fn should_refresh(state: &ProjectState, uri: &str, current_hash: &str) -> bool {
    let session_id = get_session_id(state);

    match state.store.get_last_injection(session_id, uri) {
        Ok(Some(injection)) => {
            // Content is stale if hash differs
            injection.content_hash != current_hash
        }
        Ok(None) => true, // Never injected, needs refresh
        Err(_) => true,   // Error checking, assume needs refresh
    }
}

/// Get the refresh policy for a document type and status
fn get_refresh_policy(uri: &str, _status: Option<&str>) -> blue_core::RefreshPolicy {
    use blue_core::RefreshPolicy;

    // Determine policy based on URI pattern (per RFC 0017)
    if uri.contains("/adrs/") {
        RefreshPolicy::SessionStart
    } else if uri.contains("/dialogues/") {
        RefreshPolicy::Never
    } else if uri.contains("/rfcs/") {
        // For RFCs, we'd ideally check status (draft/in-progress vs implemented)
        // For now, default to OnChange for active RFCs
        RefreshPolicy::OnChange
    } else if uri.contains("/spikes/") {
        RefreshPolicy::OnChange
    } else {
        RefreshPolicy::OnRequest
    }
}

/// Log a context injection to the audit trail
fn log_injection(state: &ProjectState, uri: &str, content_hash: &str, tokens: usize) -> Result<(), ServerError> {
    // Determine tier from URI
    let tier = determine_tier(uri);

    // Get session ID (generated once per MCP lifecycle)
    let session_id = get_session_id(state);

    // Log to database
    state.store.log_injection(session_id, tier, uri, content_hash, Some(tokens as i32))
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(())
}

/// Determine which tier a URI belongs to based on common patterns
fn determine_tier(uri: &str) -> &'static str {
    if uri.contains("/adrs/") || uri.contains("/context/voice") {
        "identity"
    } else if uri.contains("/state/") || uri.contains("/rfcs/") {
        "workflow"
    } else {
        "reference"
    }
}

/// Handle blue_context_status tool call (RFC 0017)
///
/// Returns context injection status including session ID, active injections,
/// staleness information, and relevance graph summary.
pub fn handle_context_status(state: &ProjectState) -> Result<Value, ServerError> {
    let session_id = get_session_id(state);

    // Get recent injections for this session
    let injections = state.store
        .get_session_injections(session_id, 10)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Get relevance edge count
    let edge_count = state.store
        .count_relevance_edges()
        .unwrap_or(0);

    // Format injection summary
    let injection_summary: Vec<Value> = injections.iter().map(|inj| {
        json!({
            "uri": inj.source_uri,
            "tier": inj.tier,
            "tokens": inj.token_count,
            "timestamp": inj.timestamp
        })
    }).collect();

    Ok(json!({
        "status": "success",
        "session": {
            "id": session_id,
            "injection_count": injections.len(),
            "injections": injection_summary
        },
        "relevance_graph": {
            "edge_count": edge_count
        },
        "rate_limit": {
            "cooldown_secs": REFRESH_COOLDOWN_SECS,
            "refresh_allowed": is_refresh_allowed(state)
        },
        "message": blue_core::voice::info(
            &format!("Session {} with {} injections", session_id, injections.len()),
            Some(&format!("{} edges in relevance graph", edge_count))
        )
    }))
}

/// Convert a URI to a human-readable name
fn uri_to_name(uri: &str) -> String {
    // Strip blue:// prefix and convert to readable form
    let path = uri.strip_prefix("blue://").unwrap_or(uri);
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    match parts.as_slice() {
        ["docs", doc_type] => format!("All {}", capitalize(doc_type)),
        ["docs", doc_type, id] => format!("{} {}", capitalize(doc_type).trim_end_matches('s'), id),
        ["context", scope] => format!("{} Context", capitalize(scope)),
        ["state", entity] => capitalize(&entity.replace('-', " ")),
        _ => path.to_string(),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_to_name() {
        assert_eq!(uri_to_name("blue://docs/adrs/"), "All Adrs");
        assert_eq!(uri_to_name("blue://docs/rfcs/0016"), "Rfc 0016");
        assert_eq!(uri_to_name("blue://context/voice"), "Voice Context");
        assert_eq!(uri_to_name("blue://state/current-rfc"), "Current rfc");
    }

    #[test]
    fn test_determine_tier() {
        assert_eq!(determine_tier("blue://docs/adrs/"), "identity");
        assert_eq!(determine_tier("blue://context/voice"), "identity");
        assert_eq!(determine_tier("blue://state/current-rfc"), "workflow");
        assert_eq!(determine_tier("blue://docs/rfcs/0016"), "workflow");
        assert_eq!(determine_tier("blue://docs/dialogues/"), "reference");
    }

    #[test]
    fn test_sanitize_id_component() {
        assert_eq!(sanitize_id_component("Blue"), "blue");
        assert_eq!(sanitize_id_component("my-project"), "my-project");
        assert_eq!(sanitize_id_component("My Project!"), "myproject");
        assert_eq!(sanitize_id_component("a".repeat(50).as_str()), "a".repeat(32));
    }

    #[test]
    fn test_session_id_format() {
        let state = ProjectState::for_test();
        let session_id = generate_session_id(&state);

        // Should be in format: repo-realm-random12
        let parts: Vec<&str> = session_id.split('-').collect();
        assert_eq!(parts.len(), 3, "Session ID should have 3 parts: {}", session_id);
        assert_eq!(parts[0], "test", "First part should be repo name");
        assert_eq!(parts[1], "default", "Second part should be realm name");
        assert_eq!(parts[2].len(), 12, "Random suffix should be 12 chars");

        // Random part should be alphanumeric
        assert!(parts[2].chars().all(|c| c.is_alphanumeric()));
    }
}
