//! LLM backend abstraction and implementations.
//!
//! Defines an enum-based dispatch for LLM backends, avoiding the
//! dyn-compatibility issues with async trait methods. Concrete
//! implementations exist for OpenAI-compatible APIs and the Anthropic
//! Messages API. All backends communicate over HTTP via `reqwest`.
//!
//! The runner does not care which model is behind the API -- it sends a
//! prompt and expects a text response containing JSON.

use crate::config::{BackendType, LlmBackendConfig};
use crate::error::RunnerError;
use crate::prompt::RenderedPrompt;

// ---------------------------------------------------------------------------
// Unified backend enum (dyn-compatible alternative to async trait)
// ---------------------------------------------------------------------------

/// An LLM backend that can process a prompt and return a response.
///
/// Uses enum dispatch instead of trait objects because async methods
/// are not dyn-compatible in Rust.
pub enum LlmBackend {
    /// OpenAI-compatible chat completions API.
    OpenAi(OpenAiBackend),
    /// Anthropic Messages API.
    Anthropic(AnthropicBackend),
}

impl LlmBackend {
    /// Send a prompt to the LLM and return the response text.
    ///
    /// Dispatches to the concrete backend implementation.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError::LlmBackend`] if the HTTP call fails or the
    /// response cannot be extracted.
    pub async fn complete(&self, prompt: &RenderedPrompt) -> Result<String, RunnerError> {
        match self {
            Self::OpenAi(backend) => backend.complete(prompt).await,
            Self::Anthropic(backend) => backend.complete(prompt).await,
        }
    }

    /// Human-readable name for logging.
    pub const fn name(&self) -> &str {
        match self {
            Self::OpenAi(_) => "openai-compatible",
            Self::Anthropic(_) => "anthropic",
        }
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible backend
// ---------------------------------------------------------------------------

/// Backend for OpenAI-compatible chat completions APIs.
///
/// Works with `OpenAI`, `DeepSeek`, and Ollama endpoints.
/// Sends requests to `{api_url}/chat/completions`.
pub struct OpenAiBackend {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl OpenAiBackend {
    /// Create a new `OpenAI`-compatible backend.
    pub fn new(config: &LlmBackendConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        }
    }

    /// Send a prompt and return the response text.
    async fn complete(&self, prompt: &RenderedPrompt) -> Result<String, RunnerError> {
        let url = format!("{}/chat/completions", self.api_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": prompt.system},
                {"role": "user", "content": prompt.user}
            ],
            "temperature": 0.7,
            "max_tokens": 512,
            "response_format": {"type": "json_object"}
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RunnerError::LlmBackend(format!("OpenAI request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "unable to read error body".to_owned());
            return Err(RunnerError::LlmBackend(format!(
                "OpenAI returned {status}: {error_body}"
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RunnerError::LlmBackend(format!("OpenAI response parse failed: {e}")))?;

        extract_openai_content(&json)
    }
}

/// Extract the text content from an `OpenAI` chat completions response.
fn extract_openai_content(json: &serde_json::Value) -> Result<String, RunnerError> {
    json.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            RunnerError::LlmBackend(
                "OpenAI response missing choices[0].message.content".to_owned(),
            )
        })
}

// ---------------------------------------------------------------------------
// Anthropic Messages API backend
// ---------------------------------------------------------------------------

/// Backend for the Anthropic Messages API.
///
/// Anthropic uses a different request format from `OpenAI`:
/// - Uses `x-api-key` header instead of `Authorization: Bearer`
/// - Messages array does not include system (system is a top-level field)
/// - Response structure differs: `content[0].text`
pub struct AnthropicBackend {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl AnthropicBackend {
    /// Create a new Anthropic Messages API backend.
    pub fn new(config: &LlmBackendConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        }
    }

    /// Send a prompt and return the response text.
    async fn complete(&self, prompt: &RenderedPrompt) -> Result<String, RunnerError> {
        let url = format!("{}/messages", self.api_url);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 512,
            "system": prompt.system,
            "messages": [
                {"role": "user", "content": prompt.user}
            ]
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RunnerError::LlmBackend(format!("Anthropic request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "unable to read error body".to_owned());
            return Err(RunnerError::LlmBackend(format!(
                "Anthropic returned {status}: {error_body}"
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| {
                RunnerError::LlmBackend(format!("Anthropic response parse failed: {e}"))
            })?;

        extract_anthropic_content(&json)
    }
}

/// Extract the text content from an Anthropic Messages API response.
fn extract_anthropic_content(json: &serde_json::Value) -> Result<String, RunnerError> {
    json.get("content")
        .and_then(|c| c.get(0))
        .and_then(|b| b.get("text"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            RunnerError::LlmBackend("Anthropic response missing content[0].text".to_owned())
        })
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create an LLM backend from configuration.
///
/// Dispatches to [`OpenAiBackend`] or [`AnthropicBackend`] based on the
/// configured [`BackendType`].
pub fn create_backend(config: &LlmBackendConfig) -> LlmBackend {
    match config.backend_type {
        BackendType::OpenAi => LlmBackend::OpenAi(OpenAiBackend::new(config)),
        BackendType::Anthropic => LlmBackend::Anthropic(AnthropicBackend::new(config)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_openai_content_valid() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "{\"action_type\": \"Gather\", \"parameters\": {\"resource\": \"Wood\"}}"
                }
            }]
        });
        let result = extract_openai_content(&json);
        assert!(result.is_ok());
        assert!(result.unwrap_or_default().contains("Gather"));
    }

    #[test]
    fn extract_openai_content_missing_choices() {
        let json = serde_json::json!({"error": "rate_limit"});
        let result = extract_openai_content(&json);
        assert!(result.is_err());
    }

    #[test]
    fn extract_anthropic_content_valid() {
        let json = serde_json::json!({
            "content": [{
                "type": "text",
                "text": "{\"action_type\": \"Rest\"}"
            }]
        });
        let result = extract_anthropic_content(&json);
        assert!(result.is_ok());
        assert!(result.unwrap_or_default().contains("Rest"));
    }

    #[test]
    fn extract_anthropic_content_missing() {
        let json = serde_json::json!({"content": []});
        let result = extract_anthropic_content(&json);
        assert!(result.is_err());
    }

    #[test]
    fn create_backend_dispatches_correctly() {
        let openai_config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://api.openai.com/v1".to_owned(),
            api_key: "test".to_owned(),
            model: "test-model".to_owned(),
        };
        let backend = create_backend(&openai_config);
        assert_eq!(backend.name(), "openai-compatible");

        let anthropic_config = LlmBackendConfig {
            backend_type: BackendType::Anthropic,
            api_url: "https://api.anthropic.com/v1".to_owned(),
            api_key: "test".to_owned(),
            model: "test-model".to_owned(),
        };
        let backend = create_backend(&anthropic_config);
        assert_eq!(backend.name(), "anthropic");
    }
}
