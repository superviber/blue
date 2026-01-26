//! Blue URI resolution
//!
//! Handles `blue://` URIs for context injection.
//!
//! URI patterns:
//! - `blue://docs/{type}/` - All documents of a type
//! - `blue://docs/{type}/{id}` - Specific document by ID/title
//! - `blue://context/{scope}` - Injection bundles (voice, relevance)
//! - `blue://state/{entity}` - Live state (current-rfc, active-tasks)
//! - `blue://{plugin}/` - Plugin-provided context

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Errors that can occur during URI resolution
#[derive(Debug, Error)]
pub enum UriError {
    #[error("Invalid URI format: {0}")]
    InvalidFormat(String),

    #[error("Unknown URI scheme: {0}")]
    UnknownScheme(String),

    #[error("Unknown document type: {0}")]
    UnknownDocType(String),

    #[error("Unknown context scope: {0}")]
    UnknownScope(String),

    #[error("Unknown state entity: {0}")]
    UnknownEntity(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    PathNotFound(String),
}

/// A parsed Blue URI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlueUri {
    /// Reference to documents: `blue://docs/{type}/` or `blue://docs/{type}/{id}`
    Docs {
        doc_type: String,
        id: Option<String>,
    },

    /// Reference to a context bundle: `blue://context/{scope}`
    Context { scope: String },

    /// Reference to live state: `blue://state/{entity}`
    State { entity: String },

    /// Reference to plugin content: `blue://{plugin}/{path}`
    Plugin { name: String, path: String },
}

impl BlueUri {
    /// Parse a URI string into a BlueUri
    pub fn parse(uri: &str) -> Result<Self, UriError> {
        // Must start with blue://
        if !uri.starts_with("blue://") {
            return Err(UriError::UnknownScheme(uri.to_string()));
        }

        let path = &uri[7..]; // Strip "blue://"
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            return Err(UriError::InvalidFormat("Empty URI path".to_string()));
        }

        match parts[0] {
            "docs" => {
                if parts.len() < 2 {
                    return Err(UriError::InvalidFormat(
                        "docs URI requires a document type".to_string(),
                    ));
                }
                let doc_type = parts[1].to_string();
                let id = if parts.len() > 2 {
                    Some(parts[2..].join("/"))
                } else {
                    None
                };
                Ok(BlueUri::Docs { doc_type, id })
            }
            "context" => {
                if parts.len() < 2 {
                    return Err(UriError::InvalidFormat(
                        "context URI requires a scope".to_string(),
                    ));
                }
                Ok(BlueUri::Context {
                    scope: parts[1..].join("/"),
                })
            }
            "state" => {
                if parts.len() < 2 {
                    return Err(UriError::InvalidFormat(
                        "state URI requires an entity".to_string(),
                    ));
                }
                Ok(BlueUri::State {
                    entity: parts[1..].join("/"),
                })
            }
            // Anything else is a plugin
            plugin => Ok(BlueUri::Plugin {
                name: plugin.to_string(),
                path: if parts.len() > 1 {
                    parts[1..].join("/")
                } else {
                    String::new()
                },
            }),
        }
    }

    /// Resolve the URI to file paths relative to a project root
    ///
    /// Returns a list of paths that match the URI pattern.
    pub fn resolve(&self, project_root: &Path) -> Result<Vec<PathBuf>, UriError> {
        let docs_dir = project_root.join(".blue").join("docs");

        match self {
            BlueUri::Docs { doc_type, id } => {
                let type_dir = match doc_type.as_str() {
                    "adrs" | "adr" => docs_dir.join("adrs"),
                    "rfcs" | "rfc" => docs_dir.join("rfcs"),
                    "spikes" | "spike" => docs_dir.join("spikes"),
                    "dialogues" | "dialogue" => docs_dir.join("dialogues"),
                    "runbooks" | "runbook" => docs_dir.join("runbooks"),
                    "patterns" | "pattern" => docs_dir.join("patterns"),
                    _ => {
                        return Err(UriError::UnknownDocType(doc_type.clone()));
                    }
                };

                if !type_dir.exists() {
                    return Ok(Vec::new());
                }

                match id {
                    Some(id) => {
                        // RFC 0019: Check for /plan suffix to return plan file
                        if id.ends_with("/plan") {
                            let rfc_num = id.trim_end_matches("/plan");
                            // Find the RFC file to get its title
                            let entries = std::fs::read_dir(&type_dir)?;
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                                    if let Some(num_str) = name.split('-').next() {
                                        if num_str == rfc_num
                                            || num_str.trim_start_matches('0') == rfc_num
                                        {
                                            // Found the RFC, now get its plan file
                                            let plan_name = format!("{}.plan.md", name);
                                            let plan_path = type_dir.join(plan_name);
                                            if plan_path.exists() {
                                                return Ok(vec![plan_path]);
                                            }
                                        }
                                    }
                                }
                            }
                            return Ok(Vec::new());
                        }

                        // Specific document - try exact match or pattern match
                        let exact = type_dir.join(format!("{}.md", id));
                        if exact.exists() {
                            return Ok(vec![exact]);
                        }

                        // Try with number prefix (e.g., "0001-title")
                        let entries = std::fs::read_dir(&type_dir)?;
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                                // Check if name contains the id (case-insensitive)
                                if name.to_lowercase().contains(&id.to_lowercase()) {
                                    return Ok(vec![path]);
                                }
                                // Check if the number portion matches
                                if let Some(num_str) = name.split('-').next() {
                                    if num_str == id
                                        || num_str.trim_start_matches('0') == id
                                    {
                                        return Ok(vec![path]);
                                    }
                                }
                            }
                        }

                        Ok(Vec::new())
                    }
                    None => {
                        // All documents in directory
                        let mut paths = Vec::new();
                        let entries = std::fs::read_dir(&type_dir)?;
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().map(|e| e == "md").unwrap_or(false) {
                                paths.push(path);
                            }
                        }
                        paths.sort();
                        Ok(paths)
                    }
                }
            }
            BlueUri::Context { scope } => {
                // Context bundles are generated or special locations
                match scope.as_str() {
                    "voice" => {
                        // Voice patterns from docs/patterns
                        let patterns_dir = docs_dir.join("patterns");
                        if patterns_dir.exists() {
                            let entries = std::fs::read_dir(&patterns_dir)?;
                            let paths: Vec<PathBuf> = entries
                                .flatten()
                                .map(|e| e.path())
                                .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
                                .collect();
                            Ok(paths)
                        } else {
                            Ok(Vec::new())
                        }
                    }
                    "relevance" => {
                        // Relevance graph - not a file, computed at runtime
                        Ok(Vec::new())
                    }
                    _ => Err(UriError::UnknownScope(scope.clone())),
                }
            }
            BlueUri::State { entity } => {
                // State URIs resolve to database queries, not files
                // Return empty - the caller should use the DocumentStore
                match entity.as_str() {
                    "current-rfc" | "active-tasks" | "active-session" => Ok(Vec::new()),
                    _ => Err(UriError::UnknownEntity(entity.clone())),
                }
            }
            BlueUri::Plugin { .. } => {
                // Plugin URIs are handled by plugin resolvers
                Ok(Vec::new())
            }
        }
    }

    /// Check if this URI references dynamic state (requires database lookup)
    pub fn is_dynamic(&self) -> bool {
        matches!(self, BlueUri::State { .. })
    }

    /// Check if this URI is a plugin reference
    pub fn is_plugin(&self) -> bool {
        matches!(self, BlueUri::Plugin { .. })
    }

    /// Get the URI as a string
    pub fn to_uri_string(&self) -> String {
        match self {
            BlueUri::Docs { doc_type, id } => match id {
                Some(id) => format!("blue://docs/{}/{}", doc_type, id),
                None => format!("blue://docs/{}/", doc_type),
            },
            BlueUri::Context { scope } => format!("blue://context/{}", scope),
            BlueUri::State { entity } => format!("blue://state/{}", entity),
            BlueUri::Plugin { name, path } => {
                if path.is_empty() {
                    format!("blue://{}/", name)
                } else {
                    format!("blue://{}/{}", name, path)
                }
            }
        }
    }
}

/// Read content from resolved paths and concatenate with separators
pub fn read_uri_content(paths: &[PathBuf]) -> Result<String, UriError> {
    let mut content = String::new();
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            content.push_str("\n---\n\n");
        }
        content.push_str(&std::fs::read_to_string(path)?);
    }
    Ok(content)
}

/// Estimate token count for content (rough approximation: ~4 chars per token)
pub fn estimate_tokens(content: &str) -> usize {
    content.len() / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_docs_uri() {
        let uri = BlueUri::parse("blue://docs/adrs/").unwrap();
        assert_eq!(
            uri,
            BlueUri::Docs {
                doc_type: "adrs".to_string(),
                id: None
            }
        );

        let uri = BlueUri::parse("blue://docs/rfcs/0016").unwrap();
        assert_eq!(
            uri,
            BlueUri::Docs {
                doc_type: "rfcs".to_string(),
                id: Some("0016".to_string())
            }
        );
    }

    #[test]
    fn test_parse_context_uri() {
        let uri = BlueUri::parse("blue://context/voice").unwrap();
        assert_eq!(
            uri,
            BlueUri::Context {
                scope: "voice".to_string()
            }
        );
    }

    #[test]
    fn test_parse_state_uri() {
        let uri = BlueUri::parse("blue://state/current-rfc").unwrap();
        assert_eq!(
            uri,
            BlueUri::State {
                entity: "current-rfc".to_string()
            }
        );
    }

    #[test]
    fn test_parse_plugin_uri() {
        let uri = BlueUri::parse("blue://jira/PROJECT-123").unwrap();
        assert_eq!(
            uri,
            BlueUri::Plugin {
                name: "jira".to_string(),
                path: "PROJECT-123".to_string()
            }
        );
    }

    #[test]
    fn test_invalid_scheme() {
        let result = BlueUri::parse("http://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_to_uri_string() {
        let uri = BlueUri::Docs {
            doc_type: "adrs".to_string(),
            id: None,
        };
        assert_eq!(uri.to_uri_string(), "blue://docs/adrs/");

        let uri = BlueUri::Docs {
            doc_type: "rfcs".to_string(),
            id: Some("0016".to_string()),
        };
        assert_eq!(uri.to_uri_string(), "blue://docs/rfcs/0016");
    }
}
