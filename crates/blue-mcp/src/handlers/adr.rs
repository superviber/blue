//! ADR tool handlers
//!
//! Handles Architecture Decision Record creation.

use std::fs;

use blue_core::{Adr, DocType, Document, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

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
        .next_number(DocType::Adr)
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
