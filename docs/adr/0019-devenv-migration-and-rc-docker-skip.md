# ADR-0019: 开发环境迁移至 devenv 与 RC 标签跳过 Docker 发布

- **Status:** Accepted
- **Date:** 2026-10-25
- **Deciders:** 维护者
- **Scope:** `flake.nix`、`flake.lock`、`devenv.nix`、`nix/tools.nix`、`.envrc`、`.github/workflows/release.yml`、文档
- **Related:** Lanpath 仓库同款 devenv 集成（独立决策，结构对齐）

## Context

开发环境此前由根 `flake.nix` 直接以 `pkgs.mkShell` 描述：工具清单、runtimeLibs
与 `LD_LIBRARY_PATH` 注入都堆在一个文件里，无法按职责拆分，也没有 direnv 自动
加载。Lanpath 仓库已采用 `devenv.lib.mkShell` + `devenv.nix` + `nix/*.nix` 模块
的集成方式（`.envrc` 使用 nix-direnv，`nix develop --no-pure-eval`），两个仓库
的开发环境体验开始分化。

同时，Release 工作流的 `docker` job 对所有 `v*` 标签（包括 `v0.5.0-rc.1` 这类
RC 预发布标签）都会构建并推送 GHCR 镜像，打上 `latest` 之外的 semver 标签。
RC 的定位是验证二进制与升级链路，预发布镜像会污染 `ghcr.io` 上的版本语义，
且每次 RC 都白消耗双镜像的构建时间与缓存写入。

## Decision

1. **开发环境迁移至 devenv（flake 集成，不走 devenv CLI）**：
   - 根 `flake.nix` 改为 `devenv.lib.mkShell`，保留 `x86_64-linux` /
     `aarch64-linux` 两个 system；`nixpkgs` 维持 `nixos-unstable` 不变，
     `devenv` input 固定 `v2.4.0` 且 `inputs.nixpkgs.follows = "nixpkgs"`。
   - 原 `flake.nix` 中的 runtimeLibs（zlib/xz/bzip2/zstd）与 packages
     （git、curl、构建工具、protobuf、rustup、bun、nodejs 等）原样迁移到
     `nix/tools.nix`；`LD_LIBRARY_PATH` 前置注入逻辑保留在 `enterShell`。
   - `devenv.nix` 仅 `imports ./nix/tools.nix` 并提供进入提示。
   - `nixConfig` 在保留全部国内镜像与 `cache.nixos.org` 的前提下，新增
     `devenv.cachix.org` substituter 及其 trusted public key。
2. **新增 `.envrc`**：使用 nix-direnv 3.1.0（`source_url` 固定版本与
   sha256），`watch_file flake.nix / flake.lock / devenv.nix / nix/*.nix`，
   `use flake . --no-pure-eval`。开发者可用 direnv 自动进入环境，也可继续
   手动 `nix develop --no-pure-eval`。
3. **不启用 devenv git hooks**：提交检查唯一入口仍是 husky（`bun install`
   时 prepare 安装），devenv 侧不配置任何 hooks，避免双入口漂移。
4. **RC 标签跳过 Docker 发布**：`docker` job 增加
   `if: ${{ !contains(github.ref_name, '-') }}`——RC 预发布标签（ref 含 `-`）
   跳过镜像构建与推送；稳定版本标签与 `workflow_dispatch` 手动触发保留。
   RC 标签仍完整执行 `web`/`build`/`release`，发布二进制与 `SHA256SUMS`，
   并照常写入 GitHub Actions 构建缓存（`rust-cache` 与 docker layer cache
   的既有层不被清除）。

## Consequences

- 开发环境描述按职责拆分（`devenv.nix` + `nix/tools.nix`），与 Lanpath 结构
  对齐，降低两仓维护成本。devenv 定位项目根目录需要非 pure eval，因此所有
  `nix develop` 调用（本地、文档示例、CI）统一带 `--no-pure-eval`（与 Lanpath
  一致）；direnv 场景由 `.envrc` 内置该标志。
- `flake.lock` 新增 `devenv` 及其传递 inputs；`nixpkgs` 锁定版本不变。
  更新 devenv 需显式 `nix flake lock --update-input devenv` 并复跑
  `verify:rust` 与管理 Web verify。
- 新增 devenv binary cache 订阅源；国内镜像全部保留，优先级不变。
- RC 发布不再产生 GHCR 预发布镜像；稳定版的 `latest` 标签只由稳定标签
  推送，消除 RC 覆盖 `latest` 的风险。需要 RC 镜像验证时需手动触发
  workflow（基于无 `-` 的分支/tag ref）或本地构建。

## Alternatives considered

- **保持纯 `pkgs.mkShell` 仅用 direnv**：放弃。无法按模块拆分工具清单，
  与 Lanpath 的 devenv 结构继续分化，`.envrc` 仍要自己维护 watch 列表
  之外的加载逻辑。
- **迁移到 devenv CLI（`devenv.toml`/`devenv.nix` 独立入口）**：放弃。引入
  第二套入口与 state 目录，CI 与文档中的 `nix develop --command` 调用全部
  要改；flake 集成已能覆盖需求。
- **RC 继续发布 Docker 但禁用 `latest`**：放弃。现有 metadata 已对 `latest`
  做了 semver 门控，真正的问题是 RC 镜像本身无人消费且白消耗构建资源；
  job 级 `if` 比逐 tag 门控更简单可审计。
- **用 tag 正则精确区分 RC**（如 `*-rc.*`）：放弃。`contains(ref, '-')`
  与 Release job 的 `prerelease` 判定口径一致，覆盖所有带预发布段的
  SemVer，且无额外正则维护成本。
