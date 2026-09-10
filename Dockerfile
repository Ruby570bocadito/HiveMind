# Hive Colony v3.0 — full build (multi-stage)
# Builds the workspace from source; no pre-built host binaries required.
FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/hive
COPY . .

# Build everything the runtime image needs (agents + c2 + tui)
RUN cargo build --release -p c2-server \
    -p queen -p worker -p drone -p honeybee -p weaver -p swarm \
    -p beekeeper

# ── Runtime stage ─────────────────────────────────────────────────────────
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates openssh-client openssh-server sshpass \
    curl python3 python3-pip \
    iputils-ping dnsutils netcat-openbsd nmap \
    && rm -rf /var/lib/apt/lists/* && \
    pip3 install --no-cache-dir --break-system-packages requests

WORKDIR /hive

COPY --from=builder /opt/hive/target/release/c2-server  /hive/bin/
COPY --from=builder /opt/hive/target/release/queen      /hive/bin/
COPY --from=builder /opt/hive/target/release/worker     /hive/bin/
COPY --from=builder /opt/hive/target/release/drone      /hive/bin/
COPY --from=builder /opt/hive/target/release/honeybee   /hive/bin/
COPY --from=builder /opt/hive/target/release/weaver     /hive/bin/
COPY --from=builder /opt/hive/target/release/swarm      /hive/bin/
COPY --from=builder /opt/hive/target/release/beekeeper  /hive/bin/

COPY tests/ /hive/tests/
COPY scripts/ /hive/scripts/
COPY hive.toml /hive/hive.toml

EXPOSE 8080 8444
CMD ["/hive/bin/c2-server", "--port", "8444", "--loot-dir", "/hive/loot", "--db-path", "/hive/c2.db"]
