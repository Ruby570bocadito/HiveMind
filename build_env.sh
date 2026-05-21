#!/bin/bash
# Swarm build helper — sets OpenSSL env vars automatically.
# Usage: source build_env.sh && cargo test --workspace

export OPENSSL_DIR=/usr
export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include

echo "OpenSSL: ${OPENSSL_DIR} (lib: ${OPENSSL_LIB_DIR}, include: ${OPENSSL_INCLUDE_DIR})"
