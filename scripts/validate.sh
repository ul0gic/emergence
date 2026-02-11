#!/usr/bin/env bash
# =============================================================================
# Emergence -- Validate Running Simulation
# =============================================================================
# Checks the health and state of a running Emergence simulation:
#   1. Docker container status (all 5 services)
#   2. PostgreSQL connectivity + migration verification
#   3. Dragonfly (Redis) connectivity
#   4. NATS monitoring endpoint
#   5. Observer API responsiveness
#   6. Event store activity (are events being written?)
#   7. Agent state in Dragonfly (are agents stored?)
#   8. Summary report
#
# Usage:
#   ./scripts/validate.sh           # Run all checks
#   ./scripts/validate.sh --quiet   # Only print summary (exit code 0/1)
#   ./scripts/validate.sh --help    # Show this help
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Resolve project root
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ---------------------------------------------------------------------------
# Color output helpers
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[FAIL]${NC} $*"; }
header()  { echo -e "\n${BOLD}${CYAN}=== $* ===${NC}\n"; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
QUIET=false

for arg in "$@"; do
    case "$arg" in
        --quiet|-q)
            QUIET=true
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --quiet, -q   Only print the final summary"
            echo "  --help, -h    Show this help message"
            echo ""
            echo "Validates that the Emergence simulation is running correctly."
            echo "Checks all services, database state, and agent activity."
            echo "Exit code 0 if all critical checks pass, 1 otherwise."
            exit 0
            ;;
        *)
            error "Unknown option: $arg"
            echo "Use --help for usage information."
            exit 1
            ;;
    esac
done

# Suppress output in quiet mode
log_info()    { [ "${QUIET}" = false ] && info "$@"    || true; }
log_success() { [ "${QUIET}" = false ] && success "$@" || true; }
log_warn()    { [ "${QUIET}" = false ] && warn "$@"    || true; }
log_error()   { [ "${QUIET}" = false ] && error "$@"   || true; }
log_header()  { [ "${QUIET}" = false ] && header "$@"  || true; }

# ---------------------------------------------------------------------------
# Load .env for connection parameters
# ---------------------------------------------------------------------------
ENV_FILE="${PROJECT_ROOT}/.env"
if [ -f "${ENV_FILE}" ]; then
    set -a
    # shellcheck disable=SC1090
    source <(grep -v '^\s*#' "${ENV_FILE}" | grep -v '^\s*$' | sed 's/^export //')
    set +a
fi

PG_CONTAINER="emergence-postgres"
PG_DB="${POSTGRES_DB:-emergence}"
PG_USER="${POSTGRES_USER:-emergence}"
PG_PASS="${POSTGRES_PASSWORD:-}"
DRAGONFLY_CONTAINER="emergence-dragonfly"
NATS_CONTAINER="emergence-nats"
OBSERVER_PORT="${OBSERVER_PORT:-8080}"

# Helper: run psql inside the postgres container.
pg_exec() {
    docker exec -e PGPASSWORD="${PG_PASS}" "${PG_CONTAINER}" \
        psql -U "${PG_USER}" -d "${PG_DB}" "$@"
}

# Helper: run redis-cli inside the dragonfly container.
df_exec() {
    docker exec "${DRAGONFLY_CONTAINER}" redis-cli "$@"
}

# Helper: curl inside the nats container for monitoring endpoints.
nats_health() {
    docker exec "${NATS_CONTAINER}" wget -qO- "$@" 2>/dev/null
}

# ---------------------------------------------------------------------------
# Tracking variables
# ---------------------------------------------------------------------------
CHECKS_PASSED=0
CHECKS_FAILED=0
CHECKS_WARNED=0

pass()  { CHECKS_PASSED=$((CHECKS_PASSED + 1)); log_success "$@"; }
fail()  { CHECKS_FAILED=$((CHECKS_FAILED + 1)); log_error "$@"; }
warned() { CHECKS_WARNED=$((CHECKS_WARNED + 1)); log_warn "$@"; }

# Summary data (populated during checks)
CONTAINERS_UP=0
CONTAINERS_TOTAL=5
EVENTS_COUNT="N/A"
AGENTS_ALIVE="N/A"
TICKS_PROCESSED="N/A"
AGENT_KEYS_COUNT="N/A"
TABLES_FOUND="N/A"

# ---------------------------------------------------------------------------
# 1. Docker container status
# ---------------------------------------------------------------------------
log_header "Docker Container Status"

EXPECTED_CONTAINERS=(
    "emergence-dragonfly"
    "emergence-postgres"
    "emergence-nats"
    "emergence-engine"
    "emergence-runner"
)

for container in "${EXPECTED_CONTAINERS[@]}"; do
    status=$(docker inspect --format='{{.State.Status}}' "${container}" 2>/dev/null || echo "not_found")
    case "${status}" in
        running)
            pass "${container} is running"
            CONTAINERS_UP=$((CONTAINERS_UP + 1))
            ;;
        not_found)
            fail "${container} does not exist (not started?)"
            ;;
        *)
            fail "${container} status: ${status}"
            ;;
    esac
done

# ---------------------------------------------------------------------------
# 2. PostgreSQL connectivity + migration verification
# ---------------------------------------------------------------------------
log_header "PostgreSQL Database"

if [ -z "${PG_PASS}" ]; then
    fail "POSTGRES_PASSWORD not set -- cannot connect to database"
else
    # Basic connectivity via docker exec
    if pg_exec -c "SELECT 1" &>/dev/null; then
        pass "PostgreSQL is accessible inside container"

        # Check for core tables created by migrations
        CORE_TABLES=("agents" "locations" "routes" "structures" "ledger" "events" "discoveries"
                     "agent_snapshots" "world_snapshots" "simulation_runs" "operator_actions"
                     "social_constructs" "construct_memberships" "deception_records" "reputation_events")

        tables_found=0
        tables_missing=0
        for table in "${CORE_TABLES[@]}"; do
            exists=$(pg_exec -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='public' AND table_name='${table}'" 2>/dev/null)
            if [ "${exists}" = "1" ]; then
                tables_found=$((tables_found + 1))
            else
                tables_missing=$((tables_missing + 1))
                log_warn "  Missing table: ${table}"
            fi
        done

        TABLES_FOUND="${tables_found}/${#CORE_TABLES[@]}"

        if [ "${tables_missing}" -eq 0 ]; then
            pass "All ${#CORE_TABLES[@]} expected tables exist"
        elif [ "${tables_found}" -gt 0 ]; then
            warned "${tables_found}/${#CORE_TABLES[@]} tables found (${tables_missing} missing -- run migrations?)"
        else
            fail "No migration tables found -- migrations have not been run"
        fi

        # Check migrations tracking table
        migration_tracking=$(pg_exec -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='public' AND table_name='_migrations'" 2>/dev/null)
        if [ "${migration_tracking}" = "1" ]; then
            migrations_applied=$(pg_exec -tAc "SELECT COUNT(*) FROM _migrations" 2>/dev/null)
            pass "Migration tracking: ${migrations_applied} migrations recorded"
        else
            warned "No migration tracking table (_migrations) -- migrations may have been applied externally"
        fi
    else
        fail "Cannot connect to PostgreSQL inside container"
    fi
fi

# ---------------------------------------------------------------------------
# 3. Dragonfly (Redis) connectivity
# ---------------------------------------------------------------------------
log_header "Dragonfly (Hot State)"

pong=$(df_exec ping 2>/dev/null || echo "FAIL")
if [ "${pong}" = "PONG" ]; then
    pass "Dragonfly is accessible (container PING)"
else
    fail "Dragonfly did not respond to PING"
fi

# Check for agent state keys
agent_keys=$(df_exec keys "agent:*" 2>/dev/null | wc -l || echo "0")
AGENT_KEYS_COUNT="${agent_keys}"

if [ "${agent_keys}" -gt 0 ] 2>/dev/null; then
    pass "Found ${agent_keys} agent-related keys in Dragonfly"
else
    warned "No agent keys found in Dragonfly (simulation may not have started yet)"
fi

# Check for world tick key
world_tick=$(df_exec get "world:tick" 2>/dev/null || echo "")

if [ -n "${world_tick}" ] && [ "${world_tick}" != "(nil)" ]; then
    pass "Current world tick in Dragonfly: ${world_tick}"
    TICKS_PROCESSED="${world_tick}"
else
    warned "No world:tick key found (simulation may not have started yet)"
fi

# ---------------------------------------------------------------------------
# 4. NATS monitoring endpoint
# ---------------------------------------------------------------------------
log_header "NATS (Event Bus)"

nats_response=$(nats_health "http://localhost:8222/healthz" || echo "FAIL")
if [ "${nats_response}" != "FAIL" ]; then
    pass "NATS monitoring endpoint is responding (container)"
else
    fail "NATS monitoring endpoint not reachable inside container"
fi

# Get NATS connection count
nats_connz=$(nats_health "http://localhost:8222/connz" || echo "")
if [ -n "${nats_connz}" ]; then
    # Extract num_connections using grep/sed (avoid jq dependency)
    nats_conns=$(echo "${nats_connz}" | grep -o '"num_connections":[0-9]*' | grep -o '[0-9]*' || echo "0")
    if [ -n "${nats_conns}" ] && [ "${nats_conns}" -gt 0 ] 2>/dev/null; then
        pass "NATS has ${nats_conns} active connection(s)"
    else
        warned "NATS has 0 active connections (engine/runner may not be connected)"
    fi
fi

# ---------------------------------------------------------------------------
# 5. Observer API
# ---------------------------------------------------------------------------
log_header "Observer API"

observer_response=$(curl -sf -o /dev/null -w "%{http_code}" "http://localhost:${OBSERVER_PORT}/" 2>/dev/null || echo "000")
if [ "${observer_response}" = "200" ]; then
    pass "Observer API responding on port ${OBSERVER_PORT} (HTTP 200)"
else
    warned "Observer API on port ${OBSERVER_PORT} returned HTTP ${observer_response} (engine may still be initializing)"
fi

# Try the operator status endpoint
operator_status=$(curl -sf "http://localhost:${OBSERVER_PORT}/api/operator/status" 2>/dev/null || echo "")
if [ -n "${operator_status}" ]; then
    pass "Operator status endpoint is responding"
    log_info "  Response: ${operator_status}"
else
    warned "Operator status endpoint not responding (observer may not be started)"
fi

# ---------------------------------------------------------------------------
# 6. Event store activity
# ---------------------------------------------------------------------------
log_header "Event Store Activity"

if [ -n "${PG_PASS}" ]; then
    event_count=$(pg_exec -tAc "SELECT COUNT(*) FROM events" 2>/dev/null || echo "ERROR")

    if [ "${event_count}" = "ERROR" ]; then
        fail "Cannot query events table"
    elif [ "${event_count}" -gt 0 ] 2>/dev/null; then
        EVENTS_COUNT="${event_count}"
        pass "Events in store: ${event_count}"

        # Get recent events breakdown
        recent_events=$(pg_exec -tAc "SELECT event_type || ': ' || COUNT(*) FROM events GROUP BY event_type ORDER BY COUNT(*) DESC LIMIT 5" 2>/dev/null || echo "")
        if [ -n "${recent_events}" ]; then
            log_info "  Event type breakdown (top 5):"
            while IFS= read -r line; do
                [ -n "${line}" ] && log_info "    ${line}"
            done <<< "${recent_events}"
        fi

        # Get latest tick with events
        latest_tick=$(pg_exec -tAc "SELECT MAX(tick) FROM events" 2>/dev/null || echo "")
        if [ -n "${latest_tick}" ] && [ "${latest_tick}" != "" ]; then
            TICKS_PROCESSED="${latest_tick}"
            log_info "  Latest event tick: ${latest_tick}"
        fi
    else
        EVENTS_COUNT="0"
        warned "No events in store (simulation may not have completed any ticks yet)"
    fi

    # Check agent count in database
    agent_count=$(pg_exec -tAc "SELECT COUNT(*) FROM agents" 2>/dev/null || echo "ERROR")
    alive_count=$(pg_exec -tAc "SELECT COUNT(*) FROM agents WHERE died_at_tick IS NULL" 2>/dev/null || echo "ERROR")

    if [ "${agent_count}" != "ERROR" ] && [ "${agent_count}" -gt 0 ] 2>/dev/null; then
        AGENTS_ALIVE="${alive_count} alive / ${agent_count} total"
        pass "Agents in database: ${agent_count} total, ${alive_count} alive"
    else
        warned "No agents found in database (agents may only exist in Dragonfly)"
    fi

    # Check ledger entries
    ledger_count=$(pg_exec -tAc "SELECT COUNT(*) FROM ledger" 2>/dev/null || echo "ERROR")
    if [ "${ledger_count}" != "ERROR" ] && [ "${ledger_count}" -gt 0 ] 2>/dev/null; then
        log_info "  Ledger entries: ${ledger_count}"
    fi
fi

# ---------------------------------------------------------------------------
# 7. Summary
# ---------------------------------------------------------------------------
log_header "Validation Summary"

echo -e "${BOLD}Containers:${NC}     ${CONTAINERS_UP}/${CONTAINERS_TOTAL} running"
echo -e "${BOLD}Tables:${NC}         ${TABLES_FOUND}"
echo -e "${BOLD}Events:${NC}         ${EVENTS_COUNT}"
echo -e "${BOLD}Agents:${NC}         ${AGENTS_ALIVE}"
echo -e "${BOLD}Ticks:${NC}          ${TICKS_PROCESSED}"
echo -e "${BOLD}Agent keys:${NC}     ${AGENT_KEYS_COUNT} in Dragonfly"
echo ""
echo -e "${BOLD}Checks:${NC}         ${GREEN}${CHECKS_PASSED} passed${NC}, ${RED}${CHECKS_FAILED} failed${NC}, ${YELLOW}${CHECKS_WARNED} warnings${NC}"
echo ""

if [ "${CHECKS_FAILED}" -gt 0 ]; then
    error "Validation failed with ${CHECKS_FAILED} error(s)"
    exit 1
else
    if [ "${CHECKS_WARNED}" -gt 0 ]; then
        warn "Validation passed with ${CHECKS_WARNED} warning(s)"
    else
        success "All checks passed"
    fi
    exit 0
fi
