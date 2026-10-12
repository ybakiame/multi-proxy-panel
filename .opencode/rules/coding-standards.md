# 编码规范与验证命令

## 验证命令矩阵

### Rust（根 workspace）

所有构建、验证和提交命令均在项目 `nix develop` 内执行。

```bash
cargo build --workspace
cargo test -p <受影响 crate>
cargo clippy -p <受影响 crate> --all-targets -- -D warnings
cargo fmt --all
```

跨 crate 变更使用全量门禁：

```bash
bun run verify:rust        # scripts/check-rust-gates.sh
bun run verify:rust:fast   # 跳过测试，pre-commit 同款
```

### 前端（依赖在仓库根目录安装，单一 `bun.lock`）

```bash
bun install

bun run --filter pp-web verify            # apps/panel：构建 + oxlint + oxfmt
```

> 前端目前没有单元测试运行器，`verify` 即自动化边界（见 `AGENTS.md` §7）。

## 提交规范

见 `AGENTS.md` §5：原子化提交，`type(scope): subject`，一个提交一个逻辑单元，禁止 `git add -A`。
完成定义与停止条件见 `AGENTS.md` §11。
