//! LLM backend abstraction and implementations.
//!
//! Defines an enum-based dispatch for LLM backends, avoiding the
//! dyn-compatibility issues with async trait methods. Concrete
//! implementations exist for `OpenAI`-compatible APIs and the Anthropic
//! Messages API. All backends communicate over HTTP via `reqwest`.
//!
//! The runner does not care which model is behind the API -- it sends a
//! prompt and expects a text response containing JSON.
//!
//! When a `CostTracker` is attached, successful responses are inspected
//! for the `usage` field and token counts are recorded for cost estimation.

use std::sync::Arc;

use tracing::debug;

use crate::config::{BackendType, LlmBackendConfig, OpenRouterConfig};
use crate::cost::CostTracker;
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
    /// `OpenAI`-compatible chat completions API.
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

/// Token usage information extracted from an LLM API response.
///
/// Not all providers return this field; when absent, token counts are zero.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    /// Number of tokens in the prompt (input).
    pub prompt_tokens: u64,
    /// Number of tokens in the completion (output).
    pub completion_tokens: u64,
}

// ---------------------------------------------------------------------------
// OpenAI-compatible backend
// ---------------------------------------------------------------------------

/// Backend for `OpenAI`-compatible chat completions APIs.
///
/// Works with `OpenAI`, `DeepSeek`, Ollama, and `OpenRouter` endpoints.
/// Sends requests to `{api_url}/chat/completions`.
///
/// When `OpenRouter` headers are provided (via [`OpenRouterConfig`]), the
/// required `HTTP-Referer` and `X-Title` headers are included on every
/// request.
pub struct OpenAiBackend {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
    /// `OpenRouter`-specific headers (optional, empty when not using `OpenRouter`).
    openrouter_config: OpenRouterConfig,
    /// Shared cost tracker for recording token usage.
    cost_tracker: Option<Arc<CostTracker>>,
    /// Human-readable backend label for cost tracking entries.
    backend_label: String,
}

impl OpenAiBackend {
    /// Create a new `OpenAI`-compatible backend.
    pub fn new(
        config: &LlmBackendConfig,
        openrouter_config: &OpenRouterConfig,
        cost_tracker: Option<Arc<CostTracker>>,
        backend_label: String,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            openrouter_config: openrouter_config.clone(),
            cost_tracker,
            backend_label,
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

        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // OpenRouter requires these headers for ranking and attribution.
        if let Some(referer) = &self.openrouter_config.http_referer {
            request = request.header("HTTP-Referer", referer.as_str());
        }
        if let Some(title) = &self.openrouter_config.app_title {
            request = request.header("X-Title", title.as_str());
        }

        let response = request
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

        // Record token usage for cost tracking (best-effort).
        if let Some(tracker) = &self.cost_tracker {
            let usage = extract_openai_usage(&json);
            tracker.record_call(
                &self.backend_label,
                usage.prompt_tokens,
                usage.completion_tokens,
            );
            debug!(
                backend = self.backend_label,
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                "token usage recorded"
            );
        }

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

/// Extract token usage from an `OpenAI`-compatible response.
///
/// The `usage` field is optional -- some providers omit it. When absent,
/// returns a zeroed [`TokenUsage`].
fn extract_openai_usage(json: &serde_json::Value) -> TokenUsage {
    let Some(usage) = json.get("usage") else {
        return TokenUsage::default();
    };
    TokenUsage {
        prompt_tokens: usage
            .get("prompt_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        completion_tokens: usage
            .get("completion_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }
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
/// - Usage is returned as `usage.input_tokens` / `usage.output_tokens`
pub struct AnthropicBackend {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
    /// Shared cost tracker for recording token usage.
    cost_tracker: Option<Arc<CostTracker>>,
    /// Human-readable backend label for cost tracking entries.
    backend_label: String,
}

impl AnthropicBackend {
    /// Create a new Anthropic Messages API backend.
    pub fn new(
        config: &LlmBackendConfig,
        cost_tracker: Option<Arc<CostTracker>>,
        backend_label: String,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            cost_tracker,
            backend_label,
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

        // Record token usage for cost tracking (best-effort).
        if let Some(tracker) = &self.cost_tracker {
            let usage = extract_anthropic_usage(&json);
            tracker.record_call(
                &self.backend_label,
                usage.prompt_tokens,
                usage.completion_tokens,
            );
            debug!(
                backend = self.backend_label,
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                "token usage recorded"
            );
        }

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

/// Extract token usage from an Anthropic Messages API response.
///
/// Anthropic uses `usage.input_tokens` / `usage.output_tokens` rather
/// than `prompt_tokens` / `completion_tokens`. When absent, returns a
/// zeroed [`TokenUsage`].
fn extract_anthropic_usage(json: &serde_json::Value) -> TokenUsage {
    let Some(usage) = json.get("usage") else {
        return TokenUsage::default();
    };
    TokenUsage {
        prompt_tokens: usage
            .get("input_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        completion_tokens: usage
            .get("output_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create an LLM backend from configuration.
///
/// Dispatches to [`OpenAiBackend`] or [`AnthropicBackend`] based on the
/// configured [`BackendType`]. Injects `OpenRouter` headers and the shared
/// cost tracker into the concrete backend.
pub fn create_backend(
    config: &LlmBackendConfig,
    openrouter_config: &OpenRouterConfig,
    cost_tracker: Option<Arc<CostTracker>>,
    backend_label: &str,
) -> LlmBackend {
    match config.backend_type {
        BackendType::OpenAi => LlmBackend::OpenAi(OpenAiBackend::new(
            config,
            openrouter_config,
            cost_tracker,
            backend_label.to_owned(),
        )),
        BackendType::Anthropic => LlmBackend::Anthropic(AnthropicBackend::new(
            config,
            cost_tracker,
            backend_label.to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

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
    fn extract_openai_usage_present() {
        let json = serde_json::json!({
            "choices": [{"message": {"content": "test"}}],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 25,
                "total_tokens": 125
            }
        });
        let usage = extract_openai_usage(&json);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 25);
    }

    #[test]
    fn extract_openai_usage_missing() {
        let json = serde_json::json!({"choices": [{"message": {"content": "test"}}]});
        let usage = extract_openai_usage(&json);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
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
    fn extract_anthropic_usage_present() {
        let json = serde_json::json!({
            "content": [{"type": "text", "text": "test"}],
            "usage": {
                "input_tokens": 200,
                "output_tokens": 50
            }
        });
        let usage = extract_anthropic_usage(&json);
        assert_eq!(usage.prompt_tokens, 200);
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn extract_anthropic_usage_missing() {
        let json = serde_json::json!({"content": [{"type": "text", "text": "test"}]});
        let usage = extract_anthropic_usage(&json);
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
    }

    #[test]
    fn create_backend_dispatches_correctly() {
        let or_config = OpenRouterConfig::default();

        let openai_config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://api.openai.com/v1".to_owned(),
            api_key: "test".to_owned(),
            model: "test-model".to_owned(),
            cost_per_m_input: None,
            cost_per_m_output: None,
        };
        let backend = create_backend(&openai_config, &or_config, None, "primary");
        assert_eq!(backend.name(), "openai-compatible");

        let anthropic_config = LlmBackendConfig {
            backend_type: BackendType::Anthropic,
            api_url: "https://api.anthropic.com/v1".to_owned(),
            api_key: "test".to_owned(),
            model: "test-model".to_owned(),
            cost_per_m_input: None,
            cost_per_m_output: None,
        };
        let backend = create_backend(&anthropic_config, &or_config, None, "escalation");
        assert_eq!(backend.name(), "anthropic");
    }

    #[test]
    fn create_backend_openrouter_with_cost_tracker() {
        let or_config = OpenRouterConfig {
            http_referer: Some("http://localhost:8080".to_owned()),
            app_title: Some("Emergence".to_owned()),
        };
        let tracker = Arc::new(CostTracker::new(
            Decimal::new(30, 2),
            Decimal::new(88, 2),
            Decimal::new(300, 2),
            Decimal::new(1500, 2),
        ));

        let config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://openrouter.ai/api/v1".to_owned(),
            api_key: "test".to_owned(),
            model: "deepseek/deepseek-chat-v3-0324".to_owned(),
            cost_per_m_input: Some(Decimal::new(30, 2)),
            cost_per_m_output: Some(Decimal::new(88, 2)),
        };
        let backend = create_backend(
            &config,
            &or_config,
            Some(Arc::clone(&tracker)),
            "primary",
        );
        assert_eq!(backend.name(), "openai-compatible");
    }
}
