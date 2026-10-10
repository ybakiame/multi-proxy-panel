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

- **桌面端规则管理**：补齐「指定出站」动作（对齐移动端）——出站候选 = 订阅节点 +
  模板分组 + 切片出站并集，候选装配下沉 `client-core` 共享 hook（ADR-0011 D1/D3）。

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
- **桌面端 MITM CA 信任管理**：MITM 页新增「一键安装到系统信任库」与信任状态检测
  （Windows 写入当前用户信任库、免管理员；macOS / Linux 经系统提权框安装），并修正
  CA 说明文案（移除与本页无关的 Android / iOS 条目、更正证书来源为客户端本地自签）。
- 补充 `LICENSE` 文件（GNU AGPL-3.0 完整文本，对应 `Cargo.toml` / `package.json` 声明的
  `AGPL-3.0-or-later`）；此前 README 的许可证链接指向不存在的文件。
- **客户端 MITM / 脚本生态兼容**：Snippet 导入与远程订阅解析补齐三方生态语法——
  QX `rewrite_local` 全 action（reject 五变体 / 302 / 307 / echo-response / script 六形态 /
  双正则 body 改写，含混入 Loon / Surge 风格行按行型分派）、Loon `[Rewrite]` section 与
  逗号粘连参数 / URL 编码 argument / max-size、Surge 尾部 mode token / 阶段前缀 /
  `[Body Rewrite]`；QX 顶层裸 `hostname = ...` 行正确汇入 MITM 白名单；hostname 支持
  中段 / 尾部通配与端口剥离。重写引擎支持 Reject 响应变体、Redirect、头块正则与
  双正则 Body 改写；脚本钩子 `$done` 语义对齐三方（URL 替换、mock 响应、abort、
  字符串 body、statusCode 别名），并修复 URL Rewrite 命中后不回写请求行的问题。
- **客户端 MITM 白名单派生视图**：MITM 页白名单改由已启用远程 Snippet 与本地导入
  自动派生（域名 + 来源标注），手动填写降级为「手动补充（高级）」并与派生白名单
  合并生效。
- **客户端核心生命周期管理**（ADR-0012，纵深防御四层）：OS 级父子绑定（Windows Job
  Object / Linux PR_SET_PDEATHSIG，父进程被杀核心即被 OS 回收）、PID 文件 + 启动收割
  （exe 路径校验防 PID 复用误杀）、退出事件清理（ExitRequested 时停止核心并恢复系统
  代理）、端口占用前置诊断（mixed / Clash API / MITM 回流端口被外部进程占用时报错并
  指明进程名与 PID，替代原始核心 FATAL）。

### Changed

- **客户端版本**：客户端独立版本升级至 0.1.1（前端、Tauri 配置与壳清单同步）。

- **客户端订阅**：双端移除启停开关，全部订阅均可选择；旧停用订阅在选择生效时恢复可用。

- **客户端仪表盘**：双端共用运行数据、今日统计与规则模式控件，保留桌面核心门禁和 Android VPN 授权重试；今日统计卡支持键盘操作。

- **客户端统计**：双端共用查询与排序流程，桌面保留可排序表格与完整明细，移动保留卡片视图。

- **客户端日志**：双端共用文件查看与导出流程，桌面保留实时日志过滤和路径复制，清空内存日志前增加确认。

- **客户端 UI**：桌面与 Android 统一使用自研 `@pp/ui`，移除客户端 HeroUI/Konsta 依赖，
  保留移动 iOS/Material 双主题。配置、DNS、出站、路由、规则、规则集及实验性配置
  共用页面实现；桌面规则列表为表格，移动为卡片，编辑分别使用弹窗/抽屉。路由表合一，
  订阅规范路径为 `/subscriptions`，旧 `/nodes` 等路径自动重定向。双端通知统一使用
  静态队列，避免 view-transition。

- 客户端收敛为 sing-box 单核心（移除 mihomo 支持），见 ADR-0004。
- 客户端架构由 ADR-0003 的 desktop/mobile 双应用回摆为 `apps/client` 单应用（ADR-0007），
  平台差异保留为编译期事实。

### Fixed

- **Android 输入法避让**：原生容器处理 edge-to-edge 下的 IME Insets，让旧 WebView 的实际视口也随键盘缩小，修复共享表单仍被键盘遮挡。

- **Windows 构建**：protoc 安装 Action 使用 GitHub token，避免匿名 API 限流导致测试和发布构建失败。

- **客户端开发预览**：桌面和移动 Vite 缓存按 mode 隔离，修复同时运行时桌面 React Compiler 依赖被覆盖导致页面无法加载。

- **客户端关于信息**：双端统一实际应用版本与项目链接；桌面项目链接改由系统浏览器打开，移动补齐版本与链接，tags 明确标注为构建配置。

- **移动订阅列表**：长 URL 在卡片内省略显示，避免撑宽容器。

- **移动表单**：共享抽屉随键盘可见视口抬升，并自动滚动焦点输入框，避免表单被输入法遮挡。

- **客户端订阅编辑**：保存失败保留表单；移动编辑保留未展示的覆写模板绑定；桌面删除增加确认。

- **桌面订阅导航**：侧栏与代理页统一使用 `/subscriptions`，修复点击「订阅」后菜单高亮失效。

- **桌面端规则编辑**：规则集绑定由单选改为多选（后端与移动端本就支持逗号分隔多 tag，
  桌面表单此前误用单值选择器只能绑定一个规则集）。
- **客户端 MITM**：未配置任何脚本 / 白名单时不再劫持全部流量——此前空白名单会退化为
  match-all 路由，所有 CONNECT 被强制解密，CA 未受系统信任时全网 TLS 握手失败；同时
  为 MITM 引擎补齐 TLS ClientHello SNI 级第二道拦截判定（白名单外盲隧道透传）。
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
- **Windows 客户端**：修复启动时 cmd 窗口不断闪动（GUI 应用下子进程未标记
  `CREATE_NO_WINDOW`，核心 run / 版本探测 / reg 系统代理命令各自弹出控制台窗口，
  现统一施加）。

### Changed

- **桌面客户端**：规则管理与规则集管理自侧边栏独立「规则」页迁入「配置 → 路由管理」
  （`/config/route` 入口区，`/config/route/rules` 与 `/config/route/rulesets` 子页），
  与移动端组织方式对齐，数据键不变（`local_override.json`）。

### Removed

- **客户端**：移除「探测系统核心」功能（PATH 系统核心探测、系统来源标记与相关命令/
  界面入口）；可用核心统一为下载 / 安装包种子核心。

### Added

- **桌面客户端**：规则集市场上线（配置 → 路由管理 → 规则集管理 → 市场，MetaCubeX
  固定源本地检索 + 一键添加，与移动端共用 client-core 的 `useRuleSetMarket` 视图模型）；
  配置三级页面（规则管理 / 规则集管理 / 市场）补充返回导航。

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
