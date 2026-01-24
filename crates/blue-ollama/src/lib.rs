//! Blue Ollama - Embedded Ollama Server Management
//!
//! Implements RFC 0005: Local LLM Integration.
//!
//! This crate provides:
//! - Embedded Ollama server management
//! - OllamaLlm implementation of LlmProvider trait
//! - Model management (pull, list, remove)
//! - Health monitoring and recovery

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use blue_core::{
    CompletionOptions, CompletionResult, LlmBackendChoice, LlmError, LlmProvider, LocalLlmConfig,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Ollama version embedded with Blue
pub const OLLAMA_VERSION: &str = "0.5.4";

/// Default Ollama port
pub const DEFAULT_PORT: u16 = 11434;

/// Ollama API response for version
#[derive(Debug, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    #[serde(default)]
    pub gpu: Option<String>,
}

/// Ollama model info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    #[serde(default)]
    pub digest: String,
}

/// List of models response
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

/// Generate request
#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Debug, Serialize)]
struct GenerateOptions {
    num_predict: usize,
    temperature: f32,
    stop: Vec<String>,
}

/// Generate response
#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

/// Health status of Ollama
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy { version: String, gpu: Option<String> },
    Unhealthy { error: String },
    NotRunning,
}

/// Ollama operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaMode {
    /// Blue manages embedded Ollama
    Embedded,
    /// Using external Ollama instance
    External,
}

/// Embedded Ollama server manager
pub struct EmbeddedOllama {
    /// Running Ollama process
    process: Mutex<Option<Child>>,
    /// Port Ollama is running on
    port: u16,
    /// Directory for models
    models_dir: PathBuf,
    /// Backend configuration
    backend: LlmBackendChoice,
    /// Operation mode
    mode: OllamaMode,
    /// Is server ready
    ready: AtomicBool,
    /// HTTP client
    client: reqwest::blocking::Client,
}

impl EmbeddedOllama {
    /// Create a new embedded Ollama manager
    pub fn new(config: &LocalLlmConfig) -> Self {
        let models_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ollama")
            .join("models");

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min for model operations
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            process: Mutex::new(None),
            port: config.port,
            models_dir,
            backend: config.backend,
            mode: if config.use_external {
                OllamaMode::External
            } else {
                OllamaMode::Embedded
            },
            ready: AtomicBool::new(false),
            client,
        }
    }

    /// Get the base URL for Ollama API
    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Check if port is in use
    fn port_in_use(port: u16) -> bool {
        std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok()
    }

    /// Check if Ollama is already running on the port
    pub fn is_ollama_running(&self) -> bool {
        if let Ok(resp) = self.client.get(format!("{}/api/version", self.base_url())).send() {
            if let Ok(version) = resp.json::<VersionResponse>() {
                debug!("Found running Ollama: {}", version.version);
                return true;
            }
        }
        false
    }

    /// Find a free port starting from the given port
    #[allow(dead_code)]
    fn find_free_port(start: u16) -> Option<u16> {
        for port in start..start + 100 {
            if !Self::port_in_use(port) {
                return Some(port);
            }
        }
        None
    }

    /// Get path to bundled Ollama binary
    ///
    /// Resolution order:
    /// 1. BLUE_OLLAMA_PATH environment variable (for air-gapped builds)
    /// 2. Bundled binary next to executable
    /// 3. Common system locations (/usr/local/bin, /opt/homebrew/bin)
    /// 4. Fall back to PATH lookup
    pub fn bundled_binary_path() -> PathBuf {
        // First check BLUE_OLLAMA_PATH for air-gapped/custom builds
        if let Ok(custom_path) = std::env::var("BLUE_OLLAMA_PATH") {
            let path = PathBuf::from(&custom_path);
            if path.exists() {
                debug!("Using BLUE_OLLAMA_PATH: {}", custom_path);
                return path;
            }
        }

        // In development, look for it in the target directory
        // In production, it's bundled with the binary
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        #[cfg(target_os = "macos")]
        let binary_name = "ollama";
        #[cfg(target_os = "linux")]
        let binary_name = "ollama";
        #[cfg(target_os = "windows")]
        let binary_name = "ollama.exe";

        // Check common locations
        let candidates = vec![
            exe_dir.join(binary_name),
            exe_dir.join("bin").join(binary_name),
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/opt/homebrew/bin/ollama"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate;
            }
        }

        // Fall back to PATH
        PathBuf::from(binary_name)
    }

    /// Start the embedded Ollama server
    pub fn start(&self) -> Result<(), LlmError> {
        // Check if already running
        if self.ready.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Check if port is in use
        if Self::port_in_use(self.port) {
            if self.is_ollama_running() {
                // Use existing Ollama instance
                info!("Using existing Ollama on port {}", self.port);
                self.ready.store(true, Ordering::SeqCst);
                return Ok(());
            } else {
                // Something else is on the port
                return Err(LlmError::NotAvailable(format!(
                    "Port {} is in use by another service",
                    self.port
                )));
            }
        }

        // External mode - don't start, just check
        if self.mode == OllamaMode::External {
            return Err(LlmError::NotAvailable(
                "External Ollama not running".to_string(),
            ));
        }

        // Start embedded Ollama
        let binary = Self::bundled_binary_path();
        info!("Starting Ollama from {:?}", binary);

        let mut cmd = Command::new(&binary);
        cmd.arg("serve");
        cmd.env("OLLAMA_HOST", format!("127.0.0.1:{}", self.port));
        cmd.env("OLLAMA_MODELS", &self.models_dir);

        // Configure backend
        match self.backend {
            LlmBackendChoice::Cuda => {
                cmd.env("CUDA_VISIBLE_DEVICES", "0");
            }
            LlmBackendChoice::Mps => {
                cmd.env("CUDA_VISIBLE_DEVICES", "");
            }
            LlmBackendChoice::Cpu => {
                cmd.env("CUDA_VISIBLE_DEVICES", "");
                cmd.env("OLLAMA_NO_METAL", "1");
            }
            LlmBackendChoice::Auto => {
                // Let Ollama auto-detect
            }
        }

        // Suppress stdout/stderr in background
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            LlmError::NotAvailable(format!("Failed to start Ollama: {}", e))
        })?;

        *self.process.lock().unwrap() = Some(child);

        // Wait for server to be ready
        self.wait_for_ready()?;

        Ok(())
    }

    /// Wait for Ollama to be ready
    fn wait_for_ready(&self) -> Result<(), LlmError> {
        let max_attempts = 30; // 30 seconds
        for i in 0..max_attempts {
            if self.is_ollama_running() {
                info!("Ollama ready after {}s", i);
                self.ready.store(true, Ordering::SeqCst);
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }

        Err(LlmError::NotAvailable(
            "Ollama failed to start within 30 seconds".to_string(),
        ))
    }

    /// Stop the embedded Ollama server
    pub fn stop(&self) -> Result<(), LlmError> {
        self.ready.store(false, Ordering::SeqCst);

        let mut process = self.process.lock().unwrap();
        if let Some(mut child) = process.take() {
            // Try graceful shutdown first
            let _ = self.client.post(format!("{}/api/shutdown", self.base_url())).send();

            // Wait briefly for graceful shutdown
            std::thread::sleep(Duration::from_secs(2));

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait();

            info!("Ollama stopped");
        }

        Ok(())
    }

    /// Get health status
    pub fn health_check(&self) -> HealthStatus {
        match self.client.get(format!("{}/api/version", self.base_url())).send() {
            Ok(resp) => {
                match resp.json::<VersionResponse>() {
                    Ok(version) => HealthStatus::Healthy {
                        version: version.version,
                        gpu: version.gpu,
                    },
                    Err(e) => HealthStatus::Unhealthy {
                        error: e.to_string(),
                    },
                }
            }
            Err(_) => HealthStatus::NotRunning,
        }
    }

    /// List available models
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base_url()))
            .send()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        let models: ModelsResponse = resp
            .json()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        Ok(models.models)
    }

    /// Pull a model
    pub fn pull_model(&self, name: &str) -> Result<(), LlmError> {
        info!("Pulling model: {}", name);

        let resp = self
            .client
            .post(format!("{}/api/pull", self.base_url()))
            .json(&serde_json::json!({ "name": name, "stream": false }))
            .send()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(LlmError::RequestFailed(format!(
                "Pull failed: {}",
                resp.status()
            )));
        }

        info!("Model {} pulled successfully", name);
        Ok(())
    }

    /// Remove a model
    pub fn remove_model(&self, name: &str) -> Result<(), LlmError> {
        let resp = self
            .client
            .delete(format!("{}/api/delete", self.base_url()))
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(LlmError::RequestFailed(format!(
                "Delete failed: {}",
                resp.status()
            )));
        }

        Ok(())
    }

    /// Warm up a model (load into memory)
    pub fn warmup(&self, model: &str) -> Result<(), LlmError> {
        info!("Warming up model: {}", model);

        let resp = self
            .client
            .post(format!("{}/api/generate", self.base_url()))
            .json(&serde_json::json!({
                "model": model,
                "prompt": "Hi",
                "stream": false,
                "options": { "num_predict": 1 }
            }))
            .send()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(LlmError::RequestFailed(format!(
                "Warmup failed: {}",
                resp.status()
            )));
        }

        info!("Model {} warmed up", model);
        Ok(())
    }

    /// Generate completion
    pub fn generate(
        &self,
        model: &str,
        prompt: &str,
        options: &CompletionOptions,
    ) -> Result<CompletionResult, LlmError> {
        let request = GenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            options: GenerateOptions {
                num_predict: options.max_tokens,
                temperature: options.temperature,
                stop: options.stop_sequences.clone(),
            },
        };

        let resp = self
            .client
            .post(format!("{}/api/generate", self.base_url()))
            .json(&request)
            .send()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(LlmError::RequestFailed(format!(
                "Generate failed: {} - {}",
                status, body
            )));
        }

        let response: GenerateResponse = resp
            .json()
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        Ok(CompletionResult {
            text: response.response,
            prompt_tokens: response.prompt_eval_count,
            completion_tokens: response.eval_count,
            provider: "ollama".to_string(),
        })
    }

    /// Check if ready
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

impl Drop for EmbeddedOllama {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Ollama LLM provider
pub struct OllamaLlm {
    ollama: EmbeddedOllama,
    model: String,
}

impl OllamaLlm {
    /// Create a new Ollama LLM provider
    pub fn new(config: &LocalLlmConfig) -> Self {
        Self {
            ollama: EmbeddedOllama::new(config),
            model: config.model.clone(),
        }
    }

    /// Start the Ollama server
    pub fn start(&self) -> Result<(), LlmError> {
        self.ollama.start()
    }

    /// Stop the Ollama server
    pub fn stop(&self) -> Result<(), LlmError> {
        self.ollama.stop()
    }

    /// Get the embedded Ollama manager
    pub fn ollama(&self) -> &EmbeddedOllama {
        &self.ollama
    }
}

impl LlmProvider for OllamaLlm {
    fn complete(&self, prompt: &str, options: &CompletionOptions) -> Result<CompletionResult, LlmError> {
        if !self.ollama.is_ready() {
            return Err(LlmError::NotAvailable("Ollama not started".to_string()));
        }

        self.ollama.generate(&self.model, prompt, options)
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn is_ready(&self) -> bool {
        self.ollama.is_ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url() {
        let config = LocalLlmConfig::default();
        let ollama = EmbeddedOllama::new(&config);
        assert_eq!(ollama.base_url(), "http://127.0.0.1:11434");
    }

    #[test]
    fn test_base_url_custom_port() {
        let config = LocalLlmConfig {
            port: 12345,
            ..Default::default()
        };
        let ollama = EmbeddedOllama::new(&config);
        assert_eq!(ollama.base_url(), "http://127.0.0.1:12345");
    }

    #[test]
    fn test_health_status_not_running() {
        let config = LocalLlmConfig {
            port: 19999, // Unlikely to be in use
            ..Default::default()
        };
        let ollama = EmbeddedOllama::new(&config);
        matches!(ollama.health_check(), HealthStatus::NotRunning);
    }

    #[test]
    fn test_ollama_mode_embedded() {
        let config = LocalLlmConfig {
            use_external: false,
            ..Default::default()
        };
        let ollama = EmbeddedOllama::new(&config);
        assert_eq!(ollama.mode, OllamaMode::Embedded);
    }

    #[test]
    fn test_ollama_mode_external() {
        let config = LocalLlmConfig {
            use_external: true,
            ..Default::default()
        };
        let ollama = EmbeddedOllama::new(&config);
        assert_eq!(ollama.mode, OllamaMode::External);
    }

    #[test]
    fn test_port_in_use_detection() {
        // Port 22 is usually in use (SSH) on most systems
        // But we can't rely on that, so just verify the function doesn't panic
        let _ = EmbeddedOllama::port_in_use(22);
        let _ = EmbeddedOllama::port_in_use(65535);
    }

    #[test]
    fn test_bundled_binary_path_returns_path() {
        // Should return some path (either found or fallback)
        let path = EmbeddedOllama::bundled_binary_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_is_ready_initially_false() {
        let config = LocalLlmConfig {
            port: 19998,
            ..Default::default()
        };
        let ollama = EmbeddedOllama::new(&config);
        assert!(!ollama.is_ready());
    }

    #[test]
    fn test_ollama_llm_name() {
        let config = LocalLlmConfig::default();
        let llm = OllamaLlm::new(&config);
        assert_eq!(llm.name(), "ollama");
    }

    #[test]
    fn test_ollama_llm_not_ready_without_start() {
        let config = LocalLlmConfig {
            port: 19997,
            ..Default::default()
        };
        let llm = OllamaLlm::new(&config);
        assert!(!llm.is_ready());
    }

    #[test]
    fn test_complete_fails_when_not_ready() {
        let config = LocalLlmConfig {
            port: 19996,
            ..Default::default()
        };
        let llm = OllamaLlm::new(&config);
        let options = CompletionOptions::default();
        let result = llm.complete("test prompt", &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_options_serialization() {
        let options = GenerateOptions {
            num_predict: 100,
            temperature: 0.5,
            stop: vec!["stop1".to_string()],
        };
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"num_predict\":100"));
        assert!(json.contains("\"temperature\":0.5"));
    }

    #[test]
    fn test_model_info_clone() {
        let info = ModelInfo {
            name: "test-model".to_string(),
            size: 1024,
            modified_at: "2024-01-01".to_string(),
            digest: "abc123".to_string(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert_eq!(cloned.size, info.size);
    }
}
