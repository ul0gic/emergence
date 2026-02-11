#!/usr/bin/env bash
# =============================================================================
# Emergence -- Clean Reset
# =============================================================================
# Performs a full clean reset for a fresh simulation run:
#   1. Stops all Docker services
#   2. Removes Docker volumes (Dragonfly, PostgreSQL, NATS data)
#   3. Optionally removes Docker images
#
# Usage:
#   ./scripts/reset.sh              # Stop + remove volumes
#   ./scripts/reset.sh --images     # Also remove Docker images (full rebuild)
#   ./scripts/reset.sh --yes        # Skip confirmation prompt
#   ./scripts/reset.sh --help       # Show this help
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
error()   { echo -e "${RED}[ERROR]${NC} $*"; }
header()  { echo -e "\n${BOLD}${CYAN}=== $* ===${NC}\n"; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
REMOVE_IMAGES=false
SKIP_CONFIRM=false

for arg in "$@"; do
    case "$arg" in
        --images)
            REMOVE_IMAGES=true
            ;;
        --yes|-y)
            SKIP_CONFIRM=true
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --images       Also remove Docker images (requires full rebuild)"
            echo "  --yes, -y      Skip confirmation prompt"
            echo "  --help, -h     Show this help message"
            echo ""
            echo "Performs a destructive reset of all Emergence simulation data."
            echo "This removes all simulation history, agent state, events, and"
            echo "ledger entries. After reset, run first-run.sh to start fresh."
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
# Confirmation
# ---------------------------------------------------------------------------
header "Emergence Clean Reset"

warn "This will PERMANENTLY DELETE all simulation data:"
echo ""
echo -e "  ${RED}Dragonfly${NC}   All hot state (agent vitals, tick queues, world state)"
echo -e "  ${RED}PostgreSQL${NC}  All events, ledger entries, agent records, snapshots"
echo -e "  ${RED}NATS${NC}        All JetStream data and message history"

if [ "${REMOVE_IMAGES}" = true ]; then
    echo -e "  ${RED}Images${NC}      Docker images will be removed (requires full rebuild)"
fi

echo ""

if [ "${SKIP_CONFIRM}" = false ]; then
    read -rp "$(echo -e "${YELLOW}Are you sure? Type 'reset' to confirm: ${NC}")" confirmation
    if [ "${confirmation}" != "reset" ]; then
        info "Reset cancelled."
        exit 0
    fi
    echo ""
fi

# ---------------------------------------------------------------------------
# 1. Stop all services
# ---------------------------------------------------------------------------
header "Stopping Services"

info "Stopping all Emergence containers..."
docker compose -f "${PROJECT_ROOT}/docker-compose.yml" down 2>/dev/null || true
success "Services stopped"

# ---------------------------------------------------------------------------
# 2. Remove Docker volumes
# ---------------------------------------------------------------------------
header "Removing Data Volumes"

VOLUMES=(
    "emergence_dragonfly_data"
    "emergence_postgres_data"
    "emergence_nats_data"
)

for volume in "${VOLUMES[@]}"; do
    if docker volume inspect "${volume}" &>/dev/null; then
        docker volume rm "${volume}"
        success "Removed volume: ${volume}"
    else
        info "Volume not found (already removed?): ${volume}"
    fi
done

# ---------------------------------------------------------------------------
# 3. Remove Docker images (optional)
# ---------------------------------------------------------------------------
if [ "${REMOVE_IMAGES}" = true ]; then
    header "Removing Docker Images"

    # Get the compose project name (defaults to directory name)
    COMPOSE_PROJECT="${COMPOSE_PROJECT_NAME:-emergence}"

    # Find and remove project images built by docker compose
    # These follow the pattern: <project>-<service>
    COMPOSE_IMAGES=$(docker images --format '{{.Repository}}:{{.Tag}}' | grep "^${COMPOSE_PROJECT}-" 2>/dev/null || true)

    if [ -n "${COMPOSE_IMAGES}" ]; then
        while IFS= read -r image; do
            docker rmi "${image}" 2>/dev/null && success "Removed image: ${image}" || warn "Could not remove image: ${image}"
        done <<< "${COMPOSE_IMAGES}"
    else
        info "No project images found to remove"
    fi

    # Also check the directory-based naming docker compose uses
    DIR_NAME=$(basename "${PROJECT_ROOT}")
    DIR_IMAGES=$(docker images --format '{{.Repository}}:{{.Tag}}' | grep "^${DIR_NAME}-" 2>/dev/null || true)

    if [ -n "${DIR_IMAGES}" ] && [ "${DIR_IMAGES}" != "${COMPOSE_IMAGES}" ]; then
        while IFS= read -r image; do
            docker rmi "${image}" 2>/dev/null && success "Removed image: ${image}" || warn "Could not remove image: ${image}"
        done <<< "${DIR_IMAGES}"
    fi

    success "Image cleanup complete"
fi

# ---------------------------------------------------------------------------
# 4. Summary
# ---------------------------------------------------------------------------
header "Reset Complete"

success "All simulation data has been removed."
echo ""
echo -e "To start a fresh simulation:"
echo -e "  ${CYAN}./scripts/first-run.sh${NC}"
echo ""
