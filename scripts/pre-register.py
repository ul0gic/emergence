#!/usr/bin/env python3
"""
Emergence -- Pre-Registration Tool

Generates baseline LLM predictions for a simulation run BEFORE the run begins.
This implements a pre-registration protocol: query the same LLM that powers the
agents with the experiment configuration and ask it to predict outcomes for each
research question. These predictions serve as a baseline to compare against
actual simulation results.

The purpose is to distinguish genuine emergent dynamics from training-data
recapitulation. If the simulation produces outcomes that closely match what
the LLM predicts without running the simulation, the dynamics may be
recapitulation rather than emergence. Divergences are the scientifically
interesting findings.

Usage:
    python3 scripts/pre-register.py --config emergence-config.yaml --agents 10 --ticks 5000
    python3 scripts/pre-register.py --config emergence-config.yaml --agents 50 --ticks 30000 --model deepseek/deepseek-chat-v3-0324
    python3 scripts/pre-register.py --config emergence-config.yaml --agents 10 --ticks 5000 --save-to-db

Requirements:
    pip install requests pyyaml

Environment:
    OPENROUTER_API_KEY  -- API key for OpenRouter (or set LLM_DEFAULT_API_KEY)
    DATABASE_URL        -- PostgreSQL connection string (only needed with --save-to-db)
"""

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:
    import requests
except ImportError:
    print("ERROR: 'requests' package is required. Install with: pip install requests", file=sys.stderr)
    sys.exit(1)

try:
    import yaml
except ImportError:
    print("ERROR: 'pyyaml' package is required. Install with: pip install pyyaml", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Research questions -- must match README.md
# ---------------------------------------------------------------------------

RESEARCH_QUESTIONS = [
    {
        "id": "social_coordination",
        "category": "Social Organization",
        "min_agents": 5,
        "question": "How do coordination patterns form and restructure? When agents need to solve collective problems (resource scarcity, defense), what organizational structures arise? How stable are they?",
    },
    {
        "id": "social_stratification",
        "category": "Social Organization",
        "min_agents": 5,
        "question": "How does social stratification develop? When agents differ in personality, skill, and accumulated resources, how does hierarchy form? Is it contested?",
    },
    {
        "id": "leadership",
        "category": "Social Organization",
        "min_agents": 5,
        "question": "How do agents handle leadership? Does authority centralize around individuals? Is it stable or contested? What triggers leadership transitions?",
    },
    {
        "id": "exchange_networks",
        "category": "Economic Dynamics",
        "min_agents": 5,
        "question": "How do exchange networks form and restructure? Starting from no trade infrastructure, how do agents discover exchange? How do trade relationships stabilize or shift?",
    },
    {
        "id": "resource_inequality",
        "category": "Economic Dynamics",
        "min_agents": 5,
        "question": "How does resource inequality develop over time? Does wealth concentrate? At what rate? Does it self-correct or compound?",
    },
    {
        "id": "scarcity_response",
        "category": "Economic Dynamics",
        "min_agents": 5,
        "question": "How do agents respond to scarcity? Cooperation, hoarding, conflict, migration, innovation? How do responses change as scarcity intensifies?",
    },
    {
        "id": "shared_practices",
        "category": "Cultural & Social Practices",
        "min_agents": 10,
        "question": "How do shared practices and norms form? Do agents develop conventions, rituals, or behavioral norms? How do they spread?",
    },
    {
        "id": "bonding_structures",
        "category": "Cultural & Social Practices",
        "min_agents": 10,
        "question": "How do bonding and family structures develop? Monogamy, communal arrangements, or something else? How do reproduction decisions interact with resource availability?",
    },
    {
        "id": "deception",
        "category": "Cultural & Social Practices",
        "min_agents": 10,
        "question": "How does deception operate in agent societies? When do agents lie? To whom? How do other agents respond when deception is discovered?",
    },
    {
        "id": "disruption_response",
        "category": "Response to Disruption",
        "min_agents": 10,
        "question": "How do agent societies respond to exogenous shocks? Resource depletion, environmental change, population loss. Does the social structure adapt, collapse, or reorganize?",
    },
    {
        "id": "personality_effects",
        "category": "Response to Disruption",
        "min_agents": 10,
        "question": "How do different personality distributions produce different outcomes? Does initial personality distribution determine long-term social structure, or do the dynamics converge?",
    },
    {
        "id": "convergence_divergence",
        "category": "Meta",
        "min_agents": 5,
        "question": "Where do agent trajectories converge with human historical patterns, and where do they diverge?",
    },
    {
        "id": "automation_threshold",
        "category": "Meta",
        "min_agents": 5,
        "question": "What percentage of decisions can be automated without losing behavioral complexity?",
    },
    {
        "id": "novel_behaviors",
        "category": "Meta",
        "min_agents": 5,
        "question": "Do agents produce genuinely novel behaviors that were not anticipated by the system designers?",
    },
    {
        "id": "feasibility",
        "category": "Meta",
        "min_agents": 5,
        "question": "Can a 24-hour bounded run at the given agent count produce observable social dynamics worth analyzing?",
    },
]


def load_config(config_path: str) -> dict:
    """Load and parse the emergence-config.yaml file."""
    path = Path(config_path)
    if not path.exists():
        print(f"ERROR: Config file not found: {config_path}", file=sys.stderr)
        sys.exit(1)

    with open(path, "r") as f:
        return yaml.safe_load(f)


def get_api_key() -> str:
    """Resolve the OpenRouter API key from environment variables."""
    key = os.environ.get("OPENROUTER_API_KEY") or os.environ.get("LLM_DEFAULT_API_KEY")
    if not key:
        print(
            "ERROR: No API key found. Set OPENROUTER_API_KEY or LLM_DEFAULT_API_KEY.",
            file=sys.stderr,
        )
        sys.exit(1)
    return key


def build_prediction_prompt(config: dict, agent_count: int, tick_count: int, questions: list[dict]) -> str:
    """Build the prompt that asks the LLM to predict simulation outcomes."""

    personality_mode = config.get("agents", {}).get("personality_mode", "random")
    seed_knowledge = config.get("agents", {}).get("seed_knowledge", [])
    knowledge_level = config.get("world", {}).get("knowledge_level", 1)
    starting_wallet = config.get("economy", {}).get("starting_wallet", {})
    hunger_rate = config.get("economy", {}).get("hunger_rate", 5)
    ticks_per_season = config.get("time", {}).get("ticks_per_season", 90)
    agent_lifespan = config.get("population", {}).get("agent_lifespan_ticks", 2500)
    reproduction = config.get("population", {}).get("reproduction_enabled", True)
    day_night = config.get("time", {}).get("day_night", True)

    world_years = tick_count / (ticks_per_season * 4) if ticks_per_season > 0 else 0

    questions_block = ""
    for i, q in enumerate(questions, 1):
        questions_block += f"\n{i}. [{q['category']}] {q['question']}\n"

    prompt = f"""You are being asked to predict the outcomes of a multi-agent LLM simulation BEFORE it runs. This is a pre-registration exercise -- your predictions will be compared against actual outcomes to assess how much agent behavior is predictable from LLM training priors versus emergent from persistent interaction.

## Simulation Parameters

- **Agent count:** {agent_count}
- **Total ticks:** {tick_count} (~{world_years:.1f} world years)
- **Ticks per season:** {ticks_per_season} (4 seasons per year)
- **Agent lifespan:** {agent_lifespan} ticks (~{agent_lifespan / (ticks_per_season * 4):.1f} world years)
- **Personality distribution:** {personality_mode} (8 dimensions: curiosity, cooperation, aggression, risk tolerance, industriousness, sociability, honesty, loyalty)
- **Knowledge level:** {knowledge_level} (0=blank slate, 1=primitive, 2=ancient, 3=medieval)
- **Seed knowledge:** {', '.join(seed_knowledge) if seed_knowledge else 'none'}
- **Starting resources per agent:** {json.dumps(starting_wallet)}
- **Hunger rate:** {hunger_rate} per tick
- **Reproduction:** {'enabled' if reproduction else 'disabled'}
- **Day/night cycle:** {'enabled' if day_night else 'disabled'}

## World Design

- Agents inhabit a graph of connected locations with varying resources
- Resources are finite and regenerate at fixed rates per location per tick
- All resource movements go through a double-entry ledger (conservation law enforced)
- Agents can only see their current location (fog of war)
- Agents can propose freeform actions beyond a base catalog (gather, build, trade, move, rest, etc.)
- Agents have tiered memory (immediate, short-term, long-term) with compression over time
- Agents have persistent social graphs with trust scores

## Population Scale Note

At {agent_count} agents, this is a {'band-level (very small group)' if agent_count < 10 else 'small village' if agent_count < 30 else 'village'} scale simulation. Keep your predictions calibrated to what is plausible at this population size.

## Research Questions

For each question below, provide a specific, falsifiable prediction about what you expect to happen in this simulation. Be concrete -- reference tick ranges, percentages, specific behavioral patterns. Do NOT hedge with "it depends" -- commit to a prediction even if uncertain.

{questions_block}

## Response Format

Respond with a JSON array. Each element must have exactly these fields:
- "question_id": the identifier string for the question
- "prediction": your specific prediction (2-4 sentences, concrete and falsifiable)
- "confidence": your confidence level ("low", "medium", "high")
- "expected_tick_range": approximate tick range when you expect this to become observable, as [start, end] or null if not applicable
- "key_indicators": list of 2-3 specific observable indicators that would confirm or refute your prediction

Respond ONLY with the JSON array, no other text."""

    return prompt


def query_llm(api_key: str, model: str, prompt: str, api_url: str) -> dict:
    """Send the prediction prompt to the LLM and parse the response."""

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": "http://localhost:8080",
        "X-Title": "Emergence Pre-Registration",
    }

    payload = {
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a social scientist specializing in multi-agent systems and computational sociology. You are making predictions about simulation outcomes for a pre-registration protocol. Be specific and falsifiable. Respond only with valid JSON.",
            },
            {"role": "user", "content": prompt},
        ],
        "temperature": 0.3,
        "max_tokens": 8000,
        "response_format": {"type": "json_object"},
    }

    print(f"Querying {model} via {api_url}...")
    start = time.monotonic()

    try:
        response = requests.post(
            f"{api_url}/chat/completions",
            headers=headers,
            json=payload,
            timeout=120,
        )
        response.raise_for_status()
    except requests.exceptions.RequestException as e:
        print(f"ERROR: LLM API request failed: {e}", file=sys.stderr)
        if hasattr(e, "response") and e.response is not None:
            print(f"Response body: {e.response.text}", file=sys.stderr)
        sys.exit(1)

    elapsed = time.monotonic() - start
    print(f"Response received in {elapsed:.1f}s")

    result = response.json()
    content = result["choices"][0]["message"]["content"]

    # Parse the JSON response -- handle both bare array and wrapped object
    try:
        parsed = json.loads(content)
        if isinstance(parsed, list):
            return {"predictions": parsed}
        if isinstance(parsed, dict):
            # The model may wrap the array in an object
            if "predictions" in parsed:
                return parsed
            # Or it may return a dict with question_ids as keys
            return {"predictions": list(parsed.values()) if all(isinstance(v, dict) for v in parsed.values()) else [parsed]}
        print(f"ERROR: Unexpected JSON structure from LLM: {type(parsed)}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"ERROR: Failed to parse LLM response as JSON: {e}", file=sys.stderr)
        print(f"Raw response:\n{content}", file=sys.stderr)
        sys.exit(1)


def save_to_db(registration: dict, db_url: str) -> None:
    """Optionally save the pre-registration to PostgreSQL."""
    try:
        import psycopg2
    except ImportError:
        print(
            "WARNING: psycopg2 not installed. Skipping database save. Install with: pip install psycopg2-binary",
            file=sys.stderr,
        )
        return

    try:
        conn = psycopg2.connect(db_url)
        cur = conn.cursor()

        # Create the pre_registrations table if it does not exist
        cur.execute("""
            CREATE TABLE IF NOT EXISTS pre_registrations (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                experiment_config JSONB NOT NULL,
                model TEXT NOT NULL,
                predictions JSONB NOT NULL,
                run_id UUID REFERENCES simulation_runs(id),
                comparison_report JSONB
            )
        """)

        cur.execute(
            """
            INSERT INTO pre_registrations (created_at, experiment_config, model, predictions)
            VALUES (%s, %s, %s, %s)
            RETURNING id
            """,
            (
                registration["timestamp"],
                json.dumps(registration["experiment_config"]),
                registration["model"],
                json.dumps(registration["predictions"]),
            ),
        )

        row_id = cur.fetchone()[0]
        conn.commit()
        cur.close()
        conn.close()
        print(f"Saved to PostgreSQL: pre_registrations.id = {row_id}")

    except Exception as e:
        print(f"WARNING: Failed to save to database: {e}", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(
        description="Emergence Pre-Registration: generate LLM baseline predictions before a simulation run.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python3 scripts/pre-register.py --config emergence-config.yaml --agents 10 --ticks 5000
  python3 scripts/pre-register.py --config emergence-config.yaml --agents 50 --ticks 30000 --model anthropic/claude-sonnet-4-5-20250929
  python3 scripts/pre-register.py --config emergence-config.yaml --agents 10 --ticks 5000 --save-to-db
        """,
    )
    parser.add_argument(
        "--config",
        required=True,
        help="Path to emergence-config.yaml",
    )
    parser.add_argument(
        "--agents",
        type=int,
        required=True,
        help="Number of agents in the experiment",
    )
    parser.add_argument(
        "--ticks",
        type=int,
        required=True,
        help="Total ticks for the experiment",
    )
    parser.add_argument(
        "--model",
        default="deepseek/deepseek-chat-v3-0324",
        help="LLM model to query for predictions (default: deepseek/deepseek-chat-v3-0324)",
    )
    parser.add_argument(
        "--api-url",
        default="https://openrouter.ai/api/v1",
        help="LLM API base URL (default: OpenRouter)",
    )
    parser.add_argument(
        "--output-dir",
        default="results",
        help="Directory for output files (default: results/)",
    )
    parser.add_argument(
        "--save-to-db",
        action="store_true",
        help="Also save predictions to PostgreSQL (requires DATABASE_URL env var and psycopg2)",
    )

    args = parser.parse_args()

    # Load config
    config = load_config(args.config)
    api_key = get_api_key()

    # Filter questions by minimum agent count
    applicable_questions = [
        q for q in RESEARCH_QUESTIONS if q["min_agents"] <= args.agents
    ]

    skipped = len(RESEARCH_QUESTIONS) - len(applicable_questions)
    if skipped > 0:
        print(f"Skipping {skipped} questions that require more than {args.agents} agents.")

    if not applicable_questions:
        print("ERROR: No research questions are applicable at this agent count.", file=sys.stderr)
        sys.exit(1)

    print(f"\nPre-registration for {args.agents} agents, {args.ticks} ticks")
    print(f"Model: {args.model}")
    print(f"Applicable questions: {len(applicable_questions)}/{len(RESEARCH_QUESTIONS)}")
    print()

    # Build prompt and query LLM
    prompt = build_prediction_prompt(config, args.agents, args.ticks, applicable_questions)
    result = query_llm(api_key, args.model, prompt, args.api_url)

    # Assemble the full registration document
    now = datetime.now(timezone.utc)
    timestamp_str = now.strftime("%Y%m%d-%H%M%S")

    registration = {
        "schema_version": 1,
        "timestamp": now.isoformat(),
        "model": args.model,
        "experiment_config": {
            "agent_count": args.agents,
            "tick_count": args.ticks,
            "personality_mode": config.get("agents", {}).get("personality_mode", "random"),
            "knowledge_level": config.get("world", {}).get("knowledge_level", 1),
            "seed_knowledge": config.get("agents", {}).get("seed_knowledge", []),
            "starting_wallet": config.get("economy", {}).get("starting_wallet", {}),
            "reproduction_enabled": config.get("population", {}).get("reproduction_enabled", True),
            "ticks_per_season": config.get("time", {}).get("ticks_per_season", 90),
            "agent_lifespan_ticks": config.get("population", {}).get("agent_lifespan_ticks", 2500),
            "config_file": args.config,
        },
        "research_questions": applicable_questions,
        "predictions": result.get("predictions", []),
    }

    # Write output
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_file = output_dir / f"pre-registration-{timestamp_str}.json"

    with open(output_file, "w") as f:
        json.dump(registration, f, indent=2)

    print(f"\nPre-registration saved to: {output_file}")
    print(f"Predictions: {len(registration['predictions'])}")

    # Optionally save to database
    if args.save_to_db:
        db_url = os.environ.get("DATABASE_URL")
        if not db_url:
            print("WARNING: --save-to-db specified but DATABASE_URL not set. Skipping.", file=sys.stderr)
        else:
            save_to_db(registration, db_url)

    # Print summary
    print("\n--- Prediction Summary ---\n")
    for pred in registration["predictions"]:
        qid = pred.get("question_id", "unknown")
        confidence = pred.get("confidence", "?")
        prediction_text = pred.get("prediction", "(no prediction)")
        tick_range = pred.get("expected_tick_range")
        tick_str = f" [ticks {tick_range[0]}-{tick_range[1]}]" if tick_range else ""

        print(f"  [{confidence.upper():>6}] {qid}{tick_str}")
        print(f"          {prediction_text}")
        print()


if __name__ == "__main__":
    main()
