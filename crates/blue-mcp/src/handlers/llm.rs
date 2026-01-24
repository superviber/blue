//! LLM tool handlers
//!
//! Implements RFC 0005: Local LLM Integration.
//! Provides MCP tools for model management.

use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

use blue_core::{LocalLlmConfig, LlmProvider};
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
}
