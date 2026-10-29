# ADR-0021: 开发环境全面采用原生 devenv CLI

- **Status:** Accepted
- **Date:** 2026-10-29
- **Deciders:** ProxyPanel 维护者
- **Scope:** `devenv.nix`、`devenv.yaml`、`devenv.lock`、`.envrc`、CI 与开发文档
- **Related:** ADR-0019

## Context

ADR-0019 使用 `devenv.lib.mkShell` 将 devenv 集成到 flake，虽然拆分了模块，但仍保留 `flake.nix`、`flake.lock` 和 `nix develop` 第二套入口。原生 devenv 已能通过 `devenv.yaml` 管理 inputs、通过 `devenv.lock` 固定版本，并以 `devenv shell` 在本地和 CI 复用同一环境。

## Decision

- 使用 `devenv.nix`、`devenv.yaml`、`devenv.lock` 作为唯一环境声明，删除 `flake.nix` 与 `flake.lock`。
- 本地及 CI 统一使用 `devenv shell`，direnv 使用标准 `use devenv`。
- CI 通过 Nix 安装 devenv CLI，path filters 跟踪原生配置和锁文件。
- 仅使用官方 `cache.nixos.org` 与 devenv 官方 Cachix，不声明其他第三方镜像。
- 保留 ADR-0019 中 RC 标签跳过 Docker 发布的决定；Release 的 GNU/Linux runner/容器构建链路不随开发环境迁移改变。

## Consequences

- 环境入口和锁文件只有一套，减少 flake 与 devenv 并存造成的漂移。
- 本地、CI 和文档不再依赖 `--no-pure-eval` 或 `nix develop`。
- 更新 inputs 需运行 `devenv update` 并提交 `devenv.lock`，随后复跑 Rust 与 Web 门禁。
- 原生锁文件变更会影响 CI 环境，必须在本地完成相同命令验证后提交。
