# ADR-0010: 桌面核心收敛为内嵌 pinned 核心（FlClash 模式）

- **Status:** Accepted
- **Date:** 2026-10-08
- **Deciders:** ProxyPanel Contributors
- **Scope:** `crates/pp-client`（cores）、`apps/client/src-tauri`（core_mgmt 命令）、客户端核心管理 UI、`apps/client/src-tauri/seed-manifest.json`、发布流程
- **Related:** ADR-0008（Windows 种子核心 D5，本决策的机制基础）、ADR-0009（仓库边界）、ADR-0005（sing-box >= 1.14 基线）

---

## 1. Context

桌面端核心（sing-box）长期为**双轨分发**：ADR-0008 引入的安装包内嵌种子核心
（pinned 版本，首启释放）+ 运行时通道下载（稳定/测试/预发布三通道，GitHub Releases
拉取）。Android 端则始终是内嵌 pinned 形态（`panel-core` Go libbox，随 APK 构建）。

双轨的实际维护成本已被反复实证：

- **GitHub 可达性故障类**：远端通道查询静默失败（检查更新"无响应"）、慢速直连下载
  被 30s 全局超时砍断、wintun.dll 运行时拉取失败——这一整类问题只存在于运行时下载轨；
- **下载链路维护面**：`pp-client::cores` 的 HTTP 客户端 / 镜像回退 / 通道模型 /
  版本探测 / wintun 随附下载与对应 UI（通道卡片、检查更新、下载按钮）；
- **版本钉分叉**：种子 1.14.2 与 panel-core 的 sing-box 1.15.0-alpha.5 已经漂移，
  双端行为差异（如配置合成断言的版本基线）难以陈述。

ADR-0008 已交付两项关键前提：**种子机制**（pinned 核心随安装包分发并释放）与
**自动更新**（updater，核心更新可随 App 版本送达用户，无需用户手动下载）。这使
"核心更新必须发版"的代价被大幅对冲。

## 2. Decision

**桌面核心收敛为仅内嵌 pinned 核心（FlClash 模式）**，与 Android 形态对齐：

### D1 移除运行时通道下载

删除 `pp-client::cores` 的下载链路（`download()` / 远端通道查询 / 通道模型 /
镜像与代理前缀接入 / wintun 运行时拉取）与对应 UI（版本通道区、检查更新按钮、
下载/更新按钮）。`github_proxy_prefix` 设置保留（远端资源与规则集下载仍使用），
但核心分发不再经过它。

### D2 种子从"首启补白"升级为"版本同步"

`seed_bundled_core` 语义扩展：启动时确保**当前构建内嵌的 pinned 版本**存在且为
活跃核心——App 升级带入新版本种子时自动释放并切换；用户手动删除活跃核心后
下次启动可再释放。不再保留多版本并存管理 UI。

### D3 版本钉单一来源

桌面种子版本（`seed-manifest.json`）与 Android panel-core 的 sing-box 版本
（`apps/client/panel-core/go.mod`）建立**单一来源陈述**（发版检查清单要求同基线
提升并记录差异理由）；两端的 sing-box 基线约束（ADR-0005 的 >= 1.14）继续适用。

### D4 存量清理

存量 `data_dir/cores/` 下的非 pinned 版本在升级后 best-effort 清理（或保留但不可选，
以简单者为准，实施时确定）；`client.json` 的 `core_binary` 若指向被清理版本，
回落到 pinned 核心。

## 3. Consequences

**正面**：

- GitHub 可达性故障类（通道查询、下载超时、镜像回退、wintun 拉取）在核心分发上
  整体消失；首启/升级体验确定性大幅提高；
- 删除约一个子系统量的代码与 UI（cores 下载链路、通道卡片、检查更新），核心管理
  页收敛为"当前核心 + 版本"展示；
- 双端核心版本陈述统一，配置合成的版本基线断言可 written down。

**代价**：

- 用户失去 stable/beta/alpha 通道选择与"只升级核心不升级 App"的路径；核心 hotfix
  必须发版（由 updater 对冲，但发布动作本身成为关键路径）;
- 发版流程新增职责：提升 sing-box pinned 版本时须同步种子 manifest 与 panel-core
  go.mod 并跑双端冒烟；
- 桌面失去"用系统/自定义 sing-box 二进制"的既有便利（探测系统核心已于上轮移除，
  本决策是该方向的延续）。

**回摆条件**：若出现强烈的"核心独立升级"需求（如 sing-box 安全补丁等不到 App 发版），
以高级"自定义核心二进制路径"形式回摆（A2 预案），不恢复通道下载。

## 4. Alternatives considered

- **A2 种子默认 + 下载降级为高级项**：保留下载链路作为"自定义核心"入口。维护成本
  基本全保留（HTTP/镜像/超时/UI 一个不少），与"减少维护成本"的目标不符，拒绝；
  仅作为回摆预案记录。
- **libbox FFI 统一引擎（桌面也内嵌 Go 引擎）**：彻底消灭子进程与路径/控制台类问题，
  但需重写 pp-core 集成、解决 cgo 三桌面平台编译打包签名，相对 D1-D4 边际收益小、
  风险大，本期拒绝；若未来子进程模型再次成为主要痛点，单独评估。
