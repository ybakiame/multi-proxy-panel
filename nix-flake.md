# devenv 开发环境

ProxyPanel 的本地构建、运行、验证与提交钩子统一使用原生 **devenv**。环境由 `devenv.nix`、`devenv.yaml` 和 `devenv.lock` 声明，不再使用 `flake.nix` / `flake.lock` 或 `nix develop`。决策见 [ADR-0021](docs/adr/0021-native-devenv-cli.md)。

管理 Web 的 Bun、Node.js、Protobuf 编译器与系统构建工具由 Nix 提供；Rust 经 rustup 按 `rust-toolchain.toml` 管理。Linux CI 使用同一环境。Release 保留既有 GNU/Linux runner/容器构建链路，不直接分发带 Nix store 动态加载器的开发产物。

## 结构

- `devenv.nix`：devenv 模块入口，导入 `nix/tools.nix` 并显示进入提示。
- `devenv.yaml`：声明 nixpkgs 与 devenv v2.4.0 inputs，devenv 的 nixpkgs 跟随本仓 input。
- `devenv.lock`：锁定所有 inputs，由 `devenv update` 生成或更新，必须提交。
- `nix/tools.nix`：构建工具、runtimeLibs 与 `LD_LIBRARY_PATH` 前置注入。
- `.envrc`：使用 direnv 标准库 `use devenv`，watch 上述配置与锁文件。
- husky 仍是唯一提交检查入口，不启用 devenv git hooks。
- 缓存只使用官方 `cache.nixos.org` 与 devenv 官方 Cachix，不配置其他第三方镜像。

## 使用

安装 Nix 与 devenv 后，在仓库根目录执行：

```bash
devenv shell
bun install --frozen-lockfile
cargo build --workspace
bun run verify:rust
bun run --filter pp-web verify
```

单条命令可直接执行：

```bash
devenv shell -- bun run verify:rust
devenv shell -- bun run --filter pp-web verify
```

已安装 direnv 时，进入仓库会自动加载同一环境。支持 `x86_64-linux` 和 `aarch64-linux`。

## 环境范围

- Rust：rustup、C/C++ 构建工具、pkg-config、Protobuf。
- 管理 Web：Bun、Node.js。
- Agent 压缩库：zlib、xz、bzip2、zstd；运行时库路径由 shell 设置。
- Git、curl、wget、unzip 与常规构建工具。

数据库和容器服务由外部提供。Lanpath 客户端使用独立仓库的环境配置。

## 更新与验证

更新 inputs 后提交新的 `devenv.lock`，并复跑门禁：

```bash
devenv update
devenv shell -- bun run verify:rust
devenv shell -- bun run --filter pp-web verify
```

所有开发、验证和 Git 提交命令均在该环境运行。
