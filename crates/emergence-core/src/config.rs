//! Configuration loading and typed config structures for the Emergence simulation.
//!
//! The canonical configuration lives in `emergence-config.yaml` at the project
//! root. This module defines strongly-typed structs that mirror the YAML
//! structure, and provides a loader that reads and validates the file.
//!
//! See `world-engine.md` section 13 for the full configuration reference.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// Errors that can occur when loading configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Failed to read the configuration file from disk.
    #[error("failed to read config file: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },

    /// Failed to parse YAML content.
    #[error("failed to parse config YAML: {source}")]
    Yaml {
        /// The underlying YAML parse error.
        source: serde_yml::Error,
    },
}

impl From<serde_yml::Error> for ConfigError {
    fn from(source: serde_yml::Error) -> Self {
        Self::Yaml { source }
    }
}

/// Top-level simulation configuration.
///
/// Mirrors the structure of `emergence-config.yaml`. All fields have
/// sensible defaults matching the values in the design documents.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SimulationConfig {
    /// World-level settings (name, seed, timing, starting era).
    #[serde(default)]
    pub world: WorldConfig,

    /// Time and season settings.
    #[serde(default)]
    pub time: TimeConfig,

    /// Population parameters.
    #[serde(default)]
    pub population: PopulationConfig,

    /// Economy parameters.
    #[serde(default)]
    pub economy: EconomyConfig,

    /// Environment toggles.
    #[serde(default)]
    pub environment: EnvironmentConfig,

    /// Discovery and learning parameters.
    #[serde(default)]
    pub discovery: DiscoveryConfig,

    /// Infrastructure connection strings.
    #[serde(default)]
    pub infrastructure: InfrastructureConfig,

    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,

    /// LLM backend configuration.
    #[serde(default)]
    pub llm: LlmConfig,

    /// Simulation boundary parameters.
    #[serde(default)]
    pub simulation: SimulationBoundsConfig,

    /// Operator control configuration.
    #[serde(default)]
    pub operator: OperatorConfig,
}

impl SimulationConfig {
    /// Load configuration from a YAML file at the given path.
    ///
    /// Environment variables override YAML values for infrastructure URLs:
    /// - `NATS_URL` overrides `infrastructure.nats_url`
    /// - `DATABASE_URL` overrides `infrastructure.postgres_url`
    /// - `DRAGONFLY_URL` overrides `infrastructure.dragonfly_url`
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Io`] if the file cannot be read, or
    /// [`ConfigError::Yaml`] if the content is not valid YAML.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let mut config: Self = serde_yml::from_str(&contents)?;
        config.infrastructure.apply_env_overrides();
        Ok(config)
    }

    /// Parse configuration from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Yaml`] if the string is not valid YAML.
    pub fn parse(yaml: &str) -> Result<Self, ConfigError> {
        let mut config: Self = serde_yml::from_str(yaml)?;
        config.infrastructure.apply_env_overrides();
        Ok(config)
    }
}


/// World-level configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WorldConfig {
    /// Human-readable simulation name.
    #[serde(default = "default_world_name")]
    pub name: String,

    /// Random seed for reproducibility.
    #[serde(default = "default_seed")]
    pub seed: u64,

    /// Real-time milliseconds per tick.
    #[serde(default = "default_tick_interval_ms")]
    pub tick_interval_ms: u64,

    /// Milliseconds agents have to respond before forfeiting.
    #[serde(default = "default_agent_decision_timeout_ms")]
    pub agent_decision_timeout_ms: u64,

    /// Starting civilizational era name.
    #[serde(default = "default_starting_era")]
    pub starting_era: String,

    /// Initial knowledge level for seed agents (0-3).
    #[serde(default = "default_knowledge_level")]
    pub knowledge_level: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: default_world_name(),
            seed: default_seed(),
            tick_interval_ms: default_tick_interval_ms(),
            agent_decision_timeout_ms: default_agent_decision_timeout_ms(),
            starting_era: default_starting_era(),
            knowledge_level: default_knowledge_level(),
        }
    }
}

/// Time and season configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TimeConfig {
    /// Number of ticks in one season.
    #[serde(default = "default_ticks_per_season")]
    pub ticks_per_season: u64,

    /// Ordered list of season names forming the annual cycle.
    #[serde(default = "default_seasons")]
    pub seasons: Vec<String>,

    /// Whether day/night cycle is enabled.
    #[serde(default = "default_true")]
    pub day_night: bool,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            ticks_per_season: default_ticks_per_season(),
            seasons: default_seasons(),
            day_night: true,
        }
    }
}

/// Population configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PopulationConfig {
    /// Number of agents to spawn at simulation start.
    #[serde(default = "default_initial_agents")]
    pub initial_agents: u32,

    /// Maximum living agents before reproduction is blocked.
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,

    /// Base lifespan in ticks.
    #[serde(default = "default_agent_lifespan_ticks")]
    pub agent_lifespan_ticks: u64,

    /// Whether agents can reproduce.
    #[serde(default = "default_true")]
    pub reproduction_enabled: bool,

    /// Ticks before a child agent is fully mature.
    #[serde(default = "default_child_maturity_ticks")]
    pub child_maturity_ticks: u64,
}

impl Default for PopulationConfig {
    fn default() -> Self {
        Self {
            initial_agents: default_initial_agents(),
            max_agents: default_max_agents(),
            agent_lifespan_ticks: default_agent_lifespan_ticks(),
            reproduction_enabled: true,
            child_maturity_ticks: default_child_maturity_ticks(),
        }
    }
}

/// Economy configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EconomyConfig {
    /// Resources given to each agent at spawn.
    #[serde(default = "default_starting_wallet")]
    pub starting_wallet: BTreeMap<String, u32>,

    /// Maximum weight an agent can carry.
    #[serde(default = "default_carry_capacity")]
    pub carry_capacity: u32,

    /// Hunger increase per tick.
    #[serde(default = "default_hunger_rate")]
    pub hunger_rate: u32,

    /// Health damage per tick when hunger reaches 100.
    #[serde(default = "default_starvation_damage")]
    pub starvation_damage: u32,

    /// Base energy restored when resting.
    #[serde(default = "default_rest_recovery")]
    pub rest_recovery: u32,

    /// Health restored per tick when conditions are met.
    #[serde(default = "default_natural_heal_rate")]
    pub natural_heal_rate: u32,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            starting_wallet: default_starting_wallet(),
            carry_capacity: default_carry_capacity(),
            hunger_rate: default_hunger_rate(),
            starvation_damage: default_starvation_damage(),
            rest_recovery: default_rest_recovery(),
            natural_heal_rate: default_natural_heal_rate(),
        }
    }
}

/// Environment toggles.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EnvironmentConfig {
    /// Whether weather effects are applied.
    #[serde(default = "default_true")]
    pub weather_enabled: bool,

    /// Whether seasons rotate.
    #[serde(default = "default_true")]
    pub seasons_enabled: bool,

    /// Whether structures decay over time.
    #[serde(default = "default_true")]
    pub structure_decay_enabled: bool,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            weather_enabled: true,
            seasons_enabled: true,
            structure_decay_enabled: true,
        }
    }
}

/// Discovery and learning parameters.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DiscoveryConfig {
    /// Base probability per tick per agent for accidental discoveries.
    /// Stored as a string to avoid float comparison issues.
    #[serde(default = "default_accidental_discovery_chance")]
    pub accidental_discovery_chance: f64,

    /// Whether agents can learn by watching others.
    #[serde(default = "default_true")]
    pub observation_learning_enabled: bool,

    /// Base success rate for the teach action.
    #[serde(default = "default_teaching_success_base")]
    pub teaching_success_base: f64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            accidental_discovery_chance: default_accidental_discovery_chance(),
            observation_learning_enabled: true,
            teaching_success_base: default_teaching_success_base(),
        }
    }
}

/// Infrastructure connection strings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InfrastructureConfig {
    /// Dragonfly (Redis-compatible) URL.
    #[serde(default = "default_dragonfly_url")]
    pub dragonfly_url: String,

    /// `PostgreSQL` connection string.
    #[serde(default = "default_postgres_url")]
    pub postgres_url: String,

    /// NATS messaging URL.
    #[serde(default = "default_nats_url")]
    pub nats_url: String,

    /// Observer dashboard port.
    #[serde(default = "default_observer_port")]
    pub observer_port: u16,
}

impl InfrastructureConfig {
    /// Override infrastructure URLs with environment variables when set.
    ///
    /// This allows Docker Compose (or any deployment) to set connection
    /// strings via env vars without modifying the YAML config file.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("NATS_URL") {
            self.nats_url = val;
        }
        if let Ok(val) = std::env::var("DATABASE_URL") {
            self.postgres_url = val;
        }
        if let Ok(val) = std::env::var("DRAGONFLY_URL") {
            self.dragonfly_url = val;
        }
    }
}

impl Default for InfrastructureConfig {
    fn default() -> Self {
        Self {
            dragonfly_url: default_dragonfly_url(),
            postgres_url: default_postgres_url(),
            nats_url: default_nats_url(),
            observer_port: default_observer_port(),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Number of events to batch before flushing to `PostgreSQL`.
    #[serde(default = "default_event_store_batch_size")]
    pub event_store_batch_size: u32,

    /// Full world snapshot every N ticks.
    #[serde(default = "default_snapshot_interval_ticks")]
    pub snapshot_interval_ticks: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            event_store_batch_size: default_event_store_batch_size(),
            snapshot_interval_ticks: default_snapshot_interval_ticks(),
        }
    }
}

/// LLM backend configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LlmConfig {
    /// Default LLM backend name.
    #[serde(default = "default_llm_backend")]
    pub default_backend: String,

    /// Escalation backend for complex decisions.
    #[serde(default = "default_escalation_backend")]
    pub escalation_backend: String,

    /// Maximum retry attempts for LLM calls.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Request timeout in milliseconds (must be < agent decision timeout).
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            default_backend: default_llm_backend(),
            escalation_backend: default_escalation_backend(),
            max_retries: default_max_retries(),
            request_timeout_ms: default_request_timeout_ms(),
        }
    }
}

/// Simulation boundary configuration.
///
/// Controls when and how the simulation ends. A value of 0 for
/// either `max_ticks` or `max_real_time_seconds` means unlimited.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SimulationBoundsConfig {
    /// Maximum number of ticks before the simulation ends (0 = unlimited).
    #[serde(default)]
    pub max_ticks: u64,

    /// Maximum wall-clock seconds before the simulation ends (0 = unlimited).
    #[serde(default = "default_max_real_time_seconds")]
    pub max_real_time_seconds: u64,

    /// End condition type: `time_limit`, `extinction`, `era_reached`, `manual`.
    #[serde(default = "default_end_condition")]
    pub end_condition: String,

    /// Minimum number of living agents before auto-spawning kicks in.
    ///
    /// If the population drops below this value after a tick completes,
    /// the engine will automatically queue spawn requests to reach this
    /// floor. Set to 0 to disable auto-recovery.
    #[serde(default = "default_min_population")]
    pub min_population: u32,
}

impl Default for SimulationBoundsConfig {
    fn default() -> Self {
        Self {
            max_ticks: 0,
            max_real_time_seconds: default_max_real_time_seconds(),
            end_condition: default_end_condition(),
            min_population: default_min_population(),
        }
    }
}

/// Operator control configuration.
///
/// Settings for the operator REST API that controls the simulation
/// at runtime (pause, resume, speed, event injection, stop).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OperatorConfig {
    /// Whether the operator API is enabled.
    #[serde(default = "default_true")]
    pub api_enabled: bool,

    /// Bearer token for authenticating operator requests (empty = no auth).
    #[serde(default)]
    pub api_auth_token: String,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            api_enabled: true,
            api_auth_token: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Experiment Framework Configuration (Phase 5.2)
// ---------------------------------------------------------------------------

/// Experiment configuration for A/B testing and reproducible experiments.
///
/// An experiment encapsulates a bounded simulation run with explicit
/// parameter overrides. Two experiments with different personality
/// distributions but the same seed and world config can be compared
/// post-hoc.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct ExperimentConfig {
    /// Unique experiment identifier (generated at creation time).
    #[serde(default = "default_experiment_id")]
    pub experiment_id: String,

    /// Human-readable experiment name.
    #[serde(default)]
    pub name: String,

    /// Longer description of the experiment's purpose or hypothesis.
    #[serde(default)]
    pub description: String,

    /// Number of agents to spawn at experiment start.
    ///
    /// Overrides `population.initial_agents` when set.
    #[serde(default)]
    pub agent_count: Option<u32>,

    /// Personality distribution mode: `"random"`, `"cooperative"`,
    /// `"aggressive"`, `"balanced"`, `"custom"`.
    ///
    /// Determines how personality vectors are generated for seed agents.
    #[serde(default = "default_personality_distribution")]
    pub personality_distribution: String,

    /// World seed override for reproducibility.
    ///
    /// When set, overrides `world.seed` from the base config.
    #[serde(default)]
    pub world_seed: Option<u64>,

    /// Maximum ticks for this experiment (0 = use base config).
    #[serde(default)]
    pub max_ticks: u64,

    /// Arbitrary parameter overrides as key-value pairs.
    ///
    /// Keys follow dot-notation paths into the config tree,
    /// e.g. `"economy.hunger_rate"` = `"3"`.
    #[serde(default)]
    pub parameter_overrides: BTreeMap<String, String>,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            experiment_id: default_experiment_id(),
            name: String::new(),
            description: String::new(),
            agent_count: None,
            personality_distribution: default_personality_distribution(),
            world_seed: None,
            max_ticks: 0,
            parameter_overrides: BTreeMap::new(),
        }
    }
}

impl ExperimentConfig {
    /// Create a new experiment config with a fresh ID and the given name.
    pub fn new(name: &str) -> Self {
        Self {
            experiment_id: default_experiment_id(),
            name: name.to_owned(),
            ..Self::default()
        }
    }

    /// Load experiment config from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Io`] if the file cannot be read, or
    /// [`ConfigError::Yaml`] if the content is not valid YAML.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = serde_yml::from_str(&contents)?;
        Ok(config)
    }

    /// Parse experiment config from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Yaml`] if the string is not valid YAML.
    pub fn parse(yaml: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yml::from_str(yaml)?;
        Ok(config)
    }
}

fn default_experiment_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn default_personality_distribution() -> String {
    "random".to_owned()
}

// ---------------------------------------------------------------------------
// Default value functions (serde default requires named functions)
// ---------------------------------------------------------------------------

fn default_world_name() -> String {
    "Experiment Alpha".to_owned()
}

const fn default_seed() -> u64 {
    42
}

const fn default_tick_interval_ms() -> u64 {
    10_000
}

const fn default_agent_decision_timeout_ms() -> u64 {
    8_000
}

fn default_starting_era() -> String {
    "primitive".to_owned()
}

const fn default_knowledge_level() -> u32 {
    1
}

const fn default_ticks_per_season() -> u64 {
    90
}

fn default_seasons() -> Vec<String> {
    vec![
        "spring".to_owned(),
        "summer".to_owned(),
        "autumn".to_owned(),
        "winter".to_owned(),
    ]
}

const fn default_initial_agents() -> u32 {
    10
}

const fn default_max_agents() -> u32 {
    200
}

const fn default_agent_lifespan_ticks() -> u64 {
    2500
}

const fn default_child_maturity_ticks() -> u64 {
    200
}

fn default_starting_wallet() -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();
    m.insert("food_berry".to_owned(), 10);
    m.insert("water".to_owned(), 5);
    m.insert("wood".to_owned(), 3);
    m
}

const fn default_carry_capacity() -> u32 {
    50
}

const fn default_hunger_rate() -> u32 {
    5
}

const fn default_starvation_damage() -> u32 {
    10
}

const fn default_rest_recovery() -> u32 {
    30
}

const fn default_natural_heal_rate() -> u32 {
    2
}

const fn default_accidental_discovery_chance() -> f64 {
    0.02
}

const fn default_teaching_success_base() -> f64 {
    0.80
}

fn default_dragonfly_url() -> String {
    "redis://localhost:6379".to_owned()
}

fn default_postgres_url() -> String {
    "postgresql://emergence:emergence@localhost:5432/emergence".to_owned()
}

fn default_nats_url() -> String {
    "nats://localhost:4222".to_owned()
}

const fn default_observer_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_owned()
}

const fn default_event_store_batch_size() -> u32 {
    100
}

const fn default_snapshot_interval_ticks() -> u64 {
    100
}

fn default_llm_backend() -> String {
    "openai".to_owned()
}

fn default_escalation_backend() -> String {
    "anthropic".to_owned()
}

const fn default_max_retries() -> u32 {
    2
}

const fn default_request_timeout_ms() -> u64 {
    7000
}

const fn default_max_real_time_seconds() -> u64 {
    86_400
}

fn default_end_condition() -> String {
    "time_limit".to_owned()
}

const fn default_min_population() -> u32 {
    2
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = SimulationConfig::default();
        assert_eq!(config.world.seed, 42);
        assert_eq!(config.time.ticks_per_season, 90);
        assert_eq!(config.time.seasons.len(), 4);
        assert_eq!(config.population.initial_agents, 10);
        assert_eq!(config.economy.hunger_rate, 5);
    }

    #[test]
    fn parse_full_yaml() {
        let yaml = r#"
world:
  name: "Test World"
  seed: 123
  tick_interval_ms: 5000
  agent_decision_timeout_ms: 4000
  starting_era: "primitive"
  knowledge_level: 2

time:
  ticks_per_season: 45
  seasons:
    - spring
    - summer
    - autumn
    - winter
  day_night: true

population:
  initial_agents: 5
  max_agents: 100
  agent_lifespan_ticks: 1000
  reproduction_enabled: false
  child_maturity_ticks: 100

economy:
  starting_wallet:
    food_berry: 20
    water: 10
  carry_capacity: 30
  hunger_rate: 3
  starvation_damage: 5
  rest_recovery: 20
  natural_heal_rate: 1

environment:
  weather_enabled: false
  seasons_enabled: true
  structure_decay_enabled: false

discovery:
  accidental_discovery_chance: 0.05
  observation_learning_enabled: false
  teaching_success_base: 0.90

infrastructure:
  dragonfly_url: "redis://testhost:6379"
  postgres_url: "postgresql://test:test@testhost:5432/testdb"
  nats_url: "nats://testhost:4222"
  observer_port: 9090

logging:
  level: "debug"
  event_store_batch_size: 50
  snapshot_interval_ticks: 50

llm:
  default_backend: "ollama"
  escalation_backend: "openai"
  max_retries: 3
  request_timeout_ms: 5000
"#;

        let config = SimulationConfig::parse(yaml);
        assert!(config.is_ok());
        let config = config.ok().unwrap_or_else(SimulationConfig::default);

        assert_eq!(config.world.name, "Test World");
        assert_eq!(config.world.seed, 123);
        assert_eq!(config.time.ticks_per_season, 45);
        assert_eq!(config.population.initial_agents, 5);
        assert!(!config.population.reproduction_enabled);
        assert!(!config.environment.weather_enabled);
        assert_eq!(config.infrastructure.observer_port, 9090);
        assert_eq!(config.llm.default_backend, "ollama");
    }

    #[test]
    fn parse_minimal_yaml() {
        let yaml = "world:\n  seed: 7\n";
        let config = SimulationConfig::parse(yaml);
        assert!(config.is_ok());
        let config = config.ok().unwrap_or_else(SimulationConfig::default);

        // Seed is overridden
        assert_eq!(config.world.seed, 7);
        // Everything else uses defaults
        assert_eq!(config.time.ticks_per_season, 90);
        assert_eq!(config.population.initial_agents, 10);
    }

    #[test]
    fn parse_empty_yaml() {
        let yaml = "";
        let config = SimulationConfig::parse(yaml);
        assert!(config.is_ok());
    }

    #[test]
    fn load_project_config_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("emergence-config.yaml");
        if path.exists() {
            let config = SimulationConfig::from_file(&path);
            assert!(config.is_ok(), "Failed to load project config: {config:?}");
        }
    }
}
