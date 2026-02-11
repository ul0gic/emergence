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
mod containment;
mod cost;
mod error;
mod llm;
mod nats;
mod parse;
mod prompt;
mod rule_engine;
mod runner;

use std::sync::Arc;

use rust_decimal::Decimal;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::RunnerConfig;
use crate::cost::CostTracker;
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

    // Build the shared cost tracker from configured rates.
    let cost_tracker = {
        let primary_input = config
            .primary_backend
            .cost_per_m_input
            .unwrap_or(Decimal::ZERO);
        let primary_output = config
            .primary_backend
            .cost_per_m_output
            .unwrap_or(Decimal::ZERO);
        let (esc_input, esc_output) = config
            .secondary_backend
            .as_ref()
            .map_or((Decimal::ZERO, Decimal::ZERO), |c| {
                (
                    c.cost_per_m_input.unwrap_or(Decimal::ZERO),
                    c.cost_per_m_output.unwrap_or(Decimal::ZERO),
                )
            });

        Arc::new(CostTracker::new(
            primary_input,
            primary_output,
            esc_input,
            esc_output,
        ))
    };

    info!("cost tracker initialized");

    // Create LLM backends
    let primary = create_backend(
        &config.primary_backend,
        &config.openrouter_config,
        Some(Arc::clone(&cost_tracker)),
        "primary",
    );
    info!(
        backend = primary.name(),
        model = config.primary_backend.model,
        "primary LLM backend configured"
    );

    let escalation = config.secondary_backend.as_ref().map(|cfg| {
        let backend = create_backend(
            cfg,
            &config.openrouter_config,
            Some(Arc::clone(&cost_tracker)),
            "escalation",
        );
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
    )
    .with_partitioning(config.partition_id, config.total_partitions);

    info!(
        partition_id = config.partition_id,
        total_partitions = config.total_partitions,
        "agent runner initialized, entering decision loop"
    );
    agent_runner.run().await?;

    Ok(())
}
