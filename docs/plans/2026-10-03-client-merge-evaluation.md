# 客户端合并迁移评估：apps/desktop + apps/mobile → apps/client

- **Date:** 2026-10-03
- **Status:** 评估稿（M0 已于 2026-10-03 实证通过，见 §6.1；主题键已前置统一，见 §4.1）
- **Scope:** `apps/desktop`、`apps/mobile`、`packages/client-core`、`crates/pp-client-tauri`、CI/CD 与构建脚本
- **关联：** ADR-0003（双应用分离，本评估实质上是对其「双 app」形态的再评估与部分回摆）

---

## 1. 背景与核心矛盾

ADR-0003（2026-09）刚把单一 Tauri 工程**拆成** desktop/mobile 双应用，理由有五（§1.2 证据表）：

1. 前端 39 处 `isAndroid` 运行时判断散落；
2. Rust 壳 94 处 `cfg(target_os = "android")` / `require_desktop` 分支；
3. CSS 断点伪装平台适配；
4. **Tauri CLI 不支持 per-target cargo feature flags**——Android 禁 MITM 只能靠 CI 环境变量 hack；
5. 移动交互无法系统设计。

本次合并诉求要成立，必须证明这五条在今天要么已消失、要么能在单壳内以「编译期事实」重新表达，而不是把运行时判断请回来。**这是本评估的主线。**

### 1.1 结论先行

**可以合并，但必须满足三条不变式**（任一不满足则应中止或降级方案）：

| 不变式 | 违反后果 |
|---|---|
| I1. UI 选择是**构建期**事实（vite mode / TAURI_ENV_PLATFORM），禁止运行时 `isAndroid` 分支选择页面树 | 重现 ADR-0003 问题 1/3 |
| I2. Rust 平台差异是**编译期**事实（`#[cfg]` + target 依赖表），壳内 cfg 总量有预算（≤ 20 处，当前预估 12-15 处） | 重现 ADR-0003 问题 2 |
| I3. Android 构建图**不含** `pp-mitm`（经 target 依赖表 + feature 门控表达，非 CI 环境变量 hack） | 重现 ADR-0003 问题 4 |

合并的真实收益（相对现状）：单一 `tauri.conf.json` + 单一壳 Cargo 项目（一份 lockfile、一份 target/ 缓存）、CI 壳层门禁收敛为一个 job、版本号/identifier 天然一致、双端共享的壳层样板（日志初始化、插件注册）单份化。**不合并也不会损失正确性**——现状双壳已是健康状态，合并是维护性优化而非修复。

---

## 2. 现状盘点（合并前的事实基线）

### 2.1 前端

| 维度 | apps/desktop | apps/mobile |
|---|---|---|
| UI 库 | HeroUI 3.2.2（+ react-compiler / oxc-transform-react） | Konsta UI 5.4（iOS/Material 双主题，无 react-compiler） |
| 规模 | ~12.2k 行（页面：Dashboard/Nodes/Rules/Proxies/Connections/Stats/Mitm/Scripts/Override/Tools/Config×5/Settings） | ~10.5k 行（TabBar + Sheet 交互体系，页面集不同） |
| 独有依赖 | codemirror 编辑器组、tauri-plugin-notification/opener JS 绑定、clsx | konsta |
| 入口差异 | `main.tsx` 多 `installLogCapture()`；index.html 预置 `heroui-theme` 脚本 | `theme.tsx`（Konsta 双主题 + localStorage 迁移）；index.html 预置 `pp-ui-theme` 脚本 |
| vite | 端口 1420（HMR 1421） | 端口 1430（HMR 1431） |
| tsconfig / oxlint 配置 | **完全相同**（已 diff 确认） | 同左 |
| 共享层 | `@pp/client-core`（api/hooks/atoms/纯工具，已在 ADR-0003 抽出） | 同左 |

运行时平台探测残留仅 4 处（`capabilities.is_android` 类型定义 + hook 注释 + mobile Dashboard VPN 错误轮询），且都在 mobile 侧按「恒真」语义消费——**合并后前端不存在必须运行时判断平台的场景**。

### 2.2 Rust 壳层

| 维度 | desktop 壳 | mobile 壳 |
|---|---|---|
| crate 形态 | lib + bin | lib + bin + cdylib + staticlib（Android 要求） |
| 本地命令 | `core_mgmt` / `mitm` / `remote` / `platform`（tun 授权） | 无（全部来自共享层 + cfg 包裹的 Android 三命令） |
| 独有依赖 | pp-client（默认 features 含 mitm）、pp-mitm、pp-script、reqwest、base64、chrono、uuid、tracing-appender… | 仅 pp-client-tauri + tauri 插件 |
| 独有逻辑 | WSL/WebKitGTK workaround（`#[cfg(target_os = "linux")]`，android target 天然不命中） | `resolve_data_dir`（app_data_dir）、`core_bridge::set_data_dir` 注入、vpn 插件注册、release 体积优化 profile |
| 共享命令注册 | 全路径注册 pp-client-tauri 35+ 命令 | 同左（少 desktop 专属、多 Android 三命令） |

共享命令层 `pp-client-tauri` 已是单份实现，feature `mitm = ["pp-client/mitm"]` 门控已就位；workspace 依赖 `pp-client = { default-features = false, features = ["scripts"] }` 意味着共享层默认即 scripts-only 形态。

### 2.3 Android 资产

- `apps/mobile/src-tauri/gen/android`：`tauri android init` 生成工程 + 手写 Kotlin（VpnPlugin / ProxyVpnService）+ gradle 定制（minSdk 33、abiFilters arm64-v8a、`libs/panelcore.aar` 引用、keystore 签名）；
- `apps/mobile/panel-core`（Go module，gomobile bind 出 panelcore.aar）+ `apps/mobile/scripts/`（build-panel-core.sh、android-ndk-env.sh）；
- `apps/mobile/keystore/`（签名密钥，**不入库迁移时注意 git 历史与 .gitignore**）；
- VPN 插件**不是**独立 tauri 插件 crate：Kotlin 类在 app 工程内，经 `pp-client-tauri::core_bridge::vpn_plugin()` 注册。

### 2.4 构建与 CI/CD

- Bun workspaces 根级统一安装；每 app `verify`（build + oxlint + oxfmt）；
- `scripts/check-rust-gates.sh`：根 workspace + desktop 壳 clippy + mobile 壳 host clippy + 有 NDK 时 android target check（pre-commit 同款快速版）；
- CI `client-shells` job：desktop clippy / mobile host clippy / mobile android check 三步；
- CI `web` job：`pp-client-ui` 与 `pp-client-mobile-ui` 分别 verify；
- release.yml：server 矩阵 + `desktop-windows`（NSIS，tauri-action projectPath=apps/desktop）；**无 Android 发布 job**（APK 目前仅本地构建）；
- flake.nix：统一 dev shell（Rust + Android SDK/NDK + Go + Bun），无需随合并大改。

---

## 3. Demo 架构与现状的差距分析

对 demo 树逐项裁决（✅ 采纳 / ✏️ 调整后采纳 / ❌ 不采纳）：

```
apps/client/
├─ src/
│  ├─ shared/        ❌ 不新设。共享层已是 packages/client-core（独立 verify 单元、
│  │                    query key 契约集中），内联进 apps/client/src/shared 是倒退。
│  │                    （选项：保留 packages/client-core 不动——推荐）
│  ├─ desktop/       ✅ apps/desktop/src 整体迁入（含 index.css / 页面 / 布局）
│  ├─ mobile/        ✅ apps/mobile/src 整体迁入（含 theme.tsx / index.css）
│  └─ main.ts        ✏️ 采纳但改为「构建期分发」：main.tsx 只做
│                       import("@app/App")，@app 由 vite mode alias 指向
│                       src/desktop 或 src/mobile；平台判断全部编译期完成
├─ src-tauri/
│  ├─ src/
│  │  ├─ lib.rs      ✏️ 共享装配（插件/日志/共享命令注册）+ cfg 分发到适配层
│  │  ├─ main.rs     ✅ 桌面入口（mobile 由 mobile_entry_point 注入，现状即如此）
│  │  ├─ desktop/    ✏️ 桌面适配层：现 desktop 壳的 commands/{core_mgmt,mitm,
│  │  │                 remote,platform} + state.rs + WSL workaround，
│  │  │                 #[cfg(not(any(target_os="android", target_os="ios")))]
│  │  └─ mobile/     ✏️ 移动适配层：resolve_data_dir + vpn 插件注册 + Android
│  │                     三命令注册，#[cfg(target_os = "android")]
│  ├─ binaries/      ❌ 不需要。桌面核心二进制是运行时下载到 data_dir/cores 的
│  │                    （core_mgmt），不走 tauri sidecar；Android 核心是 AAR
│  │                    内嵌 Go 引擎，也不是 sidecar
│  ├─ plugins/       ❌ 首期不做。VpnPlugin 现状是 gen/android 内手写 Kotlin +
│  │                    core_bridge 注册，非独立插件 crate；抽成 plugins/sing-box-mobile
│  │                    正式插件结构列为可选后续（M5），与合并解耦
│  │                    （demo 的 mihomo-mobile 不存在——ADR-0004 已单核心 sing-box）
│  ├─ Cargo.toml     ✏️ 单壳：base deps + target 依赖表差异化（见 §4.2）
│  ├─ tauri.conf.json         ✅ 合并为一份（桌面基线）
│  ├─ tauri.android.conf.json ✅ 新增 overlay：devUrl 端口、beforeDev/BuildCommand、
│  │                              bundle.active=false、android minSdk 等
│  └─ tauri.windows.conf.json ❌ 现状无 windows 专属差异（NSIS 在主配置），不需要
├─ panel-core/       ✅ apps/mobile/panel-core 随迁（Go module 与壳同 repo 内聚）
├─ scripts/          ✅ build-panel-core.sh / android-ndk-env.sh 随迁
└─ keystore/         ✅ 随迁（核对 .gitignore 与 CI secret 引用）
```

### 3.1 平台检测方式裁决（demo 提到 plugin-os / TAURI_ENV_PLATFORM / __TAURI_INTERNALS__）

| 方式 | 结论 | 适用面 |
|---|---|---|
| `TAURI_ENV_PLATFORM`（vite `envPrefix` 已暴露）+ vite mode | ✅ **主机制**：构建期常量，UI 树 / 依赖（HeroUI vs Konsta）/ react-compiler 开关全部按它 tree-shake 掉另一平台 | UI 入口、样式、页面级差异 |
| cargo `#[cfg(target_os)]` + target 依赖表 | ✅ Rust 侧主机制（编译期） | 命令注册、适配层、依赖裁剪 |
| `get_capabilities()`（现有能力矩阵） | ✅ 保留现状语义：运行时**功能**开关（非平台判断） | 如 desktop 上 mitm 环境性禁用 |
| `@tauri-apps/plugin-os` `platform()` | ⚠️ 可引入但预期几乎无消费方；为「运行时选 UI」而引入则**明确反对** | 个别运行时文案/行为微调（如有） |
| `window.__TAURI_INTERNALS__` | ❌ 仅用于「是否在 Tauri 环境」的兜底判断（desktop App.tsx 现有用法保留），不用于平台分发 | 浏览器直开 devUrl 的拦截提示 |

**理由**：ADR-0003 的核心教训是「运行时平台判断易遗漏，编译期事实不会」。合并后若用 runtime detect 选页面树，等于把刚消灭的 39 处判断请回来。

---

## 4. 目标架构（贴合现状调整版）

### 4.1 前端：单包双入口（vite mode 构建期分发）

```
apps/client/
├─ index.html              # 壳 HTML（不含主题预置脚本）
├─ src/
│  ├─ main.tsx             # 唯一入口：installLogCapture + QueryClient + import("@app/App")
│  ├─ desktop/             # = 现 apps/desktop/src（App.tsx / pages / layout / index.css）
│  └─ mobile/              # = 现 apps/mobile/src（App.tsx / pages / theme.tsx / index.css）
├─ vite.config.ts          # defineConfig(({ mode }) => …)：mode=desktop|android
├─ package.json            # 依赖并集；scripts: dev / dev:android / build / build:android / verify
└─ tsconfig.json / oxlint.config.ts   # 现状两 app 已完全一致，直接合一
```

要点：

- **alias 分发**：`resolve.alias["@app"] = mode === "android" ? "./src/mobile" : "./src/desktop"`；`main.tsx` 只写 `import("@app/App")` + `import("@app/index.css")`。HeroUI / Konsta / codemirror 只进入各自 mode 的 bundle（构建产物互不含对方 UI 库，包体积与现状一致）。
- **react-compiler**：desktop mode 开、android mode 关（维持现状差异；若想统一开启，列为独立验证任务，Konsta 兼容性需实测）。
- **index.html 主题预置脚本**：~~两端不同（`heroui-theme` vs `pp-ui-theme` 首帧脚本）~~ **已收敛为单一键**（2026-10 合并前置提交完成）：HeroUI v3 `useTheme` 的存储键 `heroui-theme` 硬编码不可配，属 UI 库耦合的历史遗留；主题管理已上移 `@pp/client-core` theme 模块，双端统一为 UI 无关键 `pp-ui-theme`（存量 `heroui-theme` 由模块加载时一次性迁移，首帧脚本保留只读回退兜底）。**合并后 index.html 为单一文件、单一首帧脚本，无需按 mode 注入/切换**。
- **端口**：mode 决定 dev 端口（desktop 1420 / android 1430），`tauri.android.conf.json` overlay 同步 devUrl + beforeDevCommand（`bun run dev:android`）。
- **package.json scripts**：`verify` = `build` + `build:android` + `lint` + `format:check`（CI 一次门禁双产物，防止「一端绿一端红」）。
- **包名**：`pp-client` 与 Rust crate 撞名但不同生态（bun vs cargo），不冲突；也可 `pp-client-app`。**注意 CI/release 与根 package.json 的 `--filter` 引用同步改**。

### 4.2 Rust 壳：单 Cargo 项目 + target 依赖表

```toml
# apps/client/src-tauri/Cargo.toml（骨架）
[lib]
name = "pp_client_lib"
crate-type = ["lib", "cdylib", "staticlib"]   # cdylib/staticlib 仅 Android 需要，桌面多产两种产物无害

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-notification = "2"
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
pp-client-tauri = { path = "../../../crates/pp-client-tauri" }   # base：scripts-only 形态

# 桌面（Linux/Windows/macOS）目标才激活：MITM / 脚本 / 核心管理链路全量依赖。
# pp-client-tauri 同名出现在两张表是合法 cargo 语义：target 命中时 feature 并集生效，
# Android 构建图不含 mitm feature → pp-mitm 不进入构建（I3 的正式表达）。
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
pp-client-tauri = { path = "../../../crates/pp-client-tauri", features = ["mitm"] }
pp-client = { path = "../../../crates/pp-client" }   # 默认 features 含 mitm
pp-mitm = { path = "../../../crates/pp-mitm" }
pp-script = { path = "../../../crates/pp-script" }
pp-common = { path = "../../../crates/pp-common" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
base64 = "0.22"
chrono = "0.4"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = "1"

[profile.release]
# Android 体积优化（strip/lto/codegen-units/opt-level=z）对桌面 release 同样成立或无害；
# 若桌面端不愿接受 opt-level=z 的运行性能折损，此 profile 迁往 CI android job 以
# CARGO_PROFILE_RELEASE_* 环境变量注入，而非写死在 Cargo.toml。
strip = true
lto = true
codegen-units = 1
```

`src/` 组织与 cfg 预算（预估 12-15 处，远低于 ADR-0003 时代的 94 处）：

- `lib.rs`：`run()` 共享装配（通知/opener 插件、共享命令全路径注册）→ `desktop::` / `mobile::` 适配层经 `#[cfg]` 模块声明裁剪；
- `desktop/mod.rs`（`#[cfg(not(any(android, ios)))]`）：现 desktop 壳 `commands/` 四模块 + `state.rs`（data_dir 桌面语义）+ WSL workaround + desktop 专属命令注册（core_mgmt/mitm/remote/tun 授权）；
- `mobile/mod.rs`（`#[cfg(target_os = "android")]`）：`resolve_data_dir`、`core_bridge::set_data_dir` 注入、vpn 插件注册、Android 三命令注册；
- `main.rs`：桌面 host 入口调 `run()`（mobile 由 `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 注入，现状机制不变）。

### 4.3 配置与资源合并

| 项 | 处理 |
|---|---|
| `tauri.conf.json` | 以 desktop 版为基线（bundle 激活 + NSIS + icons）；devUrl/beforeDevCommand 为桌面值 |
| `tauri.android.conf.json` | overlay：`build.devUrl`=1430、`beforeDevCommand`/`beforeBuildCommand`=android scripts、`bundle.active`=false、`bundle.android.minSdkVersion`=33。**注意 RFC 7396 数组合并语义：数组整体替换而非合并**（`bundle.icon` 若 overlay 置空会丢桌面图标配置——overlay 中不写该键即可） |
| `capabilities/default.json` | 两端实测仅 description 注释不同 → 直接合一 |
| `icons/` | 合一（desktop 图标集为超集；Android 图标在 gen/android res 内，不受影响） |
| `gen/android` | 整体随迁（含手写 Kotlin 与 gradle 定制；`tauri android init` 不再重跑） |
| 双 Cargo.lock / target/ | 合一：CI rust-cache 与本地构建缓存直接受益 |

### 4.4 不动的部分

- `packages/client-core`（@pp/client-core）：保持独立 Bun workspace 包；
- `crates/pp-client` / `crates/pp-client-tauri`：层级与 feature 门控不变（合并反而强化了它们的分层正当性）；
- `apps/panel`：管理系统，与本任务无关。

---

## 5. 构建 / CI / CD 改造清单

### 5.1 本地门禁与脚本

| 文件 | 改动 |
|---|---|
| `scripts/check-rust-gates.sh` | 双壳两步并为单壳两步：host clippy（桌面 feature 全开）+ android target check（aarch64，覆盖 mobile cfg）；步骤 4/5 合并 |
| `.husky/pre-commit`（lint-staged 段） | 前端过滤由 `apps/desktop/**`、`apps/mobile/**` 改为 `apps/client/**` 跑单包 verify；文件规模门禁路径同步 |
| 根 `package.json` | workspaces 由 `apps/*` 自然覆盖；`--filter pp-client-ui` / `pp-client-mobile-ui` 引用点（CI、文档）改新包名 |
| `flake.nix` | 基本不动（dev shell 与 target 无关）；仅注释中路径描述更新 |

### 5.2 CI（.github/workflows/ci.yml）

- `client-shells` job：三个 step（desktop clippy / mobile host clippy / mobile android check）→ 两个 step（host clippy 单壳 / android target check 单壳）；
- `web` job：`pp-client-ui` + `pp-client-mobile-ui` 两次 verify → 新包一次 verify（其内部已含双 mode build）。

### 5.3 Release（.github/workflows/release.yml）

- `desktop-windows`：`projectPath: apps/desktop` → `apps/client`，rust-cache workspace 路径同步；产物路径 `apps/client/src-tauri/target/release/bundle/nsis/*.exe`；
- **机会项（建议随合并一并做）**：新增 `android-apk` job——ubuntu-latest + NDK/Go 环境（action 复用 flake 或显式 setup-go + android sdk）→ `build-panel-core.sh` 出 AAR → `tauri android build --target aarch64`（需 keystore secret：当前 keystore 在库外/库内状态需先确认）。现状没有 Android 发布 job，合并后「一个 app 两个产物」的发布语义需要它补齐；
- 未来扩展桌面 Linux/macOS bundle 时，`tauri-action` 的 `projectPath` 同样指向 apps/client，矩阵化成本低于双 app 形态。

### 5.4 文档同步

- `AGENTS.md`：项目概述（双客户端 → 单客户端双入口）、构建命令、双壳盲区提醒改写为「单壳双 target」表述；
- 新增 **ADR-0007**（客户端合并回摆）：引用 ADR-0003，说明哪些分离收益被保留（UI 分目录、命令分层）、哪些形态回摆（单壳单 conf），以及 I1-I3 不变式。ADR-0003 状态标记 Superseded（部分）；
- `docs/development.md` Android 构建章节路径更新。

---

## 6. 迁移里程碑

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M0 验证钉（spike，先于一切）** ✅ 已通过（2026-10-03，证据见 §6.1） | 三个未知项各自最小验证：① Cargo.toml target 依赖表 + `pp-client-tauri/mitm` feature 在 android target 构建图中确实排除 `pp-mitm`（`cargo tree --target aarch64-linux-android -i pp-mitm` 无输出）；② `tauri.android.conf.json` overlay 被 `tauri android dev/build` 正确合并（devUrl/beforeBuildCommand 生效）；③ vite mode alias 双构建产物互不含对方 UI 库（`grep -r konsta dist-desktop/` 为零等） | 三项全部实证通过；任一失败则降级方案（见 §7） |
| **M1** | 建 `apps/client` 壳：desktop src-tauri 为基座 + mobile 适配层迁入 + target 依赖表 + gen/android/keystore/panel-core/scripts 随迁 + tauri.android.conf.json | host clippy + android cargo check 通过；desktop 可 dev 启动（UI 暂用 desktop 目录直接挂） |
| **M2** | 前端合并：双目录迁入 + vite mode + 单 package.json/tsconfig/oxlint + 单一 index.html（主题键已前置统一，无 per-mode 注入） | `verify` 全绿；desktop dev / android dev 双端页面与行为零变化（对照截图/手测清单） |
| **M3** | CI/CD/脚本/文档切换（§5 全表） | CI 全绿；release dry-run（windows NSIS + android APK 产物齐全） |
| **M4** | 删除 `apps/desktop`、`apps/mobile`；ADR-0007 落地，ADR-0003 标记 Superseded | 仓库无残留引用（`rg "apps/desktop\|apps/mobile"` 仅剩历史文档） |
| **M5（可选，解耦）** | VPN 插件抽取为正式 tauri 插件结构（`src-tauri/plugins/sing-box-mobile/`：Rust 插件 crate + android/ gradle 子模块） | Android 功能零回归；gen/android 内手写 Kotlin 仅剩 app 胶水 |

每个里程碑独立可合入；M1/M2 之间保持双 app 可用（旧目录暂不删）以便对照验证。

### 6.1 M0 实证记录（2026-10-03，spike 工程位于 `tmp/m0-spike/`，不入库）

| 验证项 | 方法 | 结果 |
|---|---|---|
| ① target 依赖表 + feature 裁剪 | spike 壳（`tmp/m0-spike/shell`）：`[dependencies]` 基线引用 `pp-client-tauri`（scripts-only），`[target.'cfg(not(any(android, ios)))'.dependencies]` 同名引用加 `features=["mitm"]` 并直挂 `pp-client`/`pp-mitm`/`pp-script`；分别跑 `cargo tree -i pp-mitm`（host）与 `cargo tree --target aarch64-linux-android -i pp-mitm` | ✅ host：pp-mitm 经三条边（直接依赖 / pp-client 默认 features / pp-client-tauri mitm feature）在图内；android target：`nothing to print`，pp-mitm 不在构建图。**I3 成立** |
| ② Tauri 平台 overlay 合并 | spike 壳 `tauri.conf.json`（`beforeDevCommand: echo M0_MARKER_BASE_CONFIG`）+ `tauri.linux.conf.json`（overlay 为 `echo M0_MARKER_LINUX_OVERLAY`）；`tauri dev` 观察实际执行的 beforeDevCommand；对照组移除 overlay 重跑 | ✅ 有 overlay 时输出 `M0_MARKER_LINUX_OVERLAY`，无 overlay 时输出 `M0_MARKER_BASE_CONFIG`（RFC 7396 合并生效）。`tauri.android.conf.json` 为同一机制（CLI 按目标平台选择 overlay 文件），残留风险仅 android 子命令的路径差异，M1 时随 `tauri android dev` 首跑复核 |
| ③ vite mode alias 双产物隔离 | spike 前端（`tmp/m0-spike/web`）：`vite.config.ts` 按 mode 切换 `@app` alias（desktop→HeroUI+react-compiler，android→Konsta），两入口各带标记字符串与各自 UI 库 CSS；分别构建 | ✅ desktop 产物含 `__M0_DESKTOP_ONLY__` + HeroUI 主题 token（css 448KB），android 产物含 `__M0_MOBILE_ONLY__` + Konsta 样式（css 125KB），交叉 grep 均为零命中。**双 UI 库产物级隔离成立** |

另注：spike 中 bun 隔离式 node_modules 对非 workspace 目录不自动可见，spike 以 symlink 方式引入依赖；正式 apps/client 作为 workspace 成员无此问题。

### 工作量粗估

| 项 | 量级 |
|---|---|
| M0 spike | 0.5 天（决定成败） |
| M1 壳合并 | 1-1.5 天（cfg 裁剪 + 依赖表调试为主） |
| M2 前端合并 | 1 天（纯搬迁为主，vite mode 调试为辅） |
| M3 CI/CD/文档 | 0.5-1 天（android APK job 若做签名另加） |
| M4 清理 | 0.5 天 |

---

## 7. 风险与回退

| 风险 | 概率 | 缓解 / 回退 |
|---|---|---|
| target 依赖表 feature 并集行为与预期不符（android 构建仍链入 pp-mitm） | 低 | M0 第一项即实证；失败则降级：pp-client 侧先把 mitm 移出 default features（原本就在 backlog），或以 `#[cfg]` + 空实现模块兜底 |
| `tauri android dev` 对 overlay 的 beforeDevCommand/devUrl 合并有版本差异 | 低-中 | M0 第二项实证；失败则退化为两个 tauri.conf 由脚本 `--config` 显式指定（`tauri android dev -- --config` 路径），仍可单壳 |
| 双 UI 库进同一 node_modules 导致类型/构建互相污染（HeroUI v3 + Konsta 共存） | 低 | 两者无共享类型冲突史；vite mode alias 保证产物隔离；`tsc -b` 双 tsconfig 若冲突则每 mode 独立 tsconfig include |
| 合并期双端并行开发冲突（单文件锁竞争，如 package.json / tauri.conf.json） | 中 | 里程碑短周期合入；合并窗口内冻结双端 feature 提交 |
| Android 发布签名链路首次进 CI | 中 | keystore 现状先审计（库内 `apps/mobile/keystore/` 是否含真实密钥）；CI 用 secrets 注入，本地构建路径保留 |
| 回摆 ADR-0003 的决策反复成本 | — | ADR-0007 明确记录「保留分离收益、回摆壳层形态」的边界，避免下次再翻烧饼 |

**整体回退点**：M0 失败或 M2 验证双端行为有回归 → 停在当前双 app 形态（零损失）；M3 前任何时刻 apps/desktop / apps/mobile 均未删除，git revert 单里程碑即可。

---

## 8. 决策建议

**建议做，按 §6 里程碑推进，M0 先行。** 与 2026-09 相比，合并的三个技术前提今天才成立：共享命令层已抽干（壳内只剩平台组装）、desktop 专属命令已模块化（可整模块 cfg 裁剪而非函数内分支）、cargo target 依赖表可正式表达「Android 无 MITM」。满足 I1-I3 不变式的合并不是回到 ADR-0003 之前，而是把「双 app 两仓」升级为「单 app 双编译目标」。

若评审认为「双端 UI 独立迭代互不牵制」的权重高于壳层合一的维护收益，则维持现状也是合理选择——现状双壳没有正确性债务，合并是纯维护性/工程化优化。
