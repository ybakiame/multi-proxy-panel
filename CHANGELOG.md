# Changelog

本文件记录 ProxyPanel 的用户可见变更。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)；提交与发布流程见
[docs/contributing.md](docs/contributing.md)。

> 维护说明：
> - `v0.4.0`–`v0.4.5` 条目依据仓库 git 发布标签（日期 + 标签说明）整理；`v0.4.0` 之前的
>   版本历史未在仓库中保留，不做回溯重建。
> - `[Unreleased]` 记录 `v0.4.5` 之后 mainline 的主要变更主题（逐条提交历史见
>   `git log v0.4.5..HEAD`），发布时按下述流程整理为新版本标题。

## [Unreleased]

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

[Unreleased]: https://github.com/ybakiame/multi-proxy-panel/compare/v0.4.5...HEAD
[0.4.5]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.5
[0.4.4]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.4
[0.4.3]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.3
[0.4.2]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.2
[0.4.1]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.1
[0.4.0]: https://github.com/ybakiame/multi-proxy-panel/releases/tag/v0.4.0
