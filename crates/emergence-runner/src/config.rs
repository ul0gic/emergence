//! Configuration types for the agent runner.
//!
//! All configuration is loaded from environment variables. The runner needs to
//! know how to reach NATS and which LLM backends to use (with their URLs,
//! API keys, and model names).

use std::time::Duration;

use crate::error::RunnerError;

/// Complete runner configuration loaded from the environment.
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// NATS server URL (e.g. `nats://localhost:4222`).
    pub nats_url: String,
    /// Primary LLM backend configuration.
    pub primary_backend: LlmBackendConfig,
    /// Secondary (fallback) LLM backend configuration.
    pub secondary_backend: Option<LlmBackendConfig>,
    /// Maximum time allowed for an agent to decide (LLM call + parsing).
    pub decision_timeout: Duration,
    /// Maximum number of concurrent LLM calls.
    pub max_concurrent_calls: usize,
    /// Path to the templates directory.
    pub templates_dir: String,
    /// Whether to route LLM calls based on tick complexity scoring.
    ///
    /// When enabled, high-complexity decisions are sent to the escalation
    /// backend first, while low/medium complexity decisions use the
    /// primary (cheap/fast) backend. When disabled, all decisions use
    /// the primary backend with the escalation backend as fallback only.
    pub complexity_routing_enabled: bool,
    /// When true, bypass the LLM for obvious survival decisions
    /// (eat when starving, rest when exhausted, etc.).
    ///
    /// Corresponds to `llm.routine_action_bypass` in `emergence-config.yaml`.
    pub routine_action_bypass: bool,
    /// When true, sleeping or low-energy agents at night skip the
    /// LLM call entirely and auto-rest.
    ///
    /// Corresponds to `llm.night_cycle_skip` in `emergence-config.yaml`.
    pub night_cycle_skip: bool,
}

/// Configuration for a single LLM backend.
#[derive(Debug, Clone)]
pub struct LlmBackendConfig {
    /// The backend type (openai, anthropic, ollama).
    pub backend_type: BackendType,
    /// Base API URL (e.g. `https://api.openai.com/v1`).
    pub api_url: String,
    /// API key for authentication.
    pub api_key: String,
    /// Model identifier (e.g. `gpt-5-nano-2025-08-07`).
    pub model: String,
}

/// Supported LLM backend types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendType {
    /// `OpenAI`-compatible API (works with `OpenAI`, `DeepSeek`, Ollama).
    OpenAi,
    /// Anthropic Messages API (different request format).
    Anthropic,
}

impl RunnerConfig {
    /// Load configuration from environment variables.
    ///
    /// Required variables:
    /// - `NATS_URL` -- NATS server connection string
    /// - `LLM_DEFAULT_BACKEND` -- primary backend type
    /// - `LLM_DEFAULT_API_URL` -- primary API base URL
    /// - `LLM_DEFAULT_API_KEY` -- primary API key
    /// - `LLM_DEFAULT_MODEL` -- primary model name
    ///
    /// Optional variables:
    /// - `LLM_ESCALATION_BACKEND` -- secondary backend type
    /// - `LLM_ESCALATION_API_URL` -- secondary API base URL
    /// - `LLM_ESCALATION_API_KEY` -- secondary API key
    /// - `LLM_ESCALATION_MODEL` -- secondary model name
    /// - `DECISION_TIMEOUT_MS` -- decision deadline in milliseconds (default 7000)
    /// - `MAX_CONCURRENT_CALLS` -- max parallel LLM calls (default 20)
    /// - `TEMPLATES_DIR` -- path to prompt templates (default `templates`)
    /// - `COMPLEXITY_ROUTING_ENABLED` -- enable complexity-based backend routing (default `true`)
    /// - `ROUTINE_ACTION_BYPASS` -- bypass LLM for obvious survival actions (default `true`)
    /// - `NIGHT_CYCLE_SKIP` -- skip LLM for sleeping agents at night (default `true`)
    pub fn from_env() -> Result<Self, RunnerError> {
        let nats_url = env_var("NATS_URL")?;
        let primary_backend = load_backend_config("LLM_DEFAULT")?;

        let secondary_backend = load_backend_config("LLM_ESCALATION").ok();

        let decision_timeout_ms: u64 = std::env::var("DECISION_TIMEOUT_MS")
            .unwrap_or_else(|_| "7000".to_owned())
            .parse()
            .map_err(|e| RunnerError::Config(format!("invalid DECISION_TIMEOUT_MS: {e}")))?;

        let max_concurrent_calls: usize = std::env::var("MAX_CONCURRENT_CALLS")
            .unwrap_or_else(|_| "20".to_owned())
            .parse()
            .map_err(|e| RunnerError::Config(format!("invalid MAX_CONCURRENT_CALLS: {e}")))?;

        let templates_dir =
            std::env::var("TEMPLATES_DIR").unwrap_or_else(|_| "templates".to_owned());

        let complexity_routing_enabled: bool = std::env::var("COMPLEXITY_ROUTING_ENABLED")
            .unwrap_or_else(|_| "true".to_owned())
            .parse()
            .map_err(|e| {
                RunnerError::Config(format!("invalid COMPLEXITY_ROUTING_ENABLED: {e}"))
            })?;

        let routine_action_bypass: bool = std::env::var("ROUTINE_ACTION_BYPASS")
            .unwrap_or_else(|_| "true".to_owned())
            .parse()
            .map_err(|e| {
                RunnerError::Config(format!("invalid ROUTINE_ACTION_BYPASS: {e}"))
            })?;

        let night_cycle_skip: bool = std::env::var("NIGHT_CYCLE_SKIP")
            .unwrap_or_else(|_| "true".to_owned())
            .parse()
            .map_err(|e| {
                RunnerError::Config(format!("invalid NIGHT_CYCLE_SKIP: {e}"))
            })?;

        Ok(Self {
            nats_url,
            primary_backend,
            secondary_backend,
            decision_timeout: Duration::from_millis(decision_timeout_ms),
            max_concurrent_calls,
            templates_dir,
            complexity_routing_enabled,
            routine_action_bypass,
            night_cycle_skip,
        })
    }
}

/// Read a required environment variable.
fn env_var(name: &str) -> Result<String, RunnerError> {
    std::env::var(name)
        .map_err(|e| RunnerError::Config(format!("missing required env var {name}: {e}")))
}

/// Load an LLM backend config from a set of prefixed environment variables.
fn load_backend_config(prefix: &str) -> Result<LlmBackendConfig, RunnerError> {
    let backend_str = env_var(&format!("{prefix}_BACKEND"))?;
    let api_url = env_var(&format!("{prefix}_API_URL"))?;
    let api_key = env_var(&format!("{prefix}_API_KEY"))?;
    let model = env_var(&format!("{prefix}_MODEL"))?;

    let backend_type = match backend_str.to_lowercase().as_str() {
        "openai" | "deepseek" | "ollama" => BackendType::OpenAi,
        "anthropic" | "claude" => BackendType::Anthropic,
        other => {
            return Err(RunnerError::Config(format!(
                "unknown backend type: {other}"
            )))
        }
    };

    Ok(LlmBackendConfig {
        backend_type,
        api_url,
        api_key,
        model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_type_parsing() {
        // Direct construction tests since from_env requires real env vars
        let config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://api.openai.com/v1".to_owned(),
            api_key: "test-key".to_owned(),
            model: "gpt-5-nano".to_owned(),
        };
        assert_eq!(config.backend_type, BackendType::OpenAi);

        let anthropic = LlmBackendConfig {
            backend_type: BackendType::Anthropic,
            api_url: "https://api.anthropic.com/v1".to_owned(),
            api_key: "test-key".to_owned(),
            model: "claude-haiku-4-5".to_owned(),
        };
        assert_eq!(anthropic.backend_type, BackendType::Anthropic);
    }

    #[test]
    fn runner_config_defaults() {
        // Verify default values used in from_env fallbacks
        let timeout_default: u64 = "7000".parse().unwrap_or(0);
        assert_eq!(timeout_default, 7000);

        let concurrency_default: usize = "20".parse().unwrap_or(0);
        assert_eq!(concurrency_default, 20);
    }
}
