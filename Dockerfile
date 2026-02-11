# =============================================================================
# Emergence — Multi-stage Dockerfile
# =============================================================================
# Builds both the World Engine (emergence-engine) and Agent Runner
# (emergence-runner) binaries in a single image. The docker-compose
# service selects which binary to run via the `command` directive.
#
# Stage 1 (builder): Compile the entire Rust workspace in release mode.
# Stage 2 (runtime): Minimal Debian image with just the binaries.
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1: Builder
# ---------------------------------------------------------------------------
FROM rust:1.88-slim AS builder

# Install build dependencies for native libraries (openssl, postgres client)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy the workspace manifest and lockfile first for dependency caching.
COPY Cargo.toml Cargo.lock ./

# Copy all crate manifests so cargo can resolve the workspace.
COPY crates/emergence-types/Cargo.toml crates/emergence-types/Cargo.toml
COPY crates/emergence-core/Cargo.toml crates/emergence-core/Cargo.toml
COPY crates/emergence-db/Cargo.toml crates/emergence-db/Cargo.toml
COPY crates/emergence-world/Cargo.toml crates/emergence-world/Cargo.toml
COPY crates/emergence-agents/Cargo.toml crates/emergence-agents/Cargo.toml
COPY crates/emergence-ledger/Cargo.toml crates/emergence-ledger/Cargo.toml
COPY crates/emergence-events/Cargo.toml crates/emergence-events/Cargo.toml
COPY crates/emergence-observer/Cargo.toml crates/emergence-observer/Cargo.toml
COPY crates/emergence-engine/Cargo.toml crates/emergence-engine/Cargo.toml
COPY crates/emergence-runner/Cargo.toml crates/emergence-runner/Cargo.toml

# Create stub source files so cargo can build dependencies in a cached layer.
# The actual source is copied in the next step; this trick means dependency
# re-downloads are only triggered by Cargo.toml / Cargo.lock changes.
RUN mkdir -p crates/emergence-types/src && echo '//! stub' > crates/emergence-types/src/lib.rs && \
    mkdir -p crates/emergence-core/src && echo '//! stub' > crates/emergence-core/src/lib.rs && \
    mkdir -p crates/emergence-db/src && echo '//! stub' > crates/emergence-db/src/lib.rs && \
    mkdir -p crates/emergence-db/migrations && \
    mkdir -p crates/emergence-world/src && echo '//! stub' > crates/emergence-world/src/lib.rs && \
    mkdir -p crates/emergence-agents/src && echo '//! stub' > crates/emergence-agents/src/lib.rs && \
    mkdir -p crates/emergence-ledger/src && echo '//! stub' > crates/emergence-ledger/src/lib.rs && \
    mkdir -p crates/emergence-events/src && echo '//! stub' > crates/emergence-events/src/lib.rs && \
    mkdir -p crates/emergence-observer/src && echo '//! stub' > crates/emergence-observer/src/lib.rs && \
    mkdir -p crates/emergence-engine/src && echo 'fn main() {}' > crates/emergence-engine/src/main.rs && \
    mkdir -p crates/emergence-runner/src && echo 'fn main() {}' > crates/emergence-runner/src/main.rs

# Pre-build dependencies (this layer is cached unless Cargo.toml/lock change).
RUN cargo build --release 2>/dev/null || true

# Now copy the real source code.
COPY crates/ crates/

# Touch source files to ensure cargo detects the change and rebuilds.
RUN find crates -name '*.rs' -exec touch {} +

# Build the release binaries.
RUN cargo build --release --bin emergence-engine --bin emergence-runner

# ---------------------------------------------------------------------------
# Stage 2: Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install minimal runtime dependencies.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries from the builder stage.
COPY --from=builder /build/target/release/emergence-engine /usr/local/bin/emergence-engine
COPY --from=builder /build/target/release/emergence-runner /usr/local/bin/emergence-runner

# Copy templates used by the agent runner for LLM prompt generation.
COPY templates/ /app/templates/

# Copy migrations so the engine can run them at startup.
COPY crates/emergence-db/migrations/ /app/migrations/

# Non-root user for security.
RUN groupadd -r emergence && useradd -r -g emergence emergence
USER emergence

# Default to the engine binary; docker-compose overrides with `command`.
CMD ["/usr/local/bin/emergence-engine"]
