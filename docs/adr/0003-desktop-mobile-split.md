# ADR-0003: 客户端 Desktop / Mobile 双应用分离架构

- **Status:** Accepted（实施未启动）
- **Date:** 2026-09-05
- **Deciders:** ProxyPanel Contributors
- **Scope:** `apps/desktop`、`apps/android`、`apps/mobile`（新）、`packages/client-core`（新）、`crates/pp-client-tauri`（新）

---

## 1. Context & Problem Statement

### 1.1 现状

当前 `apps/desktop` 是单一 Tauri 2 工程，同时承载桌面（Linux/Windows/macOS）与 Android 两个目标：

- **UI 层**：单份 React 前端（`pp-client-ui`），`DesktopSidebar` 与 `MobileTabBar` 同时渲染、靠 CSS `lg` 断点切换，页面内部以 `capabilities.is_android` 运行时判断分支。
- **Rust 壳层**：单份 `src-tauri`，以 `#[cfg(target_os = "android")]` 与 `require_desktop()` 双分支适配平台差异。
- **Android 资产**：分散三处——`apps/android`（Go module + gomobile 构建脚本）、`apps/desktop/src-tauri/gen/android`（手写 Kotlin：`VpnPlugin` 312 行 + `ProxyVpnService` 1154 行）、`apps/desktop/src-tauri/src/core_bridge.rs`（Rust↔Kotlin 桥）。

### 1.2 问题证据（2026-09 调研数据）

| 证据 | 数据 | 影响 |
|---|---|---|
| 前端平台判断散落 | 39 处 `isAndroid` / `capabilities.is_android`（Dashboard 719 行内 8 处、Settings、Rules、Logs、NetworkSettings 等） | 运行时判断易遗漏，新增功能需人工排查双端表现 |
| Rust 壳 cfg 分支 | 94 处 `cfg(target_os = "android")` / `require_desktop`（commands/、logs/、lib.rs、state.rs） | 命令实现被平台分支切碎，可读性差 |
| 响应式伪装平台适配 | 布局靠 CSS 断点而非平台事实 | Android 平板显示桌面侧栏、桌面窄窗显示移动 TabBar，语义错位 |
| 单壳 feature gating 硬伤 | `src-tauri/Cargo.toml` 注释：Tauri CLI 不支持 per-target feature flags | Android 禁 MITM 只能靠 CI 矩阵设环境变量 hack |
| Android 交互不友好 | 桌面 UI 硬塞移动端 | 移动交互（单手操作、Sheet、滑动）无法系统性设计 |

**核心洞察**：Rust 侧 cfg 是编译期强制的、相对不易遗漏；真正易遗漏的是前端运行时判断。UI 按应用分离后，mobile UI 天然只运行于 Android，`is_android` 查询失去存在意义——平台差异从「运行时判断」转为「编译期事实」。

---

## 2. Decision

**按平台拆分为两个独立 Tauri 应用 + 两层共享库**：

```
apps/
├── desktop/          # 桌面客户端（linux/windows/macos）
│   ├── src/          # 桌面 UI（React）
│   └── src-tauri/    # 桌面壳：桌面专属命令 + 共享命令注册
├── mobile/           # 移动客户端（android，目录约定预留 ios）
│   ├── src/          # 移动 UI（移动交互从零设计，非桌面改造）
│   ├── src-tauri/    # 移动壳：Android 专属命令 + 共享命令注册
│   │   └── gen/android/   # Kotlin VpnPlugin/ProxyVpnService 迁入
│   ├── panel-core/   # Go module（自 apps/android 迁入）
│   └── scripts/      # build-panel-core.sh（AAR 输出改指 mobile）
└── panel/            # 管理系统（不变）
packages/
└── client-core/      # @pp/client-core：api 封装 + hooks + atoms + types + 纯工具
crates/
├── pp-client/        # 纯业务逻辑（维持零 tauri 依赖，不变）
└── pp-client-tauri/  # 新：双端通用 Tauri 命令层 + AppState + logs
```

### 2.1 分发的三层模型

对应「操作统一方法调用 / 按类型+平台实现分发」的诉求：

| 层 | 载体 | 职责 | 平台差异处理 |
|---|---|---|---|
| **L1 前端 api 层** | `@pp/client-core` | UI 只 import 统一函数，禁止直接 `invoke()` | 能抹平的抹平（如 `startProxy` 双端同名同签名）；抹不平的（如 `requestVpnPermission`）仅由对应端 UI import |
| **L2 Rust 命令层** | `pp-client-tauri` + 双壳 | 双端通用命令单份实现；平台专属命令只在对应壳注册 | 分离的 UI 天然不会调用不存在的命令，无需运行时守卫 |
| **L3 核心引擎层** | `pp-client::core_engine`（既有） | `CoreEngineBridge` 统一核心生命周期 | 桌面 `CoreManager` spawn 二进制 / Android Kotlin VpnPlugin 桥，维持现状 |

**命令内平台差异处理原则**：能参数化的参数化（如 `data_dir` 由壳层解析后传入共享层，替代 `resolve_data_dir` 的 cfg 分支）；不能参数化的才在共享 crate 内保留最小 cfg。

### 2.2 capabilities 矩阵语义变化

`CapabilitiesView.is_android` 字段随 UI 分离失去消费方。矩阵保留但语义从「平台判断」退化为「运行时功能开关」（如 desktop 上 mitm 仍可能因环境禁用）。`get_capabilities` 命令保留于共享层。

### 2.3 已决事项

| 决策点 | 结论 |
|---|---|
| 总体方案 | B1：双 Tauri 壳 + 两层共享库（否决 A 现状增强 / B2 单壳双前端 overlay，见 §5） |
| 共享前端库 | `packages/client-core`，包名 `@pp/client-core`（Bun workspaces 新增 `packages/*`） |
| Rust 共享 crate | `crates/pp-client-tauri`（`pp-client` 维持零 tauri 依赖，不并入） |
| mobile UI 起点 | 从零设计移动交互（参考 ADR-0002 调研的 husi / FlClash 交互模式），不复制桌面 UI |
| 迁移策略 | M1→M5 渐进式，每里程碑独立可合入、零行为变化优先 |
| iOS 预留 | `apps/mobile` 目录与命名按可扩展设计（Tauri 2 支持 iOS 目标），当前仅实现 Android |

---

## 3. Detailed Design

### 3.1 `packages/client-core`（前端共享库）

迁入内容（自 `apps/desktop/src`，纯搬迁）：

- `api/`（14 个模块：Tauri invoke 封装 + 类型定义 + query keys）
- `hooks/`（`useCapabilities`、`useClientConfig`、`useProxyStatus` 等）
- `atoms/`（jotai atoms）
- 纯逻辑工具（`toast.ts`、`logCapture.ts`、`env.ts`）

**不迁入**：页面（`pages/`）、布局（`layout/`）、平台耦合组件——这些是双端分离迭代的核心诉求载体。中立展示组件（如 `ConfigPreviewModal`）是否上移，实施时按实际复用情况决定，初期不做。

### 3.2 `crates/pp-client-tauri`（Rust 共享命令层）

迁入内容（自 `apps/desktop/src-tauri/src`）：

- 双端通用命令模块（见 §3.3 归属表）
- `state.rs`（`AppState`；`default_data_dir` 的 cfg 分支改为由壳层注入 data_dir）
- `logs/`（日志初始化与查询；导出的平台差异按 §2.1 原则处理）

命令函数以 `pub` 导出，双壳在各自 `generate_handler!` 中以路径注册（Tauri 2 支持跨 crate 命令注册）。

### 3.3 命令归属划分（初步，最终以实施时审计为准）

| 归属 | 命令模块 | 说明 |
|---|---|---|
| 共享（`pp-client-tauri`） | `config`、`profile`、`subscription`、`proxies`、`connections`、`preview`、`task`、`local_override`、`logs`、`get_capabilities`/`platform_info` | 双端语义一致 |
| 桌面壳专属 | `mitm`、`core_mgmt`、`remote`、`tun_auth_status`/`authorize_tun` | Android 不可用，`require_desktop` 守卫随拆分删除 |
| mobile 壳专属 | `request_vpn_permission`、`vpn_last_error`、`notify_prefs_changed`、`core_bridge`（vpn 插件） | 桌面不注册 |
| 实施时审计 | `gpu_acceleration`、`toast_mode_override` | 双端均有消费方，归属待定 |

### 3.4 `apps/mobile`（Android 资产合并）

- `apps/android/panel-core`（Go module）与 `apps/android/scripts/build-panel-core.sh` 整体迁入 `apps/mobile/`，AAR 输出路径改指 `apps/mobile/src-tauri/gen/android/app/libs/panelcore.aar`。
- `apps/desktop/src-tauri/gen/android` 中手写 Kotlin（`VpnPlugin.kt`、`ProxyVpnService.kt`、`MainActivity.kt` 改动）与 gradle 配置（`abiFilters`、`libs` 依赖、minSdk 26）经 `tauri android init` 重新生成后 diff 迁移。
- `apps/android` 目录随之删除。

### 3.5 `apps/desktop` 瘦身（M4）

- Rust 壳：删除全部 `#[cfg(target_os = "android")]` 分支、`require_desktop`、`core_bridge.rs`、`resolve_data_dir` 的 Android 路径；`Cargo.toml` 移除 Android feature gating 注释与 mobile profile。
- 前端：删除 `MobileTabBar`、`MobileBackHeader`、`layout/config/nav/mobile.ts`、`App.tsx` 的移动安全区适配与 `MitmGuard`/`ScriptsGuard` 重定向（桌面端 capability 恒真）；清除全部 `isAndroid` 判断。

---

## 4. 迁移路径

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M1** | 建 `packages/client-core`，desktop 前端改为消费（纯搬迁） | `pp-client-ui` 的 `build + lint + format:check` 全通过，行为零变化 |
| **M2** | 抽 `pp-client-tauri`，desktop 壳改依赖 | `cargo build/test/clippy`（含 src-tauri 独立项目）通过，行为零变化 |
| **M3** | 建 `apps/mobile`：`tauri android init` + Kotlin 迁移 + `apps/android` 并入 + AAR 路径调整 + 最小移动 UI（可启动核心的首页） | Android 真机/模拟器可启动应用并启动核心 |
| **M4** | desktop 瘦身（§3.5） | 双端构建通过，desktop 无任何 android 残留（`rg target_os.*android` 为零命中） |
| **M5** | mobile UI 逐页按移动交互重写（Dashboard → Proxies → Settings → …） | 自此双端独立迭代 |

M1/M2 顺序可互换；M3 依赖 M2（mobile 壳从共享层组装，避免复制带 cfg 的代码再删）。

---

## 5. Alternatives Considered

| 方案 | 描述 | 否决理由 |
|---|---|---|
| **A. 现状增强** | 单 app + 平台组件注册表（`<PlatformSlot>` 分发组件）收敛判断 | 不解决分离迭代诉求；bundle 含双平台代码；Rust cfg 与 feature gating hack 保留 |
| **B2. 单壳 + 双前端** | 单 `src-tauri`，`tauri.android.conf.json` overlay 切换 `frontendDist` 指向 desktop-ui / mobile-ui | 94 处 cfg 保留；Android 禁 MITM 的 feature gating 硬伤无解（Cargo.toml 注释已证）；「一个壳编译所有平台」的耦合仍在 |
| **C. 完全独立双 app** | 双 app 无共享库 | api/types/hooks 全量复制，漂移风险大 |

---

## 6. Risks & Mitigations

| 风险 | 缓解 |
|---|---|
| Kotlin 迁移遗漏（gen/android 为生成工程混手写代码，1466 行） | 迁移前 `git diff` 存档手写部分清单；迁移后真机验证 VPN 启动/授权/通知全链路 |
| 壳层命令注册表两份，漂移 | 注册表即平台差异的显式声明，属预期；共享命令单份实现于 `pp-client-tauri` |
| 过渡期（M3 前）desktop 仍带 android 分支 | 里程碑短周期合入，M4 尽快收敛 |
| Bun workspaces 新增 `packages/*` 影响根安装 | `bun install` 单一 lockfile 机制不变，仅 workspaces glob 扩展 |
| `pp-client-tauri` 命令跨 crate 注册的 Tauri 2 兼容性 | M2 首个命令迁移时即验证；若不可行，回退为共享 crate 导出普通函数、壳层薄包装注册 |

---

## 7. Consequences

**正面**：

- 前端 39 处 `isAndroid` 运行时判断全部消失；Rust 94 处 cfg 分支绝大部分消失（编译期消灭）。
- Android 禁 MITM 由 mobile 壳 `Cargo.toml` 依赖表天然表达，删除 CI 环境变量 hack。
- 双端 UI 独立迭代互不牵制；移动交互可系统性设计。
- Android 构建资产（Go module、脚本、Kotlin、桥）内聚于 `apps/mobile` 单目录。
- 构建产物干净：desktop bundle 不含 Android 代码，反之亦然。

**负面 / 成本**：

- 新增 2 个共享载体（1 个 Bun package + 1 个 Rust crate），总代码量短期上升。
- 双份 `tauri.conf.json`、CI 矩阵调整。
- Kotlin 迁移与 Android 工程重建有一次性工程成本（M3）。
- `capabilities.is_android` 语义弱化，文档需同步（`docs/api_reference.md`、`AGENTS.md` 构建章节）。

---

## 8. References

- 调研基础：`apps/desktop/src-tauri/Cargo.toml`（feature gating 注释）、`src/api/system.ts`（capabilities 消费）、`src-tauri/src/commands/platform.rs`（能力矩阵）
- 关联 ADR：ADR-0002（客户端规则管理交互重设计，移动端优先原则一脉相承）
- 既有抽象：`crates/pp-client/src/core_engine.rs`（`CoreEngineBridge`，本 ADR L3 层直接沿用）