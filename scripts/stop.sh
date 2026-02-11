#!/usr/bin/env bash
# =============================================================================
# Emergence -- Stop
# =============================================================================
# Cleanly stops all Emergence simulation services.
#
# Usage:
#   ./scripts/stop.sh              # Stop all services, keep volumes
#   ./scripts/stop.sh --volumes    # Stop all services and remove volumes
#   ./scripts/stop.sh --help       # Show this help
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
REMOVE_VOLUMES=false

for arg in "$@"; do
    case "$arg" in
        --volumes|-v)
            REMOVE_VOLUMES=true
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --volumes, -v  Also remove persistent data volumes"
            echo "  --help, -h     Show this help message"
            echo ""
            echo "Stops all Emergence Docker services. By default, data"
            echo "volumes (Dragonfly, PostgreSQL, NATS) are preserved so"
            echo "the simulation can resume from where it left off."
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
# Stop services
# ---------------------------------------------------------------------------
header "Stopping Emergence Simulation"

if [ "${REMOVE_VOLUMES}" = true ]; then
    warn "Removing persistent data volumes (simulation data will be lost)"
    docker compose -f "${PROJECT_ROOT}/docker-compose.yml" down -v
    success "All services stopped and volumes removed"
else
    docker compose -f "${PROJECT_ROOT}/docker-compose.yml" down
    success "All services stopped (volumes preserved)"
    info "To also remove data volumes, run: $0 --volumes"
fi
