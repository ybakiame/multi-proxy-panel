# 编码规范与验证命令

## 验证命令矩阵

### Rust（根 workspace，不含客户端壳 `apps/client/src-tauri`）

```bash
cargo build --workspace
cargo test -p <受影响 crate>
cargo clippy -p <受影响 crate> --all-targets -- -D warnings
cargo fmt --all
```

跨 crate 变更、壳层或 Android 相关变更，用全量门禁（含客户端壳 host clippy 与
`aarch64-linux-android` 交叉检查）：

```bash
bun run verify:rust        # scripts/check-rust-gates.sh
bun run verify:rust:fast   # 跳过测试，pre-commit 同款
```

### 客户端壳（`apps/client/src-tauri`，独立 cargo 项目）

```bash
cd apps/client/src-tauri && cargo test
cd apps/client/src-tauri && cargo clippy --all-targets -- -D warnings
cd apps/client/src-tauri && cargo check
cd apps/client/src-tauri && cargo check --target aarch64-linux-android   # 需 NDK / rust target
```

### 前端（依赖在仓库根目录安装，单一 `bun.lock`）

```bash
bun install

bun run --filter pp-web verify            # apps/panel：构建 + oxlint + oxfmt
bun run --filter pp-client-app verify     # apps/client：desktop + android 双 mode 构建 + lint + 格式
bun run --filter @pp/client-core verify   # packages/client-core：typecheck + lint + 格式
```

> 前端目前没有单元测试运行器，`verify` 即自动化边界（见 `AGENTS.md` §7）。

## 提交规范

见 `AGENTS.md` §5：原子化提交，`type(scope): subject`，一个提交一个逻辑单元，禁止 `git add -A`。
完成定义与停止条件见 `AGENTS.md` §11。
