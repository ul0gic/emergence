//! Configuration types for the agent runner.
//!
//! All configuration is loaded from environment variables. The runner needs to
//! know how to reach NATS and which LLM backends to use (with their URLs,
//! API keys, and model names).

use std::time::Duration;

use rust_decimal::Decimal;

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
    /// `OpenRouter`-specific headers (referer, app title).
    ///
    /// Populated when either backend is configured as `openrouter`.
    /// Passed through to `OpenAiBackend` so it can add the required headers.
    pub openrouter_config: OpenRouterConfig,
    /// Partition ID for multi-runner agent partitioning (0-indexed).
    ///
    /// When running multiple runner instances, each instance is assigned
    /// a unique `partition_id` in the range `[0, total_partitions)`. An
    /// agent is owned by partition `hash(agent_id) % total_partitions`.
    /// A value of `0` with `total_partitions = 1` means "handle all agents"
    /// (single-runner mode).
    pub partition_id: u32,
    /// Total number of runner partitions.
    ///
    /// Must be >= 1. When set to 1, the single runner handles all agents.
    /// When > 1, each runner instance handles the subset of agents where
    /// `hash(agent_id) % total_partitions == partition_id`.
    pub total_partitions: u32,
}

/// Configuration for a single LLM backend.
#[derive(Debug, Clone)]
pub struct LlmBackendConfig {
    /// The backend type (openai, anthropic, ollama, openrouter).
    pub backend_type: BackendType,
    /// Base API URL (e.g. `https://api.openai.com/v1`).
    pub api_url: String,
    /// API key for authentication.
    pub api_key: String,
    /// Model identifier (e.g. `gpt-5-nano-2025-08-07`).
    pub model: String,
    /// Cost per million input tokens for cost tracking (optional).
    ///
    /// When `None`, cost tracking records the call but estimates zero cost.
    pub cost_per_m_input: Option<Decimal>,
    /// Cost per million output tokens for cost tracking (optional).
    ///
    /// When `None`, cost tracking records the call but estimates zero cost.
    pub cost_per_m_output: Option<Decimal>,
}

/// `OpenRouter`-specific configuration loaded from environment variables.
///
/// `OpenRouter` requires `HTTP-Referer` and `X-Title` headers on every request
/// for ranking and attribution. These are loaded once at startup and threaded
/// into the `OpenAI`-compatible backend when the backend string is `openrouter`.
#[derive(Debug, Clone, Default)]
pub struct OpenRouterConfig {
    /// Value for the `HTTP-Referer` header (e.g. `http://localhost:8080`).
    pub http_referer: Option<String>,
    /// Value for the `X-Title` header (e.g. `Emergence`).
    pub app_title: Option<String>,
}

/// Supported LLM backend types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendType {
    /// `OpenAI`-compatible API (works with `OpenAI`, `DeepSeek`, Ollama, `OpenRouter`).
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
    /// - `PARTITION_ID` -- this runner's partition index (default `0`)
    /// - `TOTAL_PARTITIONS` -- total runner instances (default `1`)
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

        let openrouter_config = load_openrouter_config();

        let partition_id: u32 = std::env::var("PARTITION_ID")
            .unwrap_or_else(|_| "0".to_owned())
            .parse()
            .map_err(|e| RunnerError::Config(format!("invalid PARTITION_ID: {e}")))?;

        let total_partitions: u32 = std::env::var("TOTAL_PARTITIONS")
            .unwrap_or_else(|_| "1".to_owned())
            .parse()
            .map_err(|e| RunnerError::Config(format!("invalid TOTAL_PARTITIONS: {e}")))?;

        if total_partitions == 0 {
            return Err(RunnerError::Config(
                "TOTAL_PARTITIONS must be >= 1".to_owned(),
            ));
        }

        if partition_id >= total_partitions {
            return Err(RunnerError::Config(format!(
                "PARTITION_ID ({partition_id}) must be < TOTAL_PARTITIONS ({total_partitions})"
            )));
        }

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
            openrouter_config,
            partition_id,
            total_partitions,
        })
    }
}

/// Read a required environment variable.
fn env_var(name: &str) -> Result<String, RunnerError> {
    std::env::var(name)
        .map_err(|e| RunnerError::Config(format!("missing required env var {name}: {e}")))
}

/// Load an LLM backend config from a set of prefixed environment variables.
///
/// Reads `{prefix}_BACKEND`, `{prefix}_API_URL`, `{prefix}_API_KEY`,
/// `{prefix}_MODEL`, and optionally `{prefix}_COST_PER_M_INPUT` /
/// `{prefix}_COST_PER_M_OUTPUT` for cost tracking.
fn load_backend_config(prefix: &str) -> Result<LlmBackendConfig, RunnerError> {
    let backend_str = env_var(&format!("{prefix}_BACKEND"))?;
    let api_url = env_var(&format!("{prefix}_API_URL"))?;
    let api_key = env_var(&format!("{prefix}_API_KEY"))?;
    let model = env_var(&format!("{prefix}_MODEL"))?;

    let backend_type = parse_backend_type(&backend_str)?;

    let cost_per_m_input = parse_optional_decimal(&format!("{prefix}_COST_PER_M_INPUT"))?;
    let cost_per_m_output = parse_optional_decimal(&format!("{prefix}_COST_PER_M_OUTPUT"))?;

    Ok(LlmBackendConfig {
        backend_type,
        api_url,
        api_key,
        model,
        cost_per_m_input,
        cost_per_m_output,
    })
}

/// Parse a backend type string into a [`BackendType`].
///
/// Recognized strings (case-insensitive):
/// - `openai`, `deepseek`, `ollama`, `openrouter` -> [`BackendType::OpenAi`]
/// - `anthropic`, `claude` -> [`BackendType::Anthropic`]
fn parse_backend_type(s: &str) -> Result<BackendType, RunnerError> {
    match s.to_lowercase().as_str() {
        "openai" | "deepseek" | "ollama" | "openrouter" => Ok(BackendType::OpenAi),
        "anthropic" | "claude" => Ok(BackendType::Anthropic),
        other => Err(RunnerError::Config(format!(
            "unknown backend type: {other}"
        ))),
    }
}

/// Parse an optional `Decimal` from an environment variable.
///
/// Returns `Ok(None)` if the variable is not set or empty. Returns an error
/// if the variable is set but cannot be parsed as a `Decimal`.
fn parse_optional_decimal(name: &str) -> Result<Option<Decimal>, RunnerError> {
    match std::env::var(name) {
        Ok(val) if !val.is_empty() => {
            let d: Decimal = val
                .parse()
                .map_err(|e| RunnerError::Config(format!("invalid {name}: {e}")))?;
            Ok(Some(d))
        }
        _ => Ok(None),
    }
}

/// Load `OpenRouter`-specific configuration from environment variables.
///
/// Both fields are optional -- if not set, the headers are simply omitted
/// from requests.
fn load_openrouter_config() -> OpenRouterConfig {
    OpenRouterConfig {
        http_referer: std::env::var("OPENROUTER_HTTP_REFERER").ok().filter(|s| !s.is_empty()),
        app_title: std::env::var("OPENROUTER_APP_TITLE").ok().filter(|s| !s.is_empty()),
    }
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
            cost_per_m_input: None,
            cost_per_m_output: None,
        };
        assert_eq!(config.backend_type, BackendType::OpenAi);

        let anthropic = LlmBackendConfig {
            backend_type: BackendType::Anthropic,
            api_url: "https://api.anthropic.com/v1".to_owned(),
            api_key: "test-key".to_owned(),
            model: "claude-haiku-4-5".to_owned(),
            cost_per_m_input: None,
            cost_per_m_output: None,
        };
        assert_eq!(anthropic.backend_type, BackendType::Anthropic);
    }

    #[test]
    fn openrouter_maps_to_openai_backend_type() {
        // OpenRouter uses the OpenAI-compatible API, so parse_backend_type
        // must map "openrouter" to BackendType::OpenAi.
        let result = parse_backend_type("openrouter");
        assert!(result.is_ok());
        let bt = match result {
            Ok(b) => b,
            Err(_) => return,
        };
        assert_eq!(bt, BackendType::OpenAi);
    }

    #[test]
    fn openrouter_config_with_cost_fields() {
        let config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://openrouter.ai/api/v1".to_owned(),
            api_key: "test-key".to_owned(),
            model: "deepseek/deepseek-chat-v3-0324".to_owned(),
            cost_per_m_input: Some(Decimal::new(30, 2)),
            cost_per_m_output: Some(Decimal::new(88, 2)),
        };
        assert_eq!(config.backend_type, BackendType::OpenAi);
        assert!(config.cost_per_m_input.is_some());
        assert!(config.cost_per_m_output.is_some());
    }

    #[test]
    fn parse_backend_type_all_recognized_strings() {
        // Verify every recognized backend string maps to the correct type.
        for (name, expected) in [
            ("openai", BackendType::OpenAi),
            ("deepseek", BackendType::OpenAi),
            ("ollama", BackendType::OpenAi),
            ("openrouter", BackendType::OpenAi),
            ("OPENROUTER", BackendType::OpenAi),
            ("OpenRouter", BackendType::OpenAi),
            ("anthropic", BackendType::Anthropic),
            ("claude", BackendType::Anthropic),
            ("ANTHROPIC", BackendType::Anthropic),
        ] {
            let result = parse_backend_type(name);
            assert!(result.is_ok(), "backend string '{name}' should be recognized");
            let bt = match result {
                Ok(b) => b,
                Err(_) => continue,
            };
            assert_eq!(bt, expected, "'{name}' should map to {expected:?}");
        }
    }

    #[test]
    fn parse_backend_type_unknown_returns_error() {
        let result = parse_backend_type("unknown");
        assert!(result.is_err());
        let result = parse_backend_type("");
        assert!(result.is_err());
    }

    #[test]
    fn decimal_parsing_valid_values() {
        // Test the Decimal parsing logic used by parse_optional_decimal
        let d: Result<Decimal, _> = "3.50".parse();
        assert!(d.is_ok());
        assert_eq!(d.unwrap_or_default(), Decimal::new(350, 2));

        let d: Result<Decimal, _> = "0.30".parse();
        assert!(d.is_ok());
        assert_eq!(d.unwrap_or_default(), Decimal::new(30, 2));

        let d: Result<Decimal, _> = "15.00".parse();
        assert!(d.is_ok());
        assert_eq!(d.unwrap_or_default(), Decimal::new(1500, 2));
    }

    #[test]
    fn decimal_parsing_invalid_values() {
        let d: Result<Decimal, _> = "not-a-number".parse();
        assert!(d.is_err());
    }

    #[test]
    fn openrouter_config_default_is_empty() {
        let cfg = OpenRouterConfig::default();
        assert!(cfg.http_referer.is_none());
        assert!(cfg.app_title.is_none());
    }

    #[test]
    fn openrouter_config_direct_construction() {
        let cfg = OpenRouterConfig {
            http_referer: Some("http://localhost:8080".to_owned()),
            app_title: Some("Emergence".to_owned()),
        };
        assert_eq!(cfg.http_referer.as_deref(), Some("http://localhost:8080"));
        assert_eq!(cfg.app_title.as_deref(), Some("Emergence"));
    }

    #[test]
    fn runner_config_defaults() {
        // Verify default values used in from_env fallbacks
        let timeout_default: u64 = "7000".parse().unwrap_or(0);
        assert_eq!(timeout_default, 7000);

        let concurrency_default: usize = "20".parse().unwrap_or(0);
        assert_eq!(concurrency_default, 20);
    }

    #[test]
    fn cost_fields_optional_in_backend_config() {
        // When cost env vars are not set, the fields should be None.
        let config = LlmBackendConfig {
            backend_type: BackendType::OpenAi,
            api_url: "https://api.openai.com/v1".to_owned(),
            api_key: "key".to_owned(),
            model: "model".to_owned(),
            cost_per_m_input: None,
            cost_per_m_output: None,
        };
        assert!(config.cost_per_m_input.is_none());
        assert!(config.cost_per_m_output.is_none());

        // When set, they carry the configured values.
        let config_with_cost = LlmBackendConfig {
            cost_per_m_input: Some(Decimal::new(300, 2)),
            cost_per_m_output: Some(Decimal::new(1500, 2)),
            ..config
        };
        assert_eq!(
            config_with_cost.cost_per_m_input.unwrap_or_default(),
            Decimal::new(300, 2)
        );
        assert_eq!(
            config_with_cost.cost_per_m_output.unwrap_or_default(),
            Decimal::new(1500, 2)
        );
    }
}
