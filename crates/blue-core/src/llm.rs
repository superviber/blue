//! LLM Provider abstraction
//!
//! Implements RFC 0005: Local LLM Integration.
//! Provides a unified interface for LLM access, supporting both
//! local (Ollama) and API (Anthropic/OpenAI) backends.

use std::fmt;

/// Options for LLM completion
#[derive(Debug, Clone)]
pub struct CompletionOptions {
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Temperature (0.0-1.0)
    pub temperature: f32,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.7,
            stop_sequences: Vec::new(),
        }
    }
}

/// Result of an LLM completion
#[derive(Debug, Clone)]
pub struct CompletionResult {
    /// Generated text
    pub text: String,
    /// Tokens used in prompt
    pub prompt_tokens: Option<usize>,
    /// Tokens generated
    pub completion_tokens: Option<usize>,
    /// Provider that generated this
    pub provider: String,
}

/// LLM provider errors
#[derive(Debug)]
pub enum LlmError {
    /// Provider not available
    NotAvailable(String),
    /// Request failed
    RequestFailed(String),
    /// Model not found
    ModelNotFound(String),
    /// Insufficient memory for model
    InsufficientMemory {
        model: String,
        required: u64,
        available: u64,
    },
    /// Binary verification failed
    BinaryTampered {
        expected: String,
        actual: String,
    },
    /// Other error
    Other(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::NotAvailable(msg) => write!(f, "LLM not available: {}", msg),
            LlmError::RequestFailed(msg) => write!(f, "LLM request failed: {}", msg),
            LlmError::ModelNotFound(model) => write!(f, "Model not found: {}", model),
            LlmError::InsufficientMemory { model, required, available } => {
                write!(f, "Insufficient memory for {}: need {} bytes, have {}", model, required, available)
            }
            LlmError::BinaryTampered { expected, actual } => {
                write!(f, "Binary verification failed: expected {}, got {}", expected, actual)
            }
            LlmError::Other(msg) => write!(f, "LLM error: {}", msg),
        }
    }
}

impl std::error::Error for LlmError {}

/// LLM provider trait
///
/// Implementations:
/// - OllamaLlm: Local Ollama server
/// - ApiLlm: External API (Anthropic/OpenAI)
/// - MockLlm: Testing
pub trait LlmProvider: Send + Sync {
    /// Complete a prompt
    fn complete(
        &self,
        prompt: &str,
        options: &CompletionOptions,
    ) -> Result<CompletionResult, LlmError>;

    /// Provider name
    fn name(&self) -> &str;

    /// Check if provider is ready
    fn is_ready(&self) -> bool;
}

/// LLM backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmBackendChoice {
    /// Auto-detect best backend (CUDA > MPS > CPU)
    Auto,
    /// Force CUDA (NVIDIA GPU)
    Cuda,
    /// Force Metal/MPS (Apple Silicon)
    Mps,
    /// Force CPU only
    Cpu,
}

impl Default for LlmBackendChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// LLM configuration
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Provider preference: auto, local, api, none
    pub provider: LlmProviderChoice,
    /// Local Ollama configuration
    pub local: LocalLlmConfig,
    /// API configuration
    pub api: ApiLlmConfig,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProviderChoice::Auto,
            local: LocalLlmConfig::default(),
            api: ApiLlmConfig::default(),
        }
    }
}

/// Provider preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProviderChoice {
    /// Auto: local if available, else API, else keywords
    #[default]
    Auto,
    /// Only use local, fail if unavailable
    Local,
    /// Only use API, fail if unavailable
    Api,
    /// Disable LLM features entirely
    None,
}

/// Local (Ollama) configuration
#[derive(Debug, Clone)]
pub struct LocalLlmConfig {
    /// Model name (e.g., "qwen2.5:7b")
    pub model: String,
    /// Backend choice
    pub backend: LlmBackendChoice,
    /// Context window size
    pub context_size: usize,
    /// CPU threads (for CPU backend)
    pub threads: usize,
    /// Ollama port
    pub port: u16,
    /// Use external Ollama instead of embedded
    pub use_external: bool,
}

impl Default for LocalLlmConfig {
    fn default() -> Self {
        Self {
            model: "qwen2.5:7b".to_string(),
            backend: LlmBackendChoice::Auto,
            context_size: 8192,
            threads: 8,
            port: 11434,
            use_external: false,
        }
    }
}

/// API configuration
#[derive(Debug, Clone)]
pub struct ApiLlmConfig {
    /// API provider: anthropic, openai
    pub provider: String,
    /// Model name
    pub model: String,
    /// Environment variable for API key
    pub api_key_env: String,
}

impl Default for ApiLlmConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
        }
    }
}

/// Mock LLM for testing
pub struct MockLlm {
    responses: Vec<String>,
    current: std::sync::atomic::AtomicUsize,
}

impl MockLlm {
    /// Create a new mock LLM with predefined responses
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            current: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create a mock that always returns the same response
    pub fn constant(response: &str) -> Self {
        Self::new(vec![response.to_string()])
    }
}

impl LlmProvider for MockLlm {
    fn complete(&self, _prompt: &str, _options: &CompletionOptions) -> Result<CompletionResult, LlmError> {
        let idx = self.current.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let response = self.responses.get(idx % self.responses.len())
            .cloned()
            .unwrap_or_default();

        Ok(CompletionResult {
            text: response,
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            provider: "mock".to_string(),
        })
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn is_ready(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_llm() {
        let llm = MockLlm::new(vec!["response1".to_string(), "response2".to_string()]);

        let result1 = llm.complete("test", &CompletionOptions::default()).unwrap();
        assert_eq!(result1.text, "response1");

        let result2 = llm.complete("test", &CompletionOptions::default()).unwrap();
        assert_eq!(result2.text, "response2");

        // Cycles back
        let result3 = llm.complete("test", &CompletionOptions::default()).unwrap();
        assert_eq!(result3.text, "response1");
    }

    #[test]
    fn test_completion_options_default() {
        let opts = CompletionOptions::default();
        assert_eq!(opts.max_tokens, 1024);
        assert!((opts.temperature - 0.7).abs() < f32::EPSILON);
    }
}
