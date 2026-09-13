#!/usr/bin/env bash
# Rust 全量门禁（提交前 / 推送前自查，对齐 CI 并补齐 CI 覆盖不到的盲区）：
#
#   1. cargo fmt --all --check
#   2. cargo clippy --workspace --all-targets -- -D warnings   （CI 同款）
#   3. cargo test --workspace                                  （CI 同款）
#   4. desktop 壳 cargo clippy --all-targets（apps/desktop/src-tauri 为独立 cargo 项目，
#      不在根 workspace，根目录 clippy/test 永远覆盖不到）
#   5. mobile 壳 host cargo clippy --all-targets（同上；Android 专属代码经 cfg 裁剪）
#   6. mobile 壳 aarch64-linux-android cargo check（装有 NDK + rust target 时执行，
#      覆盖 host 编译不可见的 #[cfg(target_os = "android")] 代码路径；
#      历史上两次「host 全绿但 Android 编译失败」均栽在这条盲区上）
#
# 用法：bash scripts/check-rust-gates.sh
# 可选：PP_RUST_GATE_FAST=1 仅做编译检查（1/2/4/5/6，跳过测试，pre-commit 用）。
set -euo pipefail
cd "$(dirname "$0")/.."

FAST="${PP_RUST_GATE_FAST:-0}"
step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }

step "1/6 cargo fmt --all --check"
cargo fmt --all --check

step "2/6 cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

if [[ "$FAST" != "1" ]]; then
    step "3/6 cargo test --workspace"
    cargo test --workspace
else
    step "3/6 cargo test --workspace（PP_RUST_GATE_FAST=1，跳过）"
fi

step "4/6 desktop 壳 cargo clippy（apps/desktop/src-tauri）"
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings

step "5/6 mobile 壳 host cargo clippy（apps/mobile/src-tauri）"
cargo clippy --manifest-path apps/mobile/src-tauri/Cargo.toml --all-targets -- -D warnings

# Android 交叉编译检查：需要 NDK linker 配置（apps/mobile/src-tauri/.cargo/config.toml
# 按工作目录层级生效，故 cd 进入执行）与 rust 目标。
if rustup target list --installed 2>/dev/null | grep -q '^aarch64-linux-android$' \
    && [[ -n "${ANDROID_HOME:-}" || -n "${ANDROID_SDK_ROOT:-}" ]]; then
    step "6/6 mobile 壳 cargo check --target aarch64-linux-android"
    (cd apps/mobile/src-tauri && cargo check --target aarch64-linux-android)
else
    step "6/6 Android target 检查跳过（缺 aarch64-linux-android target 或 ANDROID_HOME）"
fi

printf '\n\033[1;32mRust 门禁全部通过。\033[0m\n'
