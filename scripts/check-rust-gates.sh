#!/usr/bin/env bash
# ProxyPanel Rust 门禁：fmt、workspace clippy、workspace test。
# PP_RUST_GATE_FAST=1 跳过测试，供 pre-commit 使用；在 nix develop 内运行。
set -euo pipefail
cd "$(dirname "$0")/.."

FAST="${PP_RUST_GATE_FAST:-0}"
step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }

step "1/3 cargo fmt --all --check"
cargo fmt --all --check

step "2/3 cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

if [[ "$FAST" != "1" ]]; then
    step "3/3 cargo test --workspace"
    cargo test --workspace
else
    step "3/3 cargo test --workspace（PP_RUST_GATE_FAST=1，跳过）"
fi

printf '\n\033[1;32mRust 门禁全部通过。\033[0m\n'
