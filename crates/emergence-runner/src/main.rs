//! Agent runtime entry point for the Emergence simulation.
//!
//! The runner orchestrates LLM-powered agent decisions. It receives perception
//! payloads from the World Engine via NATS, assembles prompts, calls LLM backends
//! via HTTP, parses structured actions from responses, and submits them back to
//! the World Engine for resolution.
//!
//! # Architecture
//!
//! ```text
//! NATS (perception) --> Prompt Engine --> LLM Backend --> Parser --> NATS (action)
//! ```
//!
//! Every agent gets one decision per tick. If the LLM fails or times out,
//! the runner submits `NoAction` so the agent never misses a tick.

mod complexity;
mod config;
mod error;
mod llm;
mod nats;
mod parse;
mod prompt;
mod rule_engine;
mod runner;

use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::RunnerConfig;
use crate::llm::create_backend;
use crate::nats::NatsClient;
use crate::prompt::PromptEngine;
use crate::runner::AgentRunner;

/// Application entry point.
///
/// Initializes logging, loads configuration from environment variables,
/// connects to NATS, sets up LLM backends and prompt templates, then runs
/// the agent decision loop indefinitely.
///
/// # Errors
///
/// Returns an error if initialization or the main event loop fails.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    info!("emergence-runner starting");

    // Load configuration from environment
    let config = RunnerConfig::from_env()?;
    info!(
        nats_url = config.nats_url,
        templates_dir = config.templates_dir,
        decision_timeout_ms = config.decision_timeout.as_millis(),
        max_concurrent_calls = config.max_concurrent_calls,
        "configuration loaded"
    );

    // Connect to NATS
    let nats = NatsClient::connect(&config.nats_url).await?;

    // Load prompt templates
    let prompt_engine = PromptEngine::new(&config.templates_dir)?;
    info!(
        templates_dir = config.templates_dir,
        "prompt templates loaded"
    );

    // Create LLM backends
    let primary = create_backend(&config.primary_backend);
    info!(
        backend = primary.name(),
        model = config.primary_backend.model,
        "primary LLM backend configured"
    );

    let escalation = config.secondary_backend.as_ref().map(|cfg| {
        let backend = create_backend(cfg);
        info!(
            backend = backend.name(),
            model = cfg.model,
            "escalation LLM backend configured"
        );
        backend
    });

    // Build and run the agent runner
    info!(
        routine_action_bypass = config.routine_action_bypass,
        night_cycle_skip = config.night_cycle_skip,
        complexity_routing_enabled = config.complexity_routing_enabled,
        "decision optimization configuration"
    );

    let agent_runner = AgentRunner::new(
        nats,
        prompt_engine,
        primary,
        escalation,
        config.decision_timeout,
        config.routine_action_bypass,
        config.night_cycle_skip,
        config.complexity_routing_enabled,
    );

    info!("agent runner initialized, entering decision loop");
    agent_runner.run().await?;

    Ok(())
}
