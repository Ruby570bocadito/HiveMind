# Hive Colony v3.0 — Multi-Agent Framework
# Build + runtime en un solo Dockerfile multi-stage.

FROM rust:1.80-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /hive
COPY . .

ENV OPENSSL_DIR=/usr
RUN cargo build --release --workspace

# ── Runtime ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates openssh-client curl python3 python3-pip \
    && rm -rf /var/lib/apt/lists/* && \
    pip3 install --no-cache-dir flask requests

WORKDIR /hive

COPY --from=builder /hive/target/release/worker    /hive/bin/
COPY --from=builder /hive/target/release/drone     /hive/bin/
COPY --from=builder /hive/target/release/honeybee  /hive/bin/
COPY --from=builder /hive/target/release/weaver    /hive/bin/
COPY --from=builder /hive/target/release/queen     /hive/bin/
COPY --from=builder /hive/target/release/stinger   /hive/bin/
COPY --from=builder /hive/target/release/beekeeper /hive/bin/
COPY --from=builder /hive/target/release/buzz      /hive/bin/
COPY --from=builder /hive/target/release/swarm     /hive/bin/

COPY tests/ /hive/tests/
COPY scripts/ /hive/scripts/

EXPOSE 8080 8443
CMD ["python3", "/hive/tests/c2_server.py", "--port", "8443"]
