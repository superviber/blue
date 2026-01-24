//! Semantic index tool handlers (RFC 0010)
//!
//! Handles file indexing, search, and impact analysis.

use blue_core::store::{FileIndexEntry, SymbolIndexEntry, INDEX_PROMPT_VERSION};
use blue_core::ProjectState;
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_index_status
pub fn handle_status(state: &ProjectState) -> Result<Value, ServerError> {
    let realm = "default";
    let (file_count, symbol_count) = state
        .store
        .get_index_stats(realm)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "indexed_files": file_count,
        "indexed_symbols": symbol_count,
        "prompt_version": INDEX_PROMPT_VERSION,
        "message": if file_count == 0 {
            "Index is empty. Run 'blue index --all' to bootstrap."
        } else {
            "Index ready."
        }
    }))
}

/// Handle blue_index_search
pub fn handle_search(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;

    let symbols_only = args
        .get("symbols_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let realm = "default";

    if symbols_only {
        let results = state
            .store
            .search_symbols(realm, query, limit)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let formatted: Vec<Value> = results
            .iter()
            .map(|(sym, file)| {
                json!({
                    "name": sym.name,
                    "kind": sym.kind,
                    "file": file.file_path,
                    "start_line": sym.start_line,
                    "end_line": sym.end_line,
                    "description": sym.description
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "query": query,
            "type": "symbols",
            "count": formatted.len(),
            "results": formatted
        }))
    } else {
        let results = state
            .store
            .search_file_index(realm, query, limit)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let formatted: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "file": r.file_entry.file_path,
                    "summary": r.file_entry.summary,
                    "relationships": r.file_entry.relationships,
                    "score": r.score
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "query": query,
            "type": "files",
            "count": formatted.len(),
            "results": formatted
        }))
    }
}

/// Handle blue_index_impact
pub fn handle_impact(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let file_path = args
        .get("file")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let realm = "default";

    // Get the file index entry
    let entry = state
        .store
        .get_file_index(realm, realm, file_path)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    match entry {
        Some(file_entry) => {
            // Get symbols for this file
            let symbols = if let Some(id) = file_entry.id {
                state
                    .store
                    .get_file_symbols(id)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?
            } else {
                vec![]
            };

            let symbol_values: Vec<Value> = symbols
                .iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "kind": s.kind,
                        "start_line": s.start_line,
                        "end_line": s.end_line,
                        "description": s.description
                    })
                })
                .collect();

            // Search for files that reference this file
            let filename = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);

            let references = state
                .store
                .search_file_index(realm, filename, 20)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            let referencing_files: Vec<String> = references
                .into_iter()
                .filter(|r| r.file_entry.file_path != file_path)
                .map(|r| r.file_entry.file_path)
                .collect();

            Ok(json!({
                "status": "success",
                "file": file_path,
                "summary": file_entry.summary,
                "relationships": file_entry.relationships,
                "symbols": symbol_values,
                "referenced_by": referencing_files,
                "indexed_at": file_entry.indexed_at
            }))
        }
        None => Ok(json!({
            "status": "not_indexed",
            "file": file_path,
            "message": format!("File '{}' is not indexed. Run 'blue index --file {}' to index it.", file_path, file_path)
        })),
    }
}

/// Handle blue_index_file (store index data for a file)
pub fn handle_index_file(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let file_hash = args
        .get("file_hash")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let summary = args.get("summary").and_then(|v| v.as_str());
    let relationships = args.get("relationships").and_then(|v| v.as_str());

    let realm = "default";
    let repo = "default";

    // Create the file index entry
    let mut entry = FileIndexEntry::new(realm, repo, file_path, file_hash);
    entry.summary = summary.map(|s| s.to_string());
    entry.relationships = relationships.map(|s| s.to_string());

    // Upsert the entry
    let file_id = state
        .store
        .upsert_file_index(&entry)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Parse and store symbols if provided
    if let Some(symbols_array) = args.get("symbols").and_then(|v| v.as_array()) {
        let symbols: Vec<SymbolIndexEntry> = symbols_array
            .iter()
            .filter_map(|s| {
                let name = s.get("name")?.as_str()?;
                let kind = s.get("kind")?.as_str()?;
                Some(SymbolIndexEntry {
                    id: None,
                    file_id,
                    name: name.to_string(),
                    kind: kind.to_string(),
                    start_line: s.get("start_line").and_then(|v| v.as_i64()).map(|v| v as i32),
                    end_line: s.get("end_line").and_then(|v| v.as_i64()).map(|v| v as i32),
                    description: s.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            })
            .collect();

        state
            .store
            .set_file_symbols(file_id, &symbols)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "file": file_path,
            "file_id": file_id,
            "symbols_indexed": symbols.len(),
            "message": format!("Indexed '{}' with {} symbols.", file_path, symbols.len())
        }))
    } else {
        Ok(json!({
            "status": "success",
            "file": file_path,
            "file_id": file_id,
            "symbols_indexed": 0,
            "message": format!("Indexed '{}'.", file_path)
        }))
    }
}

/// Handle blue_index_realm (list all indexed files)
pub fn handle_index_realm(state: &ProjectState, _args: &Value) -> Result<Value, ServerError> {
    let realm = "default";

    let entries = state
        .store
        .list_file_index(realm, None)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let formatted: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "file": e.file_path,
                "hash": e.file_hash,
                "summary": e.summary,
                "indexed_at": e.indexed_at
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "realm": realm,
        "count": formatted.len(),
        "files": formatted
    }))
}
