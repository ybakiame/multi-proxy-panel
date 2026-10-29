# Changelog

本文件记录 ProxyPanel 的用户可见变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)；提交与发布流程见
[docs/contributing.md](docs/contributing.md)。

> 维护说明：
> - `v0.4.0`–`v0.4.5` 条目依据仓库 git 发布标签（日期 + 标签说明）整理；`v0.4.0` 之前的
>   版本历史未在仓库中保留，不做回溯重建。
> - `[Unreleased]` 记录 `v0.5.0-rc.3` 之后 mainline 的主要变更主题（逐条提交历史见
>   `git log v0.5.0-rc.3..HEAD`），发布时按下述流程整理为新版本标题。

## [Unreleased]

### Security

- 强化预生成节点安装链路：Agent Token 不再出现在 systemd 进程参数中，安装脚本与 CLI 校验必填参数并要求非本机 Hub 使用 HTTPS，下载增加协议、超时和重试约束；Hub/Caddy 模板仅监听本机并分离公开端点与管理认证。

### Removed

- 移除 `auto_register_agents` 及 Agent 自动注册路径；所有节点必须先在 Hub 预生成，旧配置键不再生效。见 ADR-0020。

### Fixed

- 修正 README 与文档中的支持协议、订阅输出、节点接入和安装示例，并精简重复的开发部署说明。

## [0.5.0-rc.3] - 2026-10-28

### Removed

- 移除 pi 编码代理配置及其当前文档引用；保留 `.agents` 项目规则与局部面板约定。

### Fixed

- 修复节点、客户端等表格操作菜单点击后不执行任何操作的问题。
- 修复 Hub Release 的前端资源目录与 CLI 安装、升级路径不一致的问题；CLI 同时兼容已发布压缩包中的旧目录，缺少前端资源时不再静默完成安装。

## [0.5.0-rc.2] - 2026-10-25

### Added

- 开发环境支持 direnv 自动加载：新增 `.envrc`（nix-direnv 3.1.0，watch flake/devenv/nix 文件）。

### Changed

- **管理面板 UI 骨架重设计**：灰白工作区 + 白色圆角卡片 + 湖水青重点卡的新视觉骨架（全量 OKLCH token、亮暗主题），分组导航 + 紧凑顶部栏 + 移动抽屉，登录页品牌区/表单区分栏，仪表盘改为真实数据的欢迎区、KPI 与局部加载/错误重试；新增 Hub/UI 双版本标识。
- **表格易用性**：全部数据表统一为共享 DataTable，提供 sticky 表头、统一密度和加载/错误/空态；节点与客户端操作收进行菜单，小屏改为关键字段卡片，宽表保留键盘可访问的内部滚动。
- **构建环境**：开发环境迁移至 devenv flake 集成——新增 `devenv.nix` 与 `nix/tools.nix`（原 `flake.nix` 工具清单与运行时库原样迁移），`flake.nix` 改用 `devenv.lib.mkShell`（devenv v2.4.0，nixpkgs 保持 nixos-unstable，新增 devenv binary cache、国内镜像全部保留）；所有 `nix develop` 调用统一带 `--no-pure-eval`（devenv 定位项目根目录所需）；不启用 devenv git hooks，husky 仍是唯一提交检查入口。见 ADR-0019 与 [Nix 开发环境](nix-flake.md)。
- **发布流程**：RC 预发布标签（如 `v0.5.0-rc.1`）跳过 Docker 镜像发布，仅发布二进制 tar.gz 与 `SHA256SUMS` 并保留构建缓存；稳定版本标签与手动触发保留 GHCR 镜像发布。见 ADR-0019。

## [0.5.0-rc.1] - 2026-10-25

### Fixed

- **日志记录**：Hub 运行日志通过有界异步队列写入数据库；日志页可切换系统/节点日志，支持刷新、首页自动更新、完整消息展开和明确的加载失败提示。Hub 与 Agent 持久化日志增加凭据脱敏及字段长度限制。

### Changed

- **管理面板 UI**：从 HeroUI 完整迁移到 Base UI 版 shadcn/ui，组件源码纳入仓库以便定制；移动导航改用可访问的侧栏抽屉，分页补齐双语可访问名称并在调整每页条数后回到第一页。
- 更新 Agent Development Protocol 引用至 `c502eaf`，保留 0.1 接口，采用按需阅读与增量维护说明并提供可选只读检查器。
- **产品拆分**：客户端岚径 · Lanpath 已迁入独立仓库，通过公开订阅/API 与 ProxyPanel 协作；本仓只维护 Hub、Agent、管理 Web、CLI 与服务端共享模块，服务端继续支持 sing-box/mihomo。
- **开发与构建配置**：Rust/Bun workspace、依赖锁、Nix、CI、Docker 与提交钩子按服务端范围收窄；移除 Android/桌面工具链与客户端打包依赖。Linux CI 检查使用项目 Nix 环境，服务端发布与部署链路保持兼容。

### Added

- **CLI 检查更新**：`proxy-panel check-update` 无需 root 对比 Hub/Agent/CLI 当前版本与最新稳定 Release，支持 `--json` 机器可读输出；`upgrade`/`rollback` 共用 `maintenance.lock` 非阻塞锁，`upgrade --status-file` 原子写入 JSON 状态，失败时尝试从最新备份恢复（备份仅保留一份）。
- **Hub 面板更新中心**：新增 `GET/POST /api/v1/system/update` 与 `GET /api/v1/system/update/status`（scope `system:read`/`system:write`）及面板 `/system` 页。默认仅检测更新并给出手动命令；管理员可选安装受限 sudoers 规则与 `NoNewPrivileges=false` drop-in 后开启面板一键升级（transient unit 执行，Hub 停止不中断更新；本版本不含 Agent 远程升级）。
- **Agent Development Protocol 采纳**：`docs/adoption/` 保存协议审计与采纳结果；新增 ADR 索引、代理适配器说明、工作流和完成定义。
- WSL `:Zone.Identifier` 清理脚本（`scripts/clear-zone-identifier.sh`）。
- 补充 GNU AGPL-3.0 `LICENSE` 文件，明确 `AGPL-3.0-or-later` 许可。

### Removed

- 移除 `.opencode` 编码代理适配器及根配置入口，当前适配器文档与协议维护记录同步更新。
- 原仓客户端应用、共享前端包、客户端领域/Tauri/MITM/脚本 crate、Windows 测试构建与 updater 发布步骤，以及客户端专属文档、IDE/参考工程和可重建缓存。客户端历史变更由 Lanpath 与原仓 Git 历史保留，用户数据不作删除。

## [0.4.5] - 2026-08-25

### Changed

- 备份仅保留最新一份。

### Removed

- 清理过时脚本。

### Security

- 移除泄露密钥。

## [0.4.4] - 2026-08-24

### Fixed

- 修复节点列表慢查询。
- 调整 `agent_logs` 保留策略。
- 修复 CLI `chown` 问题。

## [0.4.3] - 2026-08-23

### Added

- 节点接入两阶段向导。
- 节点状态筛选。

### Fixed

- 保护 Agent 托管字段。

## [0.4.2] - 2026-08-23

### Fixed

- 修复 CLI 升级运行中服务失败（ETXTBSY）。

## [0.4.1] - 2026-08-23

### Fixed

- 修复 CLI 跨文件系统安装失败（EXDEV）。

## [0.4.0] - 2026-08-23

### Added

- CLI 生命周期管理。
- 一键安装指令。
- 前端引入 TanStack Query。
- CI/CD Release 流程。

[Unreleased]: https://github.com/ybakiame/multi-proxy-panel/compare/v0.5.0-rc.2...HEAD
[0.5.0-rc.2]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.5.0-rc.2
[0.5.0-rc.1]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.5.0-rc.1
[0.4.5]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.5
[0.4.4]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.4
[0.4.3]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.3
[0.4.2]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.2
[0.4.1]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.1
[0.4.0]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.0
