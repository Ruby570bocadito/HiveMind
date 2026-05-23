# Hive Colony v3.0 — Runtime-only image (pre-built binaries from host)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates openssh-client curl python3 python3-pip \
    && rm -rf /var/lib/apt/lists/* && \
    pip3 install --no-cache-dir --break-system-packages flask requests

WORKDIR /hive

COPY target/release/worker    /hive/bin/
COPY target/release/drone     /hive/bin/
COPY target/release/honeybee  /hive/bin/
COPY target/release/weaver    /hive/bin/
COPY target/release/queen     /hive/bin/
COPY target/release/stinger   /hive/bin/
COPY target/release/beekeeper /hive/bin/

COPY tests/ /hive/tests/
COPY scripts/ /hive/scripts/

EXPOSE 8080 8444
CMD ["python3", "/hive/tests/c2_server.py", "--port", "8444"]
