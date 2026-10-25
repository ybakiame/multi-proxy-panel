# Nix 开发环境

ProxyPanel 的本地构建、运行、验证与提交钩子统一使用根目录 `flake.nix` / `flake.lock`
（devenv flake 集成，见 [ADR-0019](docs/adr/0019-devenv-migration-and-rc-docker-skip.md)）。
管理 Web 的 Bun、Protobuf 编译器与系统构建工具由 Nix 提供；Rust 经 Nix 提供的 rustup
按 `rust-toolchain.toml` 管理。Linux CI 检查使用同一环境。服务端 Release 保留既有 runner/容器构建链路，
以维持 GNU/Linux 二进制的部署兼容性；不直接分发带 Nix store 动态加载器的开发产物。

## 结构

- `flake.nix`：flake 入口，经 `devenv.lib.mkShell` 生成 devShell（`devenv` input 固定
  `v2.4.0`，`inputs.nixpkgs.follows` 本仓 nixpkgs）；`nixConfig` 保留全部国内镜像并新增
  `devenv.cachix.org` substituter。
- `devenv.nix`：devenv 模块入口，imports `nix/tools.nix`，`enterShell` 打印进入提示。
- `nix/tools.nix`：工具清单（原 flake 的 runtimeLibs 与 packages 原样迁移）与
  `LD_LIBRARY_PATH` 前置注入。
- `.envrc`：direnv 自动加载（nix-direnv 3.1.0，`use flake . --no-pure-eval`），
  watch `flake.nix` / `flake.lock` / `devenv.nix` / `nix/*.nix`。
- husky 仍是唯一提交检查入口；不启用 devenv git hooks。

## 使用

安装支持 flakes 的 Nix 后，在仓库根目录执行（`--no-pure-eval` 为 devenv 定位
项目根目录所需，手动与自动化调用都要带上）：

```bash
nix develop --no-pure-eval
bun install --frozen-lockfile
cargo build --workspace
bun run verify:rust
bun run --filter pp-web verify
```

已安装 direnv 时 `cd` 进仓库即自动进入同一环境（`.envrc` 已含该标志）。

自动化使用非交互 shell，避免用户 rc 覆盖环境：

```bash
nix develop --no-pure-eval --command bash --noprofile --norc -c 'bun run verify:rust'
```

所有开发、验证和 git 提交命令均在该环境运行。数据库和容器服务由外部提供。
环境支持 `x86_64-linux` 和 `aarch64-linux`。
`flake.lock` 锁定 nixpkgs 与 devenv；Rust 的 channel / components 以 `rust-toolchain.toml` 为准。
当前 Rust channel 为 stable，rustup 的实际下载与缓存不写入 Nix store。

## 环境范围

- Rust：rustup、C/C++ 构建工具、pkg-config、Protobuf。
- 管理 Web：Bun、Node.js。
- Agent 的压缩库：zlib、xz、bzip2、zstd；运行时库路径由 shell 设置。
- Git、curl、wget、unzip 与常规构建工具。

客户端已独立到 [Lanpath](https://github.com/ybakiame/lanpath)；本仓不提供 Go、JDK、
Android SDK/NDK、Gradle、Tauri 或桌面 GUI 系统库。Lanpath 使用自身的 Nix 配置
（结构与本仓对齐，均为 devenv flake 集成）。

## 网络与维护

下载失败时可按本机代理配置重试；此工作区示例：

```bash
export http_proxy="http://127.0.0.1:7899"
export https_proxy="http://127.0.0.1:7899"
nix develop --no-pure-eval
```

若镜像不可用，可显式选择官方缓存：

```bash
nix develop --no-pure-eval --option substituters https://cache.nixos.org
```

更新 devenv input 必须复跑服务端 Rust 门禁与管理 Web verify：

```bash
nix flake lock --update-input devenv
bun run verify:rust
bun run --filter pp-web verify
```
