# ADR-0007: 客户端合并为单壳双目标应用（apps/client）

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** ProxyPanel Contributors
- **Scope:** `apps/desktop` + `apps/mobile` → `apps/client`、`packages/client-core`、CI/CD 与构建脚本
- **Supersedes:** ADR-0003 的「双 app 两壳」形态（其分离收益由编译期机制保留，见 §2）
- **评估依据：** `docs/plans/2026-10-03-client-merge-evaluation.md`（含 M0 实证记录 §6.1）

---

## 1. Context

ADR-0003（2026-09）将客户端拆为 `apps/desktop` 与 `apps/mobile` 两个独立 Tauri 应用，
解决了当时单壳的五个痛点（39 处前端 `isAndroid` 运行时判断、94 处壳内 cfg、CSS 断点
伪装平台适配、Tauri CLI 无 per-target feature 导致的 Android 禁 MITM hack、移动交互
无法系统设计）。拆分运行一个月后，双壳形态的维护成本显现：两份 `tauri.conf.json`、
两个壳 Cargo 项目（双 lockfile / 双 target 缓存）、CI 壳层三步门禁、壳层样板
（日志初始化 / 插件注册 / 共享命令注册表）双份且需人工防漂移。

与此同时，合并的三个技术前提在拆分期已就绪：共享命令层 `pp-client-tauri` 已抽干
（壳内只剩平台组装）、桌面专属命令已模块化（可整模块 cfg 裁剪）、cargo target 依赖表
可正式表达「Android 无 MITM」（M0-① 实证）。

## 2. Decision

**合并为单一 Tauri 应用 `apps/client`（单包双入口 + 单壳双目标）**，遵守三条不变式：

| 不变式 | 内容 | 实证 |
|---|---|---|
| I1 | UI 选择是**构建期**事实：vite mode（`desktop` 默认 / `android`）经 `@app` alias 分发到 `src/desktop` 或 `src/mobile`；禁止运行时 `isAndroid` 分支选择页面树 | M0-③：双产物互不含对方 UI 库（标记字符串 + CSS 交叉 grep 零命中） |
| I2 | Rust 平台差异是**编译期**事实：`src/desktop`、`src/mobile` 适配层由 lib.rs 以 `#[cfg]` 门控模块声明，平台专属命令在 `generate_handler!` 中逐条 cfg | M1：host clippy + android cargo check 双过 |
| I3 | Android 构建图**不含** `pp-mitm`：`Cargo.toml` target 依赖表（`cfg(not(any(android, ios)))` 才激活 `pp-client-tauri/mitm` 与 pp-client/pp-mitm/pp-script 链路） | M0-① + M1 复验：`cargo tree --target aarch64-linux-android -i pp-mitm` 无输出 |

**ADR-0003 的哪些收益被保留**（合并不是回到拆分前）：

- UI 分目录独立存在（`src/desktop` HeroUI / `src/mobile` Konsta），移动交互体系不回退；
- 共享命令层 `pp-client-tauri` 与前端共享库 `@pp/client-core` 层级不变；
- 「前端运行时平台判断」未回归（合并后前端仍为 0 处 `isAndroid` 页面分支）。

**回摆的部分**：双壳双 conf → 单壳 `tauri.conf.json`（桌面基线）+ `tauri.android.conf.json`
overlay（RFC 7396 合并：devUrl 1430 / `dev:android`·`build:android` 命令 / bundle 关闭 +
minSdk 33；M0-② 与 M1 实证 overlay 在 `tauri android build` 生效）。

### 2.1 关键实现细节

- **前端**：单 `package.json`（依赖并集，`pp-client-app`）、单 tsconfig/oxlint 配置
  （两端本就一致）、单一 `index.html`（主题存储键已前置统一为 UI 无关的
  `pp-ui-theme`，见 client-core `theme.tsx`）；`verify` = desktop build +
  android build + lint + format:check（一次门禁双产物）。
- **壳**：crate-type 增加 `cdylib`/`staticlib`（Android 构建要求）；release profile
  保留 `strip`/`lto`/`codegen-units=1`，Android 专属 `opt-level="z"` 由
  `android:build` 脚本经 `CARGO_PROFILE_RELEASE_OPT_LEVEL` 注入（不损桌面 release
  性能）。
- **数据目录**：桌面 `$HOME/.proxy-panel-client`（desktop 适配层）；Android
  `app_data_dir()`（mobile 适配层）——两端 data_dir 语义不变，存量用户数据无迁移。
- **Android 资产**：`gen/android`（Kotlin VPN 插件工程）、`panel-core`（Go module）、
  `scripts/`、`keystore/` 随迁至 `apps/client/`。

## 3. Alternatives Considered

| 方案 | 否决理由 |
|---|---|
| 维持双 app（ADR-0003 形态） | 双 lockfile / 双 conf / CI 三门禁的维护成本持续存在；见 §1 |
| 单壳 + 运行时平台检测选 UI（demo 原始提议：plugin-os / `__TAURI_INTERNALS__`） | 重现 ADR-0003 问题 1（运行时判断易遗漏）；UI 树分发必须构建期完成 |
| 共享前端内联为 `src/shared/` | `@pp/client-core` 已是独立 verify 单元与契约载体，内联是倒退 |

## 4. Consequences

**正面**：单 `tauri.conf.json`、单壳 Cargo 项目（单 lockfile、单 target/ 缓存）、
CI 壳层门禁三步骤并两步、版本号/identifier 天然一致、壳层样板单份化、
release 矩阵扩展（未来桌面 Linux/macOS bundle、Android APK job）矩阵化成本更低。

**负面 / 成本**：壳内重新引入有预算的 cfg（实测 12 处，远低于拆分前的 94 处）；
单一 `node_modules` 同时安装 HeroUI 与 Konsta（产物经 mode 隔离，无互染）；
双端共享文件（package.json / tauri.conf.json）的并行修改冲突概率上升。

**后续（未做，独立任务）**：Android 发布 job 进 release.yml（需先确定 APK 签名策略：
当前 keystore 仅为 debug 参考）；VPN 插件抽取为正式 tauri 插件结构（评估 M5 可选项）。

## 5. References

- 评估与 M0/M1 实证：`docs/plans/2026-10-03-client-merge-evaluation.md`
- 被部分取代：ADR-0003（desktop/mobile 双应用分离）
- 相关：ADR-0004（单核心 sing-box）、ADR-0005（配置切片）
