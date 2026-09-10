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
#   ./hive.sh clean          Remove build artifacts
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

C2_PORT="${C2_PORT:-8444}"
C2_HOST="${C2_HOST:-localhost}"

banner() {
    printf '\033[1;35m%s\033[0m\n' "🐝 Hive Colony v3.0 — $1"
}

need_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "error: cargo not found. Install Rust: https://rustup.rs" >&2
        exit 1
    fi
}

build()      { need_cargo; banner "building workspace"; cargo build --workspace "$@"; }
release()    { need_cargo; banner "building agents (release)"; cargo build --release -p worker -p drone -p honeybee -p weaver -p queen "$@"; }
test_all()   { need_cargo; banner "running tests"; cargo test -p hive_base -- --test-threads=2 "$@"; }

start_c2() {
    need_cargo
    banner "starting C2 server on http://${C2_HOST}:${C2_PORT}"
    cargo run --release -p c2-server -- --port "${C2_PORT}"
}

start_tui() {
    need_cargo
    banner "starting Beekeeper TUI"
    cargo run --release -p beekeeper
}

start_colony() {
    banner "starting full stack (docker compose)"
    docker compose up --build -d
    docker compose ps
}

start_lab() {
    banner "starting SSH lab (docker compose -f docker-compose.lab.yml)"
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
    clean)  shift; clean "$@" ;;
    help|*)
        sed -n '2,15p' "$0"
        ;;
esac
