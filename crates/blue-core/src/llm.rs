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
#[derive(Default)]
pub enum LlmBackendChoice {
    /// Auto-detect best backend (CUDA > MPS > CPU)
    #[default]
    Auto,
    /// Force CUDA (NVIDIA GPU)
    Cuda,
    /// Force Metal/MPS (Apple Silicon)
    Mps,
    /// Force CPU only
    Cpu,
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

/// Keyword-based fallback "LLM"
///
/// Uses simple keyword matching when no real LLM is available.
/// This provides basic functionality for tasks like ADR relevance matching.
pub struct KeywordLlm;

impl KeywordLlm {
    pub fn new() -> Self {
        Self
    }

    /// Extract keywords from text (simple word tokenization)
    fn extract_keywords(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(String::from)
            .collect()
    }

    /// Calculate keyword overlap score between two texts
    pub fn keyword_score(text1: &str, text2: &str) -> f64 {
        let words1: std::collections::HashSet<_> = Self::extract_keywords(text1).into_iter().collect();
        let words2: std::collections::HashSet<_> = Self::extract_keywords(text2).into_iter().collect();

        if words1.is_empty() || words2.is_empty() {
            return 0.0;
        }

        let intersection = words1.intersection(&words2).count();
        let union = words1.union(&words2).count();

        intersection as f64 / union as f64
    }
}

impl Default for KeywordLlm {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for KeywordLlm {
    fn complete(&self, prompt: &str, _options: &CompletionOptions) -> Result<CompletionResult, LlmError> {
        // KeywordLlm doesn't generate text - it's for scoring/matching only
        // Return the prompt keywords as a simple response
        let keywords = Self::extract_keywords(prompt);
        Ok(CompletionResult {
            text: keywords.join(", "),
            prompt_tokens: None,
            completion_tokens: None,
            provider: "keywords".to_string(),
        })
    }

    fn name(&self) -> &str {
        "keywords"
    }

    fn is_ready(&self) -> bool {
        true // Always ready - no external dependencies
    }
}

/// LLM Manager with graceful degradation
///
/// Tries providers in order: Local (Ollama) → API → Keywords
/// Falls back automatically when a provider is unavailable.
pub struct LlmManager {
    providers: Vec<Box<dyn LlmProvider>>,
    config: LlmConfig,
}

impl LlmManager {
    /// Create a new LLM manager with the given configuration
    pub fn new(config: LlmConfig) -> Self {
        Self {
            providers: Vec::new(),
            config,
        }
    }

    /// Add a provider to the fallback chain
    pub fn add_provider(&mut self, provider: Box<dyn LlmProvider>) {
        self.providers.push(provider);
    }

    /// Add the keyword fallback (always available)
    pub fn with_keyword_fallback(mut self) -> Self {
        self.providers.push(Box::new(KeywordLlm::new()));
        self
    }

    /// Get the first ready provider
    pub fn active_provider(&self) -> Option<&dyn LlmProvider> {
        self.providers.iter()
            .find(|p| p.is_ready())
            .map(|p| p.as_ref())
    }

    /// Get the active provider name
    pub fn active_provider_name(&self) -> &str {
        self.active_provider()
            .map(|p| p.name())
            .unwrap_or("none")
    }

    /// Check if any provider is available
    pub fn is_available(&self) -> bool {
        self.providers.iter().any(|p| p.is_ready())
    }

    /// Complete a prompt using the first available provider
    pub fn complete(&self, prompt: &str, options: &CompletionOptions) -> Result<CompletionResult, LlmError> {
        // Respect provider preference
        match self.config.provider {
            LlmProviderChoice::None => {
                return Err(LlmError::NotAvailable("LLM disabled by configuration".to_string()));
            }
            LlmProviderChoice::Local => {
                // Only try local providers
                for provider in &self.providers {
                    if provider.name() == "ollama" && provider.is_ready() {
                        return provider.complete(prompt, options);
                    }
                }
                return Err(LlmError::NotAvailable("Local LLM not available".to_string()));
            }
            LlmProviderChoice::Api => {
                // Only try API providers
                for provider in &self.providers {
                    if (provider.name() == "anthropic" || provider.name() == "openai") && provider.is_ready() {
                        return provider.complete(prompt, options);
                    }
                }
                return Err(LlmError::NotAvailable("API LLM not available".to_string()));
            }
            LlmProviderChoice::Auto => {
                // Try all providers in order
            }
        }

        // Auto mode: try each provider in order
        let mut last_error = None;
        for provider in &self.providers {
            if provider.is_ready() {
                match provider.complete(prompt, options) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        last_error = Some(e);
                        continue;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| LlmError::NotAvailable("No LLM providers available".to_string())))
    }

    /// Get status of all providers
    pub fn status(&self) -> Vec<ProviderStatus> {
        self.providers.iter()
            .map(|p| ProviderStatus {
                name: p.name().to_string(),
                ready: p.is_ready(),
            })
            .collect()
    }
}

/// Status of a provider
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub ready: bool,
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

    #[test]
    fn test_keyword_llm_extract_keywords() {
        let keywords = KeywordLlm::extract_keywords("Hello, World! This is a TEST.");
        assert!(keywords.contains(&"hello".to_string()));
        assert!(keywords.contains(&"world".to_string()));
        assert!(keywords.contains(&"this".to_string()));
        assert!(keywords.contains(&"test".to_string()));
        // Short words filtered out
        assert!(!keywords.contains(&"is".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
    }

    #[test]
    fn test_keyword_llm_score() {
        // Identical texts should have score 1.0
        let score = KeywordLlm::keyword_score("hello world", "hello world");
        assert!((score - 1.0).abs() < 0.01);

        // Completely different texts should have score 0.0
        let score = KeywordLlm::keyword_score("hello world", "foo bar baz");
        assert!(score < 0.01);

        // Partial overlap
        let score = KeywordLlm::keyword_score("hello world test", "hello world foo");
        assert!(score > 0.3 && score < 0.8);
    }

    #[test]
    fn test_keyword_llm_always_ready() {
        let llm = KeywordLlm::new();
        assert!(llm.is_ready());
        assert_eq!(llm.name(), "keywords");
    }

    #[test]
    fn test_llm_manager_with_keyword_fallback() {
        let config = LlmConfig::default();
        let manager = LlmManager::new(config).with_keyword_fallback();

        assert!(manager.is_available());
        assert_eq!(manager.active_provider_name(), "keywords");
    }

    #[test]
    fn test_llm_manager_complete_with_fallback() {
        let config = LlmConfig::default();
        let manager = LlmManager::new(config).with_keyword_fallback();

        let result = manager.complete("test prompt here", &CompletionOptions::default());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider, "keywords");
    }

    #[test]
    fn test_llm_manager_provider_order() {
        let config = LlmConfig::default();
        let mut manager = LlmManager::new(config);

        // Add mock first, then keywords
        manager.add_provider(Box::new(MockLlm::constant("mock response")));
        manager.add_provider(Box::new(KeywordLlm::new()));

        // Mock should be used first since it's ready
        assert_eq!(manager.active_provider_name(), "mock");

        let result = manager.complete("test", &CompletionOptions::default()).unwrap();
        assert_eq!(result.provider, "mock");
        assert_eq!(result.text, "mock response");
    }

    #[test]
    fn test_llm_manager_status() {
        let config = LlmConfig::default();
        let mut manager = LlmManager::new(config);
        manager.add_provider(Box::new(MockLlm::constant("test")));
        manager.add_provider(Box::new(KeywordLlm::new()));

        let status = manager.status();
        assert_eq!(status.len(), 2);
        assert!(status.iter().all(|s| s.ready));
    }

    #[test]
    fn test_llm_manager_disabled() {
        let config = LlmConfig {
            provider: LlmProviderChoice::None,
            ..Default::default()
        };
        let manager = LlmManager::new(config).with_keyword_fallback();

        let result = manager.complete("test", &CompletionOptions::default());
        assert!(result.is_err());
    }
}
