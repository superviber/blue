//! Semantic file indexer (RFC 0010)
//!
//! Uses Ollama with qwen2.5:3b to analyze source files and extract:
//! - Summary: one-sentence description
//! - Relationships: dependencies and connections to other files
//! - Symbols: functions, structs, classes with line numbers

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::store::{DocumentStore, FileIndexEntry, SymbolIndexEntry};
use crate::{CompletionOptions, LlmError, LlmProvider};

/// Default model for indexing
pub const DEFAULT_INDEX_MODEL: &str = "qwen2.5:3b";

/// Maximum file size in lines before partial indexing
pub const MAX_FILE_LINES: usize = 1000;

/// Indexer configuration
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub model: String,
    pub realm: String,
    pub repo: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_INDEX_MODEL.to_string(),
            realm: "default".to_string(),
            repo: "default".to_string(),
            max_tokens: 2048,
            temperature: 0.1, // Low temperature for consistent structured output
        }
    }
}

/// Result of indexing a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResult {
    pub file_path: String,
    pub file_hash: String,
    pub summary: Option<String>,
    pub relationships: Option<String>,
    pub symbols: Vec<ParsedSymbol>,
    pub is_partial: bool,
    pub error: Option<String>,
}

/// A parsed symbol from AI output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub description: Option<String>,
}

/// The indexer that uses LLM to analyze files
pub struct Indexer<P: LlmProvider> {
    provider: P,
    config: IndexerConfig,
}

impl<P: LlmProvider> Indexer<P> {
    /// Create a new indexer with the given LLM provider
    pub fn new(provider: P, config: IndexerConfig) -> Self {
        Self { provider, config }
    }

    /// Index a single file and return the result
    pub fn index_file(&self, file_path: &Path) -> Result<IndexResult, IndexerError> {
        let path_str = file_path.to_string_lossy().to_string();

        // Read file contents
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| IndexerError::FileRead(path_str.clone(), e.to_string()))?;

        // Calculate hash
        let file_hash = hash_content(&content);

        // Check file size
        let line_count = content.lines().count();
        let is_partial = line_count > MAX_FILE_LINES;

        let content_to_index = if is_partial {
            // Take first MAX_FILE_LINES lines
            content.lines().take(MAX_FILE_LINES).collect::<Vec<_>>().join("\n")
        } else {
            content.clone()
        };

        // Generate prompt
        let prompt = generate_index_prompt(&path_str, &content_to_index, is_partial);

        // Call LLM
        let options = CompletionOptions {
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            stop_sequences: vec!["```".to_string()], // Stop at end of YAML block
        };

        let completion = self.provider.complete(&prompt, &options)
            .map_err(|e| IndexerError::LlmError(e))?;

        // Parse YAML response
        let parsed = parse_index_response(&completion.text);

        Ok(IndexResult {
            file_path: path_str,
            file_hash,
            summary: parsed.summary,
            relationships: parsed.relationships,
            symbols: parsed.symbols,
            is_partial,
            error: parsed.error,
        })
    }

    /// Index a file and store in the database
    pub fn index_and_store(
        &self,
        file_path: &Path,
        store: &DocumentStore,
    ) -> Result<IndexResult, IndexerError> {
        let result = self.index_file(file_path)?;

        // Create file index entry
        let mut entry = FileIndexEntry::new(
            &self.config.realm,
            &self.config.repo,
            &result.file_path,
            &result.file_hash,
        );
        entry.summary = result.summary.clone();
        entry.relationships = result.relationships.clone();

        // Store in database
        let file_id = store.upsert_file_index(&entry)
            .map_err(|e| IndexerError::StoreError(e.to_string()))?;

        // Convert and store symbols
        let symbols: Vec<SymbolIndexEntry> = result.symbols.iter().map(|s| {
            SymbolIndexEntry {
                id: None,
                file_id,
                name: s.name.clone(),
                kind: s.kind.clone(),
                start_line: s.start_line,
                end_line: s.end_line,
                description: s.description.clone(),
            }
        }).collect();

        store.set_file_symbols(file_id, &symbols)
            .map_err(|e| IndexerError::StoreError(e.to_string()))?;

        info!("Indexed {} with {} symbols", result.file_path, symbols.len());

        Ok(result)
    }

    /// Check if a file needs re-indexing
    pub fn needs_indexing(&self, file_path: &Path, store: &DocumentStore) -> Result<bool, IndexerError> {
        let path_str = file_path.to_string_lossy().to_string();

        // Read file and calculate hash
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| IndexerError::FileRead(path_str.clone(), e.to_string()))?;
        let current_hash = hash_content(&content);

        // Check against stored hash
        store.is_file_stale(&self.config.realm, &self.config.repo, &path_str, &current_hash)
            .map_err(|e| IndexerError::StoreError(e.to_string()))
    }
}

/// Generate the indexing prompt
fn generate_index_prompt(file_path: &str, content: &str, is_partial: bool) -> String {
    let partial_note = if is_partial {
        "\n\nNote: This is a large file. Only the first 1000 lines are shown. Include a note about this in the summary."
    } else {
        ""
    };

    format!(
        r#"Analyze this source file and provide structured information about it.

File: {file_path}{partial_note}

```
{content}
```

Provide your analysis as YAML with this exact structure:

```yaml
summary: "One sentence describing what this file does"

relationships: |
  Describe how this file relates to other files.
  List imports, dependencies, and what uses this file.
  Be specific about file names when visible.

symbols:
  - name: "SymbolName"
    kind: "function|struct|class|enum|const|trait|interface|type|method"
    start_line: 10
    end_line: 25
    description: "What this symbol does"
```

Rules:
- Summary must be ONE sentence
- Relationships should mention specific file names when imports are visible
- Only include significant symbols (skip trivial helpers, private internals)
- Line numbers must be accurate
- Kind must be one of: function, struct, class, enum, const, trait, interface, type, method
- Output valid YAML only"#
    )
}

/// Parsed response from the LLM
#[derive(Debug, Default)]
struct ParsedResponse {
    summary: Option<String>,
    relationships: Option<String>,
    symbols: Vec<ParsedSymbol>,
    error: Option<String>,
}

/// Parse the YAML response from the LLM
fn parse_index_response(response: &str) -> ParsedResponse {
    // Try to find YAML block
    let yaml_content = if let Some(start) = response.find("```yaml") {
        let after_marker = &response[start + 7..];
        if let Some(end) = after_marker.find("```") {
            after_marker[..end].trim()
        } else {
            after_marker.trim()
        }
    } else if let Some(start) = response.find("summary:") {
        // No code fence, but starts with summary
        response[start..].trim()
    } else {
        response.trim()
    };

    // Parse YAML
    match serde_yaml::from_str::<serde_yaml::Value>(yaml_content) {
        Ok(value) => {
            let summary = value.get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let relationships = value.get("relationships")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string());

            let symbols = value.get("symbols")
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter().filter_map(|item| {
                        let name = item.get("name")?.as_str()?.to_string();
                        let kind = item.get("kind")?.as_str()?.to_string();

                        Some(ParsedSymbol {
                            name,
                            kind,
                            start_line: item.get("start_line")
                                .and_then(|v| v.as_i64())
                                .map(|n| n as i32),
                            end_line: item.get("end_line")
                                .and_then(|v| v.as_i64())
                                .map(|n| n as i32),
                            description: item.get("description")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        })
                    }).collect()
                })
                .unwrap_or_default();

            ParsedResponse {
                summary,
                relationships,
                symbols,
                error: None,
            }
        }
        Err(e) => {
            warn!("Failed to parse YAML response: {}", e);
            debug!("Response was: {}", yaml_content);

            ParsedResponse {
                summary: None,
                relationships: None,
                symbols: vec![],
                error: Some(format!("YAML parse error: {}", e)),
            }
        }
    }
}

/// Calculate hash of file content
fn hash_content(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Indexer errors
#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("Failed to read file '{0}': {1}")]
    FileRead(String, String),

    #[error("LLM error: {0}")]
    LlmError(#[from] LlmError),

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("Index error: {0}")]
    Other(String),
}

/// File extensions we should index
pub fn is_indexable_file(path: &Path) -> bool {
    let extensions: &[&str] = &[
        "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp",
        "rb", "php", "swift", "kt", "scala", "clj", "ex", "exs", "erl", "hs",
        "ml", "mli", "sql", "sh", "bash", "zsh", "yaml", "yml", "toml", "json",
    ];

    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| extensions.contains(&e))
        .unwrap_or(false)
}

/// Directories to skip when indexing
pub fn should_skip_dir(name: &str) -> bool {
    let skip_dirs: &[&str] = &[
        "node_modules", "target", ".git", "__pycache__", "venv", ".venv",
        "dist", "build", ".next", ".nuxt", "vendor", ".cargo", ".blue",
    ];

    skip_dirs.contains(&name) || name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("hello");
        let hash2 = hash_content("hello");
        let hash3 = hash_content("world");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_is_indexable_file() {
        assert!(is_indexable_file(Path::new("foo.rs")));
        assert!(is_indexable_file(Path::new("bar.py")));
        assert!(is_indexable_file(Path::new("baz.ts")));
        assert!(!is_indexable_file(Path::new("readme.md")));
        assert!(!is_indexable_file(Path::new("image.png")));
    }

    #[test]
    fn test_should_skip_dir() {
        assert!(should_skip_dir("node_modules"));
        assert!(should_skip_dir("target"));
        assert!(should_skip_dir(".git"));
        assert!(should_skip_dir(".hidden"));
        assert!(!should_skip_dir("src"));
        assert!(!should_skip_dir("lib"));
    }

    #[test]
    fn test_parse_index_response_valid() {
        let response = r#"```yaml
summary: "This file handles user authentication"

relationships: |
  Imports auth module from ./auth.rs
  Used by main.rs for login flow

symbols:
  - name: "authenticate"
    kind: "function"
    start_line: 10
    end_line: 25
    description: "Validates user credentials"
```"#;

        let parsed = parse_index_response(response);
        assert_eq!(parsed.summary, Some("This file handles user authentication".to_string()));
        assert!(parsed.relationships.is_some());
        assert_eq!(parsed.symbols.len(), 1);
        assert_eq!(parsed.symbols[0].name, "authenticate");
        assert_eq!(parsed.symbols[0].kind, "function");
        assert_eq!(parsed.symbols[0].start_line, Some(10));
    }

    #[test]
    fn test_parse_index_response_no_fence() {
        let response = r#"summary: "Test file"

relationships: |
  No dependencies

symbols: []"#;

        let parsed = parse_index_response(response);
        assert_eq!(parsed.summary, Some("Test file".to_string()));
        assert!(parsed.symbols.is_empty());
    }

    #[test]
    fn test_parse_index_response_invalid() {
        let response = "this is not valid yaml { broken }";
        let parsed = parse_index_response(response);
        assert!(parsed.error.is_some());
    }

    #[test]
    fn test_generate_index_prompt() {
        let prompt = generate_index_prompt("test.rs", "fn main() {}", false);
        assert!(prompt.contains("test.rs"));
        assert!(prompt.contains("fn main()"));
        assert!(!prompt.contains("large file"));
    }

    #[test]
    fn test_generate_index_prompt_partial() {
        let prompt = generate_index_prompt("test.rs", "fn main() {}", true);
        assert!(prompt.contains("large file"));
    }
}
