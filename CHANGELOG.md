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

### Added

- **客户端**：合并为单壳双目标应用 `apps/client`（Tauri 2，桌面 HeroUI / 移动 Konsta，
  独立 cargo 壳 + `crates/pp-client-tauri` + `packages/client-core`），见 ADR-0007 与
  `docs/plans/2026-10-03-client-merge-evaluation.md`。
- **客户端流量统计**：Clash API WebSocket 连接追踪 + 本地 SQLite 聚合、首页今日流量卡片与
  流量详情页，见 ADR-0006。
- **客户端配置管理**：可视化配置切片（Config Slices，ADR-0005）、DNS / 出站 / 路由 /
  实验性配置与入站管理页、本地规则与规则集管理、全局配置变更重启提示。
- **客户端核心管理**：远端核心版本改为通道模型（稳定版 / 测试版 / 预发布版）。
- **Windows 桌面端**：启用 Tauri NSIS 打包与图标集，TUN 提权与 `wintun.dll` 随核心分发，
  Release 流程新增 Windows 安装包构建。
- **Windows 桌面端打包完善**（ADR-0008）：安装包内置 sing-box 种子核心 + `wintun.dll`
  （首启免联网即可用，升级仍走核心管理下载通道，随包附 GPL-3.0 / wintun 许可证文本）；
  新增 Windows ARM64 安装包；新增自动更新（设置页「关于应用 → 检查更新」，GitHub
  Releases + ed25519 签名校验，预发布不进入自动更新）；WebView2 改为内嵌引导（离线
  可装）；NSIS 固定 per-user 安装（免 UAC）。安装包不签名，SmartScreen 警告为预期行为。
- **Android 目标**：移动 UI 迁移至 Konsta UI（iOS/Material 双主题），构建收敛为 arm64 单 ABI，
  新增 `apps/client/scripts/android-ndk-env.sh` 与 nix dev shell 的声明式 NDK 工具链。
- **Agent Development Protocol 采纳**（`docs/adoption/`）：Pass 1 审计（inventory / mapping /
  audit）与 Pass 2 结果（adoption-report）；新增 `docs/adr/README.md`、`docs/adapters/README.md`、
  `AGENTS.md` §10 工作流与 §11 完成定义/升级条件、本文件。
- WSL `:Zone.Identifier` 清理脚本（`scripts/clear-zone-identifier.sh`）。
- 补充 `LICENSE` 文件（GNU AGPL-3.0 完整文本，对应 `Cargo.toml` / `package.json` 声明的
  `AGPL-3.0-or-later`）；此前 README 的许可证链接指向不存在的文件。

### Changed

- 客户端收敛为 sing-box 单核心（移除 mihomo 支持），见 ADR-0004。
- 客户端架构由 ADR-0003 的 desktop/mobile 双应用回摆为 `apps/client` 单应用（ADR-0007），
  平台差异保留为编译期事实。

### Fixed

- 构建命令补充 `LD_LIBRARY_PATH` 环境导出。
- 移动端 Android 交叉编译与 lint/重启链路问题若干（详见 `git log v0.4.5..HEAD`）。
- **Windows 客户端**：数据目录在 HOME 环境变量缺失时解析为相对路径，导致配置落入安装
  目录且核心因子进程 `-D` 切换工作目录后读不到相对路径配置而无法启动；数据目录改经
  `dirs::home_dir` 解析（保底为绝对路径），pp-core 进程管理器构造时将二进制与配置目录
  绝对化。
- **客户端核心管理**：修复「检查更新」无响应（按钮无加载态、远端通道查询错误被静默
  吞掉，现展示错误并引导配置 GitHub 代理前缀）；修复慢速直连网络下核心下载必然失败
  （reqwest 全局 30s 超时把 body 下载计入总时长，现拆分为连接超时 10s + 仅元数据 API
  限时 30s，二进制下载不限总时长）。

### Removed

- **客户端**：移除「探测系统核心」功能（PATH 系统核心探测、系统来源标记与相关命令/
  界面入口）；可用核心统一为下载 / 安装包种子核心。

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
