# 编码规范与验证命令

## 验证命令矩阵

### Rust（根 workspace，不含 pp-client-ui）
cargo build --workspace
cargo test -p <受影响 crate>
cargo clippy -p <受影响 crate> --all-targets -- -D warnings
cargo fmt --all

### Tauri 命令层（apps/desktop/src-tauri，独立 cargo workspace）
cd apps/desktop/src-tauri && cargo test
cd apps/desktop/src-tauri && cargo clippy --all-targets -- -D warnings
cd apps/desktop/src-tauri && cargo check

### 前端（apps/desktop）
cd apps/desktop && bun run verify   # = tsc 构建 + oxlint + prettier/oxfmt 检查

## 提交规范
见 AGENTS.md §5：原子化提交，type(scope): subject，一个提交一个逻辑单元，禁止 git add -A。
