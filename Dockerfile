# Swarm Multi-Agent Framework - Docker Environment
# Complete build + C2 + lab in one docker-compose stack.

FROM rust:1.80-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev python3 python3-pip curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /swarm
COPY . .

# Build all agents
ENV OPENSSL_DIR=/usr
RUN cargo build --release --workspace

# ── Runtime image ─────────────────────────────────────────────────────────

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates openssh-client nmap python3 curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /swarm

# Copy compiled binaries
COPY --from=builder /swarm/target/release/scout /swarm/bin/scout
COPY --from=builder /swarm/target/release/shaper /swarm/bin/shaper
COPY --from=builder /swarm/target/release/hoarder /swarm/bin/hoarder
COPY --from=builder /swarm/target/release/weaver /swarm/bin/weaver
COPY --from=builder /swarm/target/release/overmind /swarm/bin/overmind
COPY --from=builder /swarm/target/release/swarmctl /swarm/bin/swarmctl
COPY --from=builder /swarm/target/release/dropper /swarm/bin/dropper

# Copy test/monitoring tools
COPY tests/ /swarm/tests/
COPY training/ /swarm/training/

# Default: launch C2 dashboard
EXPOSE 8080 8443
CMD ["python3", "/swarm/tests/c2_server.py", "--port", "8443"]
