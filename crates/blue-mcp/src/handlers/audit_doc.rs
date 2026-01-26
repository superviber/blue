//! Audit document tool handlers
//!
//! Handles audit document creation and management.
//! Note: This is different from the health check (formerly blue_audit).

use std::fs;

use blue_core::{Audit, AuditType, DocType, Document, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_audit_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let audit_type_str = args
        .get("audit_type")
        .and_then(|v| v.as_str())
        .unwrap_or("custom");

    let scope = args
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("Project audit");

    let audit_type = AuditType::parse(audit_type_str)
        .unwrap_or(AuditType::Custom);

    // Create the audit
    let audit = Audit::new(title, audit_type, scope);

    // Generate filename with ISO 8601 timestamp (RFC 0031)
    let timestamp = blue_core::utc_timestamp();
    let filename = format!("audits/{}-{}.wip.md", timestamp, title);

    // Generate markdown
    let markdown = audit.to_markdown();

    // Write the file
    let docs_path = state.home.docs_path.clone();
    let audit_path = docs_path.join(&filename);
    if let Some(parent) = audit_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }
    if audit_path.exists() {
        return Err(ServerError::CommandFailed(format!(
            "File already exists: {}",
            audit_path.display()
        )));
    }
    fs::write(&audit_path, &markdown).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Add to store
    let mut doc = Document::new(DocType::Audit, title, "in-progress");
    doc.file_path = Some(filename.clone());

    let id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "id": id,
        "title": title,
        "audit_type": audit_type_str,
        "date": timestamp,
        "file": audit_path.display().to_string(),
        "markdown": markdown,
        "message": blue_core::voice::success(
            &format!("Created audit '{}'", title),
            Some("Document your findings.")
        )
    }))
}

/// Handle blue_audit_list
pub fn handle_list(state: &ProjectState) -> Result<Value, ServerError> {
    let audits = state
        .store
        .list_documents(DocType::Audit)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let items: Vec<Value> = audits
        .iter()
        .map(|doc| {
            json!({
                "id": doc.id,
                "title": doc.title,
                "status": doc.status,
                "file_path": doc.file_path,
                "created_at": doc.created_at,
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "count": items.len(),
        "audits": items,
        "message": if items.is_empty() {
            blue_core::voice::info("No audits found.", None::<&str>)
        } else {
            blue_core::voice::info(
                &format!("Found {} audit(s).", items.len()),
                None::<&str>
            )
        }
    }))
}

/// Handle blue_audit_get
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Audit, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Read the file content if it exists
    let content = if let Some(ref file_path) = doc.file_path {
        let full_path = state.home.docs_path.join(file_path);
        fs::read_to_string(&full_path).ok()
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "id": doc.id,
        "title": doc.title,
        "doc_status": doc.status,
        "file_path": doc.file_path,
        "content": content,
        "created_at": doc.created_at,
        "updated_at": doc.updated_at,
    }))
}

/// Handle blue_audit_complete
pub fn handle_complete(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Find the audit
    let doc = state
        .store
        .find_document(DocType::Audit, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update status in database
    state
        .store
        .update_document_status(DocType::Audit, title, "complete")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Rename file for new status (RFC 0031)
    let final_path = blue_core::rename_for_status(&state.home.docs_path, &state.store, &doc, "complete")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update markdown at effective path
    let effective_path = final_path.as_deref().or(doc.file_path.as_deref());
    if let Some(p) = effective_path {
        let _ = blue_core::update_markdown_status(&state.home.docs_path.join(p), "complete");
    }

    Ok(json!({
        "status": "success",
        "title": title,
        "new_status": "complete",
        "message": blue_core::voice::success(
            &format!("Completed audit '{}'", title),
            Some("Findings documented.")
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_requires_title() {
        let state = ProjectState::for_test();
        let args = json!({});

        let result = handle_create(&state, &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_empty() {
        let state = ProjectState::for_test();
        let result = handle_list(&state).unwrap();

        assert_eq!(result["status"], "success");
        assert_eq!(result["count"], 0);
    }
}
