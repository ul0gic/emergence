#!/usr/bin/env bash
# =============================================================================
# Emergence -- First Run
# =============================================================================
# Orchestrates the full startup sequence for the Emergence simulation:
#   1. Check prerequisites (Docker, docker compose, .env)
#   2. Build Docker images
#   3. Start infrastructure (Dragonfly, PostgreSQL, NATS)
#   4. Wait for infrastructure health
#   5. Run database migrations
#   6. Start application services (Engine, Runner)
#   7. Tail application logs
#
# Usage:
#   ./scripts/first-run.sh              # Full startup + tail logs
#   ./scripts/first-run.sh --no-tail    # Start everything, exit after startup
#   ./scripts/first-run.sh --skip-build # Skip Docker image build step
#   ./scripts/first-run.sh --help       # Show this help
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Resolve project root (parent of this script's directory)
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
NC='\033[0m' # No Color

info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*"; }
header()  { echo -e "\n${BOLD}${CYAN}=== $* ===${NC}\n"; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
TAIL_LOGS=true
SKIP_BUILD=false

for arg in "$@"; do
    case "$arg" in
        --no-tail)
            TAIL_LOGS=false
            ;;
        --skip-build)
            SKIP_BUILD=true
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --no-tail      Start services but do not tail logs"
            echo "  --skip-build   Skip Docker image build step"
            echo "  --help, -h     Show this help message"
            echo ""
            echo "This script performs the full startup sequence for the"
            echo "Emergence simulation, including infrastructure, migrations,"
            echo "and application services."
            exit 0
            ;;
        *)
            error "Unknown option: $arg"
            echo "Use --help for usage information."
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# 1. Check prerequisites
# ---------------------------------------------------------------------------
header "Checking Prerequisites"

# Docker
if ! command -v docker &>/dev/null; then
    error "Docker is not installed or not in PATH."
    error "Install Docker: https://docs.docker.com/get-docker/"
    exit 1
fi
success "Docker found: $(docker --version)"

# docker compose (v2 plugin)
if docker compose version &>/dev/null; then
    success "Docker Compose found: $(docker compose version --short)"
else
    error "Docker Compose (v2 plugin) is not available."
    error "Install: https://docs.docker.com/compose/install/"
    exit 1
fi

# Docker daemon running
if ! docker info &>/dev/null; then
    error "Docker daemon is not running."
    error "Start Docker and try again."
    exit 1
fi
success "Docker daemon is running"

# .env file
ENV_FILE="${PROJECT_ROOT}/.env"
ENV_EXAMPLE="${PROJECT_ROOT}/.env.example"

if [ ! -f "${ENV_FILE}" ]; then
    if [ -f "${ENV_EXAMPLE}" ]; then
        warn ".env file not found. Copying from .env.example..."
        warn "You MUST edit .env and set your API keys before the simulation will work."
        cp "${ENV_EXAMPLE}" "${ENV_FILE}"
    else
        error ".env file not found and .env.example is missing."
        error "Cannot proceed without environment configuration."
        exit 1
    fi
fi
success ".env file exists"

# Source .env to read values (handle lines with export prefix and comments)
set -a
# shellcheck disable=SC1090
source <(grep -v '^\s*#' "${ENV_FILE}" | grep -v '^\s*$' | sed 's/^export //')
set +a

# Check LLM API key
if [ -z "${LLM_DEFAULT_API_KEY:-}" ]; then
    error "LLM_DEFAULT_API_KEY is not set in .env"
    error "An OpenRouter API key is required for agent decision-making."
    error "Get one at: https://openrouter.ai/keys"
    exit 1
fi
success "LLM_DEFAULT_API_KEY is set"

# Check POSTGRES_PASSWORD
if [ -z "${POSTGRES_PASSWORD:-}" ]; then
    error "POSTGRES_PASSWORD is not set in .env"
    error "Set a password for the PostgreSQL database."
    exit 1
fi
success "POSTGRES_PASSWORD is set"

# ---------------------------------------------------------------------------
# 2. Build Docker images
# ---------------------------------------------------------------------------
if [ "${SKIP_BUILD}" = true ]; then
    info "Skipping Docker image build (--skip-build)"
else
    header "Building Docker Images"
    info "This may take several minutes on the first run (Rust compilation)..."
    if ! docker compose -f "${PROJECT_ROOT}/docker-compose.yml" build; then
        error "Docker image build failed."
        error "Check the build output above for compilation errors."
        exit 1
    fi
    success "Docker images built"
fi

# ---------------------------------------------------------------------------
# 3. Start infrastructure services
# ---------------------------------------------------------------------------
header "Starting Infrastructure"

info "Starting Dragonfly, PostgreSQL, and NATS..."
docker compose -f "${PROJECT_ROOT}/docker-compose.yml" up -d dragonfly postgres nats

# ---------------------------------------------------------------------------
# 4. Wait for infrastructure health
# ---------------------------------------------------------------------------
header "Waiting for Infrastructure Health"

MAX_WAIT=60
POLL_INTERVAL=2

wait_for_healthy() {
    local service="$1"
    local container="$2"
    local elapsed=0

    while [ $elapsed -lt $MAX_WAIT ]; do
        local health
        health=$(docker inspect --format='{{.State.Health.Status}}' "${container}" 2>/dev/null || echo "not_found")

        case "${health}" in
            healthy)
                success "${service} is healthy"
                return 0
                ;;
            unhealthy)
                error "${service} is unhealthy"
                docker logs --tail 20 "${container}" 2>&1 || true
                return 1
                ;;
            not_found)
                # Container may not exist yet
                ;;
        esac

        sleep "${POLL_INTERVAL}"
        elapsed=$((elapsed + POLL_INTERVAL))
        info "Waiting for ${service}... (${elapsed}s / ${MAX_WAIT}s)"
    done

    error "${service} did not become healthy within ${MAX_WAIT}s"
    docker logs --tail 20 "${container}" 2>&1 || true
    return 1
}

wait_for_healthy "Dragonfly" "emergence-dragonfly"
wait_for_healthy "PostgreSQL" "emergence-postgres"

# NATS health: check via docker exec wget inside the container.
NATS_HEALTHY=false
NATS_ELAPSED=0
while [ $NATS_ELAPSED -lt $MAX_WAIT ]; do
    if docker exec emergence-nats wget --spider --quiet "http://localhost:8222/healthz" 2>/dev/null; then
        NATS_HEALTHY=true
        break
    fi
    sleep "${POLL_INTERVAL}"
    NATS_ELAPSED=$((NATS_ELAPSED + POLL_INTERVAL))
    info "Waiting for NATS... (${NATS_ELAPSED}s / ${MAX_WAIT}s)"
done

if [ "${NATS_HEALTHY}" = true ]; then
    success "NATS is healthy"
else
    error "NATS did not become healthy within ${MAX_WAIT}s"
    docker logs --tail 20 emergence-nats 2>&1 || true
    exit 1
fi

# ---------------------------------------------------------------------------
# 5. Run database migrations
# ---------------------------------------------------------------------------
header "Running Database Migrations"

MIGRATIONS_DIR="${PROJECT_ROOT}/crates/emergence-db/migrations"
PG_CONTAINER="emergence-postgres"
PG_DB="${POSTGRES_DB:-emergence}"
PG_USER="${POSTGRES_USER:-emergence}"

# Helper: run psql inside the postgres container (no host port needed).
pg_exec() {
    docker exec -e PGPASSWORD="${POSTGRES_PASSWORD}" "${PG_CONTAINER}" \
        psql -U "${PG_USER}" -d "${PG_DB}" "$@"
}

if [ ! -d "${MIGRATIONS_DIR}" ]; then
    error "Migrations directory not found: ${MIGRATIONS_DIR}"
    exit 1
fi

# Count migration files
MIGRATION_COUNT=$(find "${MIGRATIONS_DIR}" -name '*.sql' -type f | wc -l)
info "Found ${MIGRATION_COUNT} migration files"

# Create a tracking table if it does not exist so we can skip already-applied
# migrations on subsequent runs.
pg_exec -q -c "
    CREATE TABLE IF NOT EXISTS _migrations (
        filename TEXT PRIMARY KEY,
        applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );
" 2>/dev/null || {
    error "Could not connect to PostgreSQL inside container."
    error "Is the ${PG_CONTAINER} container running and healthy?"
    exit 1
}

# Run each migration in sorted order, skipping already-applied ones.
APPLIED=0
SKIPPED=0
FAILED=0

for migration in $(find "${MIGRATIONS_DIR}" -name '*.sql' -type f | sort); do
    filename="$(basename "${migration}")"

    # Check if already applied
    already_applied=$(pg_exec -tAc "SELECT COUNT(*) FROM _migrations WHERE filename = '${filename}'" 2>/dev/null)

    if [ "${already_applied}" = "1" ]; then
        info "Skipping (already applied): ${filename}"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    info "Applying: ${filename}"

    # Pipe the migration SQL into the container via stdin.
    # Migration 0008 uses ALTER TYPE ... ADD VALUE which cannot run inside a
    # transaction block. We detect this and run it without a transaction wrapper.
    if grep -q "ALTER TYPE.*ADD VALUE" "${migration}" 2>/dev/null; then
        # Run outside of a transaction
        if docker exec -i -e PGPASSWORD="${POSTGRES_PASSWORD}" "${PG_CONTAINER}" \
            psql -U "${PG_USER}" -d "${PG_DB}" \
            -v ON_ERROR_STOP=1 < "${migration}" 2>&1; then
            APPLIED=$((APPLIED + 1))
            success "Applied: ${filename}"
        else
            error "Failed to apply: ${filename}"
            FAILED=$((FAILED + 1))
            exit 1
        fi
    else
        # Run inside a transaction for atomicity
        if docker exec -i -e PGPASSWORD="${POSTGRES_PASSWORD}" "${PG_CONTAINER}" \
            psql -U "${PG_USER}" -d "${PG_DB}" \
            -v ON_ERROR_STOP=1 -1 < "${migration}" 2>&1; then
            APPLIED=$((APPLIED + 1))
            success "Applied: ${filename}"
        else
            error "Failed to apply: ${filename}"
            FAILED=$((FAILED + 1))
            exit 1
        fi
    fi

    # Record successful migration
    pg_exec -q -c "INSERT INTO _migrations (filename) VALUES ('${filename}')" 2>/dev/null
done

info "Migrations complete: ${APPLIED} applied, ${SKIPPED} skipped, ${FAILED} failed"

# ---------------------------------------------------------------------------
# 6. Start application services
# ---------------------------------------------------------------------------
header "Starting Application Services"

info "Starting emergence-engine and emergence-runner..."
docker compose -f "${PROJECT_ROOT}/docker-compose.yml" up -d emergence-engine emergence-runner

# Give services a moment to initialize
sleep 3

# Quick check that containers are running
ENGINE_STATUS=$(docker inspect --format='{{.State.Status}}' emergence-engine 2>/dev/null || echo "not_found")
RUNNER_STATUS=$(docker inspect --format='{{.State.Status}}' emergence-runner 2>/dev/null || echo "not_found")

if [ "${ENGINE_STATUS}" = "running" ]; then
    success "emergence-engine is running"
else
    error "emergence-engine is not running (status: ${ENGINE_STATUS})"
    warn "Checking logs..."
    docker logs --tail 30 emergence-engine 2>&1 || true
    exit 1
fi

if [ "${RUNNER_STATUS}" = "running" ]; then
    success "emergence-runner is running"
else
    error "emergence-runner is not running (status: ${RUNNER_STATUS})"
    warn "Checking logs..."
    docker logs --tail 30 emergence-runner 2>&1 || true
    exit 1
fi

# ---------------------------------------------------------------------------
# 7. Startup summary
# ---------------------------------------------------------------------------
header "Emergence Simulation Started"

OBSERVER_PORT="${OBSERVER_PORT:-8080}"

echo -e "${BOLD}Services:${NC}"
echo -e "  Dragonfly (hot state)   ${GREEN}running${NC}  (internal only)"
echo -e "  PostgreSQL (cold state) ${GREEN}running${NC}  (internal only)"
echo -e "  NATS (event bus)        ${GREEN}running${NC}  (internal only)"
echo -e "  World Engine            ${GREEN}running${NC}  localhost:${OBSERVER_PORT}"
echo -e "  Agent Runner            ${GREEN}running${NC}"
echo ""
echo -e "${BOLD}Exposed Endpoint:${NC}"
echo -e "  Observer API:   http://localhost:${OBSERVER_PORT}/"
echo ""
echo -e "${BOLD}Quick commands:${NC}"
echo -e "  Validate:  ${CYAN}./scripts/validate.sh${NC}"
echo -e "  Stop:      ${CYAN}./scripts/stop.sh${NC}"
echo -e "  Reset:     ${CYAN}./scripts/reset.sh${NC}"
echo -e "  Logs:      ${CYAN}docker compose logs -f emergence-engine emergence-runner${NC}"
echo ""

# ---------------------------------------------------------------------------
# 8. Tail logs (unless --no-tail)
# ---------------------------------------------------------------------------
if [ "${TAIL_LOGS}" = true ]; then
    info "Tailing engine and runner logs (Ctrl+C to detach)..."
    echo ""
    docker compose -f "${PROJECT_ROOT}/docker-compose.yml" logs -f emergence-engine emergence-runner
fi
