#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Hive Colony v3.0 — operator wrapper
#
# Usage:
#   ./hive.sh build          Compile the full workspace (debug profile)
#   ./hive.sh release        Compile agents in release mode (required by stinger)
#   ./hive.sh test           Run the hive_base test suite
#   ./hive.sh c2             Start the Rust C2 server on :8444
#   ./hive.sh tui            Start the Beekeeper operator TUI
#   ./hive.sh colony         Start the full stack via docker compose
#   ./hive.sh lab            Start the SSH lab targets via docker compose
#   ./hive.sh status         Quick health check of the C2 API
#   ./hive.sh doctor         Environment + config sanity check
#   ./hive.sh clean          Remove build artifacts
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

C2_PORT="${C2_PORT:-8444}"
C2_HOST="${C2_HOST:-localhost}"

banner() {
    printf '\033[1;35m%s\033[0m\n' "🐝 Hive Colony v3.0 — $1"
}

# Compose preflight (ronda 13): with rootless podman the compose plugin
# cannot reach unix:///run/user/$UID/podman/podman.sock until the socket
# unit is started — the raw error ("Cannot connect to the Docker daemon")
# gave no hint. Try to start it ourselves; if that is not possible, print
# the exact commands the operator needs.
ensure_compose_daemon() {
    if docker info >/dev/null 2>&1; then
        return 0
    fi
    if command -v podman >/dev/null 2>&1; then
        # Rootless podman: bring the socket up (systemd user unit).
        if command -v systemctl >/dev/null 2>&1 \
            && systemctl --user start podman.socket >/dev/null 2>&1; then
            if docker info >/dev/null 2>&1; then
                echo "  podman socket started — docker CLI emulation ready"
                return 0
            fi
        fi
        echo "error: the podman socket is not reachable." >&2
        echo "  start it with:   systemctl --user start podman.socket" >&2
        echo "  (and enable at boot: systemctl --user enable --now podman.socket)" >&2
        echo "  then re-run:     ./hive.sh $1" >&2
        exit 1
    fi
    echo "error: the docker daemon is not reachable." >&2
    echo "  start it with:   sudo systemctl start docker" >&2
    echo "  then re-run:     ./hive.sh $1" >&2
    exit 1
}

need_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "error: cargo not found. Install Rust: https://rustup.rs" >&2
        exit 1
    fi
}

build()      { need_cargo; banner "building workspace"; cargo build --workspace "$@"; }
release()    { need_cargo; banner "building agents (release)"; cargo build --release -p worker -p drone -p honeybee -p queen "$@"; }
test_all()   { need_cargo; banner "running tests (full workspace)"; cargo test --workspace "$@"; }

start_c2() {
    need_cargo
    banner "starting C2 server on http://${C2_HOST}:${C2_PORT}"
    echo "  web console on the same port → http://${C2_HOST}:${C2_PORT}/"
    cargo run --release -p c2-server -- --port "${C2_PORT}" "$@"
}

start_tui() {
    need_cargo
    banner "starting Beekeeper TUI"
    cargo run --release -p beekeeper
}

start_colony() {
    banner "starting full stack (docker compose)"
    ensure_compose_daemon colony
    docker compose up --build -d
    docker compose ps
    echo
    echo "  C2 web console → http://localhost:${C2_PORT}"
}

start_lab() {
    banner "starting SSH lab (docker compose -f docker-compose.lab.yml)"
    ensure_compose_daemon lab
    docker compose -f docker-compose.lab.yml up --build -d
    docker compose -f docker-compose.lab.yml ps
}

status() {
    banner "C2 health: http://${C2_HOST}:${C2_PORT}/health"
    curl -fsS "http://${C2_HOST}:${C2_PORT}/health" || {
        echo "C2 unreachable — is it running? try: ./hive.sh c2" >&2
        return 1
    }
    echo
}

doctor() {
    banner "environment doctor"
    local ok=0 fail=0
    pass() { printf '  \033[92m✓\033[0m %s\n' "$1"; ok=$((ok+1)); }
    miss() { printf '  \033[91m✗\033[0m %s\n' "$1"; fail=$((fail+1)); }

    if command -v cargo >/dev/null 2>&1; then
        pass "cargo: $(cargo --version 2>/dev/null || echo 'present')"
    else
        miss "cargo not found — install Rust: https://rustup.rs"
    fi

    if command -v docker >/dev/null 2>&1; then
        pass "docker: $(docker --version 2>/dev/null | cut -d, -f1 || echo present)"
        if docker compose version >/dev/null 2>&1; then
            pass "docker compose plugin available"
        else
            miss "docker compose plugin not available (needed by ./hive.sh colony|lab)"
        fi
    else
        miss "docker not found — only local cargo workflows will work"
    fi

    # Ronda 13: local runs mmap the arena in /dev/shm (~21 MB per arena).
    # Small container defaults (64 MB) plus leftover arenas from previous
    # runs exhaust the tmpfs and arena writers die with SIGBUS — a silent,
    # log-less crash that looks like a random agent freeze.
    if [ -d /dev/shm ]; then
        shm_free=$(df -BM --output=avail /dev/shm 2>/dev/null | tail -1 | tr -dc '0-9')
        if [ -n "${shm_free:-}" ] && [ "${shm_free}" -lt 48 ]; then
            miss "/dev/shm has only ${shm_free}M free — the arena needs ~21M per run; clean leftovers: rm -f /dev/shm/hive_*"
        else
            pass "/dev/shm: ${shm_free:-?}M free (arena needs ~21M)"
        fi
    fi

    if [ -f ./hive.toml ]; then
        if cargo run -q -p beekeeper -- config-check 2>&1; then
            pass "hive.toml parses correctly"
        else
            miss "hive.toml FAILED validation (see output above)"
        fi
    else
        miss "no hive.toml in this directory — agents will use compiled-in defaults"
    fi

    if curl -fsS "http://${C2_HOST}:${C2_PORT}/health" >/dev/null 2>&1; then
        pass "C2 responding on ${C2_HOST}:${C2_PORT}"
    else
        printf '  \033[33m•\033[0m C2 not running on %s:%s (start with ./hive.sh c2)\n' "${C2_HOST}" "${C2_PORT}"
    fi

    echo
    printf '  doctor summary: %d passed, %d missing\n' "${ok}" "${fail}"
    [ "${fail}" -eq 0 ]
}

clean() {
    need_cargo
    banner "cleaning build artifacts"
    cargo clean
}

case "${1:-help}" in
    build)  shift; build "$@" ;;
    release) shift; release "$@" ;;
    test)   shift; test_all "$@" ;;
    c2)     shift; start_c2 "$@" ;;
    tui)    shift; start_tui "$@" ;;
    colony) shift; start_colony "$@" ;;
    lab)    shift; start_lab "$@" ;;
    status) shift; status "$@" ;;
    doctor) shift; doctor "$@" ;;
    clean)  shift; clean "$@" ;;
    help|*)
        sed -n '2,15p' "$0"
        ;;
esac
