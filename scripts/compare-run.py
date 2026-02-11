#!/usr/bin/env python3
"""
Emergence -- Run Comparison Report

Compares actual simulation outcomes against pre-registered LLM predictions.
Generates a markdown divergence report highlighting where the simulation
produced different dynamics than predicted.

Divergences are the scientifically interesting findings -- they suggest the
simulation is producing dynamics that cannot be predicted from LLM training
priors alone.

Usage:
    python3 scripts/compare-run.py --predictions results/pre-registration-YYYYMMDD-HHMMSS.json --run-id <uuid>
    python3 scripts/compare-run.py --predictions results/pre-registration-YYYYMMDD-HHMMSS.json --run-id <uuid> --output results/comparison-report.md

Requirements:
    pip install requests psycopg2-binary pyyaml

Environment:
    DATABASE_URL        -- PostgreSQL connection string
    OPENROUTER_API_KEY  -- API key for OpenRouter (or LLM_DEFAULT_API_KEY)
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
    import psycopg2
    import psycopg2.extras
except ImportError:
    print("ERROR: 'psycopg2' package is required. Install with: pip install psycopg2-binary", file=sys.stderr)
    sys.exit(1)


def get_db_connection(db_url: str):
    """Connect to PostgreSQL."""
    try:
        conn = psycopg2.connect(db_url)
        return conn
    except Exception as e:
        print(f"ERROR: Failed to connect to database: {e}", file=sys.stderr)
        sys.exit(1)


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


def extract_run_outcomes(conn, run_id: str) -> dict:
    """Extract observable outcomes from a completed simulation run."""
    cur = conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor)
    outcomes = {}

    # --- Basic run info ---
    cur.execute(
        "SELECT * FROM simulation_runs WHERE id = %s",
        (run_id,),
    )
    run = cur.fetchone()
    if not run:
        print(f"ERROR: No simulation run found with id: {run_id}", file=sys.stderr)
        sys.exit(1)

    outcomes["run"] = {
        "id": str(run["id"]),
        "name": run.get("name", ""),
        "status": run.get("status", "unknown"),
        "max_ticks": run.get("max_ticks"),
    }

    # --- Population statistics ---
    cur.execute("""
        SELECT
            COUNT(*) AS total_agents,
            COUNT(*) FILTER (WHERE died_at_tick IS NULL) AS alive_at_end,
            COUNT(*) FILTER (WHERE died_at_tick IS NOT NULL) AS total_deaths,
            COUNT(*) FILTER (WHERE parent_a IS NOT NULL) AS born_in_sim,
            COUNT(*) FILTER (WHERE generation = 0) AS seed_agents,
            MAX(generation) AS max_generation,
            AVG(CASE WHEN died_at_tick IS NOT NULL THEN died_at_tick - born_at_tick END) AS avg_lifespan
        FROM agents
    """)
    pop = cur.fetchone()
    outcomes["population"] = dict(pop) if pop else {}

    # --- Death causes ---
    cur.execute("""
        SELECT cause_of_death, COUNT(*) AS count
        FROM agents
        WHERE died_at_tick IS NOT NULL AND cause_of_death IS NOT NULL
        GROUP BY cause_of_death
        ORDER BY count DESC
    """)
    outcomes["death_causes"] = [dict(row) for row in cur.fetchall()]

    # --- Trade statistics ---
    cur.execute("""
        SELECT
            COUNT(*) AS total_trades,
            COUNT(DISTINCT agent_id) AS unique_traders,
            MIN(tick) AS first_trade_tick,
            MAX(tick) AS last_trade_tick
        FROM events
        WHERE event_type = 'Trade' OR event_type = 'TRADE'
    """)
    trade = cur.fetchone()
    outcomes["trade"] = dict(trade) if trade else {}

    # --- Discovery timeline ---
    cur.execute("""
        SELECT knowledge_item, tick, method,
               agent_id
        FROM discoveries
        ORDER BY tick
    """)
    outcomes["discoveries"] = [
        {
            "knowledge": row["knowledge_item"],
            "tick": row["tick"],
            "method": row["method"],
            "agent_id": str(row["agent_id"]) if row["agent_id"] else None,
        }
        for row in cur.fetchall()
    ]

    # --- Social constructs ---
    cur.execute("""
        SELECT name, category, founded_at_tick, disbanded_at_tick
        FROM social_constructs
        ORDER BY founded_at_tick
    """)
    outcomes["social_constructs"] = [dict(row) for row in cur.fetchall()]

    # --- Deception statistics ---
    cur.execute("""
        SELECT
            COUNT(*) AS total_lies,
            COUNT(*) FILTER (WHERE discovered = true) AS discovered_lies,
            COUNT(DISTINCT deceiver) AS unique_deceivers
        FROM deception_records
    """)
    deception = cur.fetchone()
    outcomes["deception"] = dict(deception) if deception else {}

    # --- Wealth distribution (final snapshot) ---
    cur.execute("""
        SELECT tick, total_resources
        FROM world_snapshots
        ORDER BY tick DESC
        LIMIT 1
    """)
    final_snapshot = cur.fetchone()
    if final_snapshot:
        outcomes["final_world_state"] = {
            "tick": final_snapshot["tick"],
            "total_resources": final_snapshot["total_resources"],
        }

    # --- Conflict events ---
    cur.execute("""
        SELECT
            COUNT(*) FILTER (WHERE event_type IN ('Combat', 'COMBAT')) AS combat_events,
            COUNT(*) FILTER (WHERE event_type IN ('Theft', 'THEFT')) AS theft_events,
            COUNT(*) FILTER (WHERE event_type IN ('Diplomacy', 'DIPLOMACY')) AS diplomacy_events
        FROM events
    """)
    conflict = cur.fetchone()
    outcomes["conflict"] = dict(conflict) if conflict else {}

    # --- Ledger summary ---
    cur.execute("""
        SELECT entry_type, COUNT(*) AS count, SUM(quantity) AS total_quantity
        FROM ledger
        GROUP BY entry_type
        ORDER BY count DESC
    """)
    outcomes["ledger_summary"] = [dict(row) for row in cur.fetchall()]

    # --- Event type distribution ---
    cur.execute("""
        SELECT event_type, COUNT(*) AS count
        FROM events
        GROUP BY event_type
        ORDER BY count DESC
    """)
    outcomes["event_distribution"] = [dict(row) for row in cur.fetchall()]

    # --- Tick range ---
    cur.execute("SELECT MIN(tick) AS first_tick, MAX(tick) AS last_tick FROM events")
    tick_range = cur.fetchone()
    outcomes["tick_range"] = dict(tick_range) if tick_range else {}

    cur.close()
    return outcomes


def build_comparison_prompt(predictions: list[dict], outcomes: dict) -> str:
    """Build the prompt that asks the LLM to compare predictions against outcomes."""

    outcomes_json = json.dumps(outcomes, indent=2, default=str)

    predictions_block = ""
    for pred in predictions:
        qid = pred.get("question_id", "unknown")
        prediction_text = pred.get("prediction", "(no prediction)")
        confidence = pred.get("confidence", "unknown")
        indicators = pred.get("key_indicators", [])
        tick_range = pred.get("expected_tick_range")

        predictions_block += f"\n### {qid}\n"
        predictions_block += f"**Prediction ({confidence} confidence):** {prediction_text}\n"
        if tick_range:
            predictions_block += f"**Expected tick range:** {tick_range[0]}-{tick_range[1]}\n"
        if indicators:
            predictions_block += f"**Key indicators:** {', '.join(indicators)}\n"

    prompt = f"""You are evaluating a pre-registered prediction set against actual simulation outcomes. Your job is to assess each prediction and classify it as CONFIRMED, PARTIALLY CONFIRMED, DIVERGENT, or INSUFFICIENT DATA.

## Pre-Registered Predictions
{predictions_block}

## Actual Simulation Outcomes

```json
{outcomes_json}
```

## Instructions

For each prediction, provide:
1. **verdict**: One of "confirmed", "partially_confirmed", "divergent", "insufficient_data"
2. **evidence**: Specific data from the outcomes that supports your verdict (reference actual numbers)
3. **divergence_notes**: If divergent or partially confirmed, explain specifically HOW the outcome differed from prediction. This is the most important field -- divergences are the scientifically interesting findings.
4. **surprise_factor**: Rate 1-5 how surprising the outcome was relative to the prediction (1 = exactly as predicted, 5 = completely unexpected)

Also provide:
5. **overall_summary**: A 3-5 sentence summary of the most significant findings across all questions
6. **most_interesting_divergences**: List the top 3 most scientifically interesting divergences
7. **recapitulation_assessment**: Your assessment of how much agent behavior appears to be training-data recapitulation vs genuine emergent dynamics

Respond ONLY with a JSON object containing:
- "comparisons": array of objects with fields question_id, verdict, evidence, divergence_notes, surprise_factor
- "overall_summary": string
- "most_interesting_divergences": array of strings
- "recapitulation_assessment": string"""

    return prompt


def query_llm(api_key: str, model: str, prompt: str, api_url: str) -> dict:
    """Send the comparison prompt to the LLM and parse the response."""

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": "http://localhost:8080",
        "X-Title": "Emergence Run Comparison",
    }

    payload = {
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a social scientist evaluating simulation results against pre-registered predictions. Be rigorous and specific. Reference actual data in your assessments. Respond only with valid JSON.",
            },
            {"role": "user", "content": prompt},
        ],
        "temperature": 0.2,
        "max_tokens": 8000,
        "response_format": {"type": "json_object"},
    }

    print(f"Querying {model} for comparison analysis...")
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

    try:
        return json.loads(content)
    except json.JSONDecodeError as e:
        print(f"ERROR: Failed to parse LLM response as JSON: {e}", file=sys.stderr)
        print(f"Raw response:\n{content}", file=sys.stderr)
        sys.exit(1)


def generate_markdown_report(
    predictions_file: str,
    registration: dict,
    outcomes: dict,
    comparison: dict,
) -> str:
    """Generate a markdown divergence report."""

    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    config = registration.get("experiment_config", {})

    report = f"""# Emergence -- Run Comparison Report

**Generated:** {now}
**Predictions file:** `{predictions_file}`
**Run ID:** {outcomes['run']['id']}
**Run status:** {outcomes['run']['status']}

## Experiment Configuration

| Parameter | Value |
|---|---|
| Agent count | {config.get('agent_count', '?')} |
| Tick count | {config.get('tick_count', '?')} |
| Personality mode | {config.get('personality_mode', '?')} |
| Knowledge level | {config.get('knowledge_level', '?')} |
| Reproduction | {'enabled' if config.get('reproduction_enabled', True) else 'disabled'} |
| Prediction model | {registration.get('model', '?')} |

## Simulation Summary

| Metric | Value |
|---|---|
| Tick range | {outcomes.get('tick_range', {}).get('first_tick', '?')} - {outcomes.get('tick_range', {}).get('last_tick', '?')} |
| Total agents created | {outcomes.get('population', {}).get('total_agents', '?')} |
| Alive at end | {outcomes.get('population', {}).get('alive_at_end', '?')} |
| Total deaths | {outcomes.get('population', {}).get('total_deaths', '?')} |
| Born in simulation | {outcomes.get('population', {}).get('born_in_sim', '?')} |
| Max generation | {outcomes.get('population', {}).get('max_generation', '?')} |
| Total trades | {outcomes.get('trade', {}).get('total_trades', '?')} |
| First trade at tick | {outcomes.get('trade', {}).get('first_trade_tick', '?')} |
| Unique traders | {outcomes.get('trade', {}).get('unique_traders', '?')} |
| Combat events | {outcomes.get('conflict', {}).get('combat_events', '?')} |
| Theft events | {outcomes.get('conflict', {}).get('theft_events', '?')} |
| Diplomacy events | {outcomes.get('conflict', {}).get('diplomacy_events', '?')} |
| Total lies told | {outcomes.get('deception', {}).get('total_lies', '?')} |
| Lies discovered | {outcomes.get('deception', {}).get('discovered_lies', '?')} |
| Unique deceivers | {outcomes.get('deception', {}).get('unique_deceivers', '?')} |
| Social constructs formed | {len(outcomes.get('social_constructs', []))} |
| Discoveries made | {len(outcomes.get('discoveries', []))} |

"""

    # Discovery timeline
    discoveries = outcomes.get("discoveries", [])
    if discoveries:
        report += "## Discovery Timeline\n\n"
        report += "| Tick | Knowledge | Method |\n"
        report += "|---|---|---|\n"
        for d in discoveries[:30]:  # Cap at 30 for readability
            report += f"| {d['tick']} | {d['knowledge']} | {d['method']} |\n"
        if len(discoveries) > 30:
            report += f"\n*...and {len(discoveries) - 30} more discoveries.*\n"
        report += "\n"

    # Social constructs
    constructs = outcomes.get("social_constructs", [])
    if constructs:
        report += "## Social Constructs\n\n"
        report += "| Name | Category | Founded (tick) | Disbanded (tick) |\n"
        report += "|---|---|---|---|\n"
        for c in constructs:
            disbanded = c.get("disbanded_at_tick", "--")
            report += f"| {c['name']} | {c['category']} | {c['founded_at_tick']} | {disbanded} |\n"
        report += "\n"

    # Comparison results
    comparisons = comparison.get("comparisons", [])
    report += "## Prediction vs Outcome Comparison\n\n"

    # Verdict summary
    verdict_counts = {}
    for c in comparisons:
        v = c.get("verdict", "unknown")
        verdict_counts[v] = verdict_counts.get(v, 0) + 1

    report += "### Verdict Summary\n\n"
    report += "| Verdict | Count |\n"
    report += "|---|---|\n"
    for verdict, count in sorted(verdict_counts.items()):
        report += f"| {verdict} | {count} |\n"
    report += "\n"

    # Individual comparisons
    report += "### Detailed Comparisons\n\n"
    for c in comparisons:
        qid = c.get("question_id", "unknown")
        verdict = c.get("verdict", "unknown").upper()
        evidence = c.get("evidence", "")
        divergence = c.get("divergence_notes", "")
        surprise = c.get("surprise_factor", "?")

        # Find the original prediction
        original_pred = ""
        for p in registration.get("predictions", []):
            if p.get("question_id") == qid:
                original_pred = p.get("prediction", "")
                break

        report += f"#### {qid} -- {verdict} (surprise: {surprise}/5)\n\n"
        report += f"**Prediction:** {original_pred}\n\n"
        report += f"**Evidence:** {evidence}\n\n"
        if divergence:
            report += f"**Divergence:** {divergence}\n\n"
        report += "---\n\n"

    # Overall assessment
    report += "## Overall Assessment\n\n"
    report += comparison.get("overall_summary", "(no summary provided)") + "\n\n"

    # Most interesting divergences
    divergences = comparison.get("most_interesting_divergences", [])
    if divergences:
        report += "### Most Interesting Divergences\n\n"
        for i, d in enumerate(divergences, 1):
            report += f"{i}. {d}\n"
        report += "\n"

    # Recapitulation assessment
    recap = comparison.get("recapitulation_assessment", "")
    if recap:
        report += "### Recapitulation Assessment\n\n"
        report += recap + "\n\n"

    report += "---\n\n"
    report += "*Report generated by `scripts/compare-run.py`. See the pre-registration framework documentation in README.md.*\n"

    return report


def main():
    parser = argparse.ArgumentParser(
        description="Emergence Run Comparison: compare simulation outcomes against pre-registered predictions.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python3 scripts/compare-run.py --predictions results/pre-registration-20260211-143200.json --run-id 01234567-89ab-cdef-0123-456789abcdef
  python3 scripts/compare-run.py --predictions results/pre-registration-20260211-143200.json --run-id <uuid> --output results/comparison.md
        """,
    )
    parser.add_argument(
        "--predictions",
        required=True,
        help="Path to the pre-registration JSON file",
    )
    parser.add_argument(
        "--run-id",
        required=True,
        help="UUID of the simulation run to compare against",
    )
    parser.add_argument(
        "--model",
        default="deepseek/deepseek-chat-v3-0324",
        help="LLM model to use for comparison analysis (default: deepseek/deepseek-chat-v3-0324)",
    )
    parser.add_argument(
        "--api-url",
        default="https://openrouter.ai/api/v1",
        help="LLM API base URL (default: OpenRouter)",
    )
    parser.add_argument(
        "--output",
        help="Output path for the markdown report (default: results/comparison-YYYYMMDD-HHMMSS.md)",
    )
    parser.add_argument(
        "--save-to-db",
        action="store_true",
        help="Also save the comparison report to the pre_registrations table",
    )

    args = parser.parse_args()

    # Load predictions
    predictions_path = Path(args.predictions)
    if not predictions_path.exists():
        print(f"ERROR: Predictions file not found: {args.predictions}", file=sys.stderr)
        sys.exit(1)

    with open(predictions_path, "r") as f:
        registration = json.load(f)

    predictions = registration.get("predictions", [])
    if not predictions:
        print("ERROR: No predictions found in the pre-registration file.", file=sys.stderr)
        sys.exit(1)

    print(f"Loaded {len(predictions)} predictions from {args.predictions}")

    # Connect to database
    db_url = os.environ.get("DATABASE_URL")
    if not db_url:
        print("ERROR: DATABASE_URL environment variable is required.", file=sys.stderr)
        sys.exit(1)

    conn = get_db_connection(db_url)

    # Extract outcomes
    print(f"Extracting outcomes for run {args.run_id}...")
    outcomes = extract_run_outcomes(conn, args.run_id)
    print(f"Extracted outcomes: {outcomes['tick_range'].get('last_tick', 0)} ticks, {outcomes['population'].get('total_agents', 0)} agents")

    # Query LLM for comparison
    api_key = get_api_key()
    prompt = build_comparison_prompt(predictions, outcomes)
    comparison = query_llm(api_key, args.model, prompt, args.api_url)

    # Generate markdown report
    report = generate_markdown_report(args.predictions, registration, outcomes, comparison)

    # Determine output path
    if args.output:
        output_path = Path(args.output)
    else:
        timestamp_str = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
        output_dir = Path("results")
        output_dir.mkdir(parents=True, exist_ok=True)
        output_path = output_dir / f"comparison-{timestamp_str}.md"

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        f.write(report)

    print(f"\nComparison report saved to: {output_path}")

    # Also save the raw comparison JSON
    json_path = output_path.with_suffix(".json")
    comparison_data = {
        "schema_version": 1,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "predictions_file": str(args.predictions),
        "run_id": args.run_id,
        "model": args.model,
        "outcomes": outcomes,
        "comparison": comparison,
    }
    with open(json_path, "w") as f:
        json.dump(comparison_data, f, indent=2, default=str)
    print(f"Raw comparison data saved to: {json_path}")

    # Optionally update the database
    if args.save_to_db:
        try:
            cur = conn.cursor()
            # Try to find and update the matching pre-registration record
            cur.execute(
                """
                UPDATE pre_registrations
                SET run_id = %s, comparison_report = %s
                WHERE predictions = %s::jsonb
                """,
                (
                    args.run_id,
                    json.dumps(comparison),
                    json.dumps(registration.get("predictions", [])),
                ),
            )
            if cur.rowcount > 0:
                conn.commit()
                print(f"Updated pre_registrations record with comparison data.")
            else:
                print("NOTE: No matching pre_registrations record found to update.")
            cur.close()
        except Exception as e:
            print(f"WARNING: Failed to update database: {e}", file=sys.stderr)

    conn.close()

    # Print summary
    comparisons = comparison.get("comparisons", [])
    print("\n--- Comparison Summary ---\n")

    for c in comparisons:
        qid = c.get("question_id", "unknown")
        verdict = c.get("verdict", "unknown")
        surprise = c.get("surprise_factor", "?")
        print(f"  [{verdict.upper():>20}] {qid} (surprise: {surprise}/5)")

    print()

    overall = comparison.get("overall_summary", "")
    if overall:
        print(f"Overall: {overall}")
        print()

    divergences = comparison.get("most_interesting_divergences", [])
    if divergences:
        print("Most interesting divergences:")
        for i, d in enumerate(divergences, 1):
            print(f"  {i}. {d}")
        print()


if __name__ == "__main__":
    main()
