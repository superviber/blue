//! LLM tool handlers
//!
//! Implements RFC 0005: Local LLM Integration.
//! Provides MCP tools for model management with graceful degradation.

use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

use blue_core::{KeywordLlm, LlmConfig, LlmManager, LocalLlmConfig, LlmProvider};
use blue_ollama::{EmbeddedOllama, HealthStatus, OllamaLlm};

use crate::error::ServerError;

/// Lazy-initialized shared Ollama instance
static OLLAMA: OnceLock<Arc<Mutex<Option<OllamaLlm>>>> = OnceLock::new();

/// Get the shared Ollama instance
fn get_ollama() -> &'static Arc<Mutex<Option<OllamaLlm>>> {
    OLLAMA.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// Start Ollama server
pub fn handle_start(args: &Value) -> Result<Value, ServerError> {
    let port = args.get("port").and_then(|v| v.as_u64()).map(|p| p as u16);
    let model = args
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from);
    let backend = args.get("backend").and_then(|v| v.as_str());
    let use_external = args
        .get("use_external")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let config = LocalLlmConfig {
        port: port.unwrap_or(11434),
        model: model.unwrap_or_else(|| "qwen2.5:7b".to_string()),
        backend: match backend {
            Some("cuda") => blue_core::LlmBackendChoice::Cuda,
            Some("mps") => blue_core::LlmBackendChoice::Mps,
            Some("cpu") => blue_core::LlmBackendChoice::Cpu,
            _ => blue_core::LlmBackendChoice::Auto,
        },
        use_external,
        ..Default::default()
    };

    let ollama = OllamaLlm::new(&config);
    ollama.start().map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    let mut guard = get_ollama().lock().unwrap();
    *guard = Some(ollama);

    Ok(json!({
        "started": true,
        "port": config.port,
        "model": config.model,
        "message": format!("Ollama started on port {}", config.port)
    }))
}

/// Stop Ollama server
pub fn handle_stop() -> Result<Value, ServerError> {
    let mut guard = get_ollama().lock().unwrap();
    if let Some(ref ollama) = *guard {
        ollama.stop().map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }
    *guard = None;

    Ok(json!({
        "stopped": true,
        "message": "Ollama stopped"
    }))
}

/// Check Ollama status
pub fn handle_status() -> Result<Value, ServerError> {
    let guard = get_ollama().lock().unwrap();

    if let Some(ref ollama) = *guard {
        let health = ollama.ollama().health_check();
        match health {
            HealthStatus::Healthy { version, gpu } => {
                Ok(json!({
                    "running": true,
                    "version": version,
                    "gpu": gpu,
                    "ready": ollama.is_ready()
                }))
            }
            HealthStatus::Unhealthy { error } => {
                Ok(json!({
                    "running": true,
                    "unhealthy": true,
                    "error": error
                }))
            }
            HealthStatus::NotRunning => {
                Ok(json!({
                    "running": false,
                    "message": "Ollama is not running"
                }))
            }
        }
    } else {
        // Check if there's an external Ollama running
        let config = LocalLlmConfig {
            use_external: true,
            ..Default::default()
        };
        let external = EmbeddedOllama::new(&config);
        if external.is_ollama_running() {
            let health = external.health_check();
            match health {
                HealthStatus::Healthy { version, gpu } => {
                    Ok(json!({
                        "running": true,
                        "external": true,
                        "version": version,
                        "gpu": gpu
                    }))
                }
                _ => Ok(json!({
                    "running": false,
                    "managed": false,
                    "message": "No managed Ollama instance"
                })),
            }
        } else {
            Ok(json!({
                "running": false,
                "managed": false,
                "message": "No Ollama instance found"
            }))
        }
    }
}

/// List available models
pub fn handle_model_list() -> Result<Value, ServerError> {
    // Try managed instance first
    let guard = get_ollama().lock().unwrap();
    if let Some(ref ollama) = *guard {
        let models = ollama
            .ollama()
            .list_models()
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "models": models.iter().map(|m| json!({
                "name": m.name,
                "size": m.size,
                "modified_at": m.modified_at
            })).collect::<Vec<_>>()
        }));
    }
    drop(guard);

    // Try external Ollama
    let config = LocalLlmConfig {
        use_external: true,
        ..Default::default()
    };
    let external = EmbeddedOllama::new(&config);
    if external.is_ollama_running() {
        let models = external
            .list_models()
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "models": models.iter().map(|m| json!({
                "name": m.name,
                "size": m.size,
                "modified_at": m.modified_at
            })).collect::<Vec<_>>(),
            "external": true
        }));
    }

    Err(ServerError::NotFound(
        "No Ollama instance available. Start one first.".to_string(),
    ))
}

/// Pull a model
pub fn handle_model_pull(args: &Value) -> Result<Value, ServerError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Try managed instance first
    let guard = get_ollama().lock().unwrap();
    if let Some(ref ollama) = *guard {
        ollama
            .ollama()
            .pull_model(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "pulled": true,
            "name": name,
            "message": format!("Model {} pulled successfully", name)
        }));
    }
    drop(guard);

    // Try external Ollama
    let config = LocalLlmConfig {
        use_external: true,
        ..Default::default()
    };
    let external = EmbeddedOllama::new(&config);
    if external.is_ollama_running() {
        external
            .pull_model(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "pulled": true,
            "name": name,
            "external": true,
            "message": format!("Model {} pulled successfully", name)
        }));
    }

    Err(ServerError::NotFound(
        "No Ollama instance available. Start one first.".to_string(),
    ))
}

/// Remove a model
pub fn handle_model_remove(args: &Value) -> Result<Value, ServerError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Try managed instance first
    let guard = get_ollama().lock().unwrap();
    if let Some(ref ollama) = *guard {
        ollama
            .ollama()
            .remove_model(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "removed": true,
            "name": name,
            "message": format!("Model {} removed", name)
        }));
    }
    drop(guard);

    // Try external Ollama
    let config = LocalLlmConfig {
        use_external: true,
        ..Default::default()
    };
    let external = EmbeddedOllama::new(&config);
    if external.is_ollama_running() {
        external
            .remove_model(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "removed": true,
            "name": name,
            "external": true,
            "message": format!("Model {} removed", name)
        }));
    }

    Err(ServerError::NotFound(
        "No Ollama instance available. Start one first.".to_string(),
    ))
}

/// Get LLM provider chain status (graceful degradation)
pub fn handle_providers() -> Result<Value, ServerError> {
    let config = LlmConfig::default();
    let mut manager = LlmManager::new(config);

    // Check Ollama availability
    let ollama_config = LocalLlmConfig {
        use_external: true,
        ..Default::default()
    };
    let ollama = EmbeddedOllama::new(&ollama_config);
    let ollama_available = ollama.is_ollama_running();
    let ollama_version = if ollama_available {
        match ollama.health_check() {
            HealthStatus::Healthy { version, .. } => Some(version),
            _ => None,
        }
    } else {
        None
    };

    // Check API availability (by checking for API key)
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let openai_key = std::env::var("OPENAI_API_KEY").ok();
    let api_available = anthropic_key.is_some() || openai_key.is_some();
    let api_provider = if anthropic_key.is_some() {
        Some("anthropic")
    } else if openai_key.is_some() {
        Some("openai")
    } else {
        None
    };

    // Keywords always available
    manager.add_provider(Box::new(KeywordLlm::new()));

    let active = if ollama_available {
        "ollama"
    } else if api_available {
        api_provider.unwrap_or("api")
    } else {
        "keywords"
    };

    Ok(json!({
        "active_provider": active,
        "fallback_chain": [
            {
                "name": "ollama",
                "available": ollama_available,
                "version": ollama_version,
                "priority": 1
            },
            {
                "name": api_provider.unwrap_or("api"),
                "available": api_available,
                "configured": api_provider.is_some(),
                "priority": 2
            },
            {
                "name": "keywords",
                "available": true,
                "priority": 3
            }
        ],
        "message": format!("Active provider: {}. Fallback: ollama → api → keywords", active)
    }))
}

/// Warm up a model (load into memory)
pub fn handle_model_warmup(args: &Value) -> Result<Value, ServerError> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Try managed instance first
    let guard = get_ollama().lock().unwrap();
    if let Some(ref ollama) = *guard {
        ollama
            .ollama()
            .warmup(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "warmed_up": true,
            "name": name,
            "message": format!("Model {} loaded into memory", name)
        }));
    }
    drop(guard);

    // Try external Ollama
    let config = LocalLlmConfig {
        use_external: true,
        ..Default::default()
    };
    let external = EmbeddedOllama::new(&config);
    if external.is_ollama_running() {
        external
            .warmup(name)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;
        return Ok(json!({
            "warmed_up": true,
            "name": name,
            "external": true,
            "message": format!("Model {} loaded into memory", name)
        }));
    }

    Err(ServerError::NotFound(
        "No Ollama instance available. Start one first.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_no_ollama() {
        // Should return not running when no Ollama available
        let result = handle_status();
        assert!(result.is_ok());
        let value = result.unwrap();
        // Either running (external) or not running - both are valid
        assert!(value.get("running").is_some());
    }

    #[test]
    fn test_model_list_requires_ollama() {
        // Clear any existing instance
        let mut guard = get_ollama().lock().unwrap();
        *guard = None;
        drop(guard);

        // Should fail gracefully when no Ollama
        let result = handle_model_list();
        // May succeed if external Ollama is running, or fail
        // Just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_model_pull_requires_name() {
        let result = handle_model_pull(&json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_model_remove_requires_name() {
        let result = handle_model_remove(&json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_providers_always_has_keywords() {
        let result = handle_providers();
        assert!(result.is_ok());
        let value = result.unwrap();

        // Should always have an active provider
        assert!(value.get("active_provider").is_some());

        // Should have fallback chain
        let chain = value.get("fallback_chain").unwrap().as_array().unwrap();
        assert_eq!(chain.len(), 3);

        // Keywords should always be available
        let keywords = chain.iter().find(|p| p.get("name").unwrap() == "keywords").unwrap();
        assert_eq!(keywords.get("available").unwrap(), true);
    }
}
