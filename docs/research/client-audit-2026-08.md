# 客户端审计与平台策略（2026-08）

> 输入：pp-client 核心库 / apps/desktop / apps/android 三份只读审计 + husi、FlClash 等参考项目调研
> 结论先行：**桌面/移动不拆分代码库、Android 放弃 MITM、移动端收敛 sing-box 单核（mihomo 转为静默 fallback）**

---

## 1. 现状全景

### 架构分层（健康）

```
apps/desktop（Tauri 壳：命令注册/视图转换/平台适配）
    ↓ CoreEngineBridge（安卓）/ 进程 spawn（桌面）
crates/pp-client（订阅、配置合成、Profile 复写、核心生命周期）
    ↓
crates/pp-mitm + pp-script（桌面增值：MITM 抓包/重写、QX 脚本生态）
```

- `CoreEngineBridge` 抽象有效隔离了桌面/安卓的核心生命周期差异（桌面 spawn 二进制、安卓桥接 Kotlin VpnPlugin）
- 安卓端禁用路径集中且干净：`state.rs` 的 `apply_android_overrides`（强制关闭 mitm/系统代理/配置层 TUN）

### 关键事实

| 事实 | 来源 |
|---|---|
| 双内核在安卓均已可用：sing-box TUN 实现成熟（完整 DefaultInterfaceMonitor/per-app 代理），mihomo 为"P1 spike 最小实现" | android 审计 |
| `apply_android_overrides` 已强制 `mitm_enabled=false`，方向正确 | pp-client `state.rs` |
| 63 处 `cfg(target_os="android")` 散布、`commands.rs` 2500+ 行、`CoreEngineBridgeAdapter` 有安卓无意义的桩方法 | pp-client/desktop 审计 |
| 安卓 MITM 页（/mitm）未做平台判断，仍在移动端展示不可用的功能 | desktop 审计 |
| 业界单核趋势：FlClash/Karing/CMFA/Streisand 全部单核；NekoBox 多核插件化被普遍认为增加困惑；husi 实为"sing-box 单核+协议插件 APK" | 调研 |

---

## 2. 决策一：desktop 与 mobile 是否分开迭代？

**结论：不拆分代码库，改为"单代码库 + 显性平台分层"；中期若移动端要原生体验，复用 pp-client 库重写 UI 即可。**

理由：
- 订阅/配置合成/Profile 复写/节点转换是两端 100% 共享的逻辑，拆成两个代码库等于双份维护
- 当前最大的痛不是"在一起"，而是平台边界不够显性（页面在移动端展示不可用功能、命令层未排除）
- pp-client 已是独立 crate，未来若做原生 Android UI（Compose），库可直接复用——拆分成本被架构设计对冲了

行动项：
- [ ] 建立平台特性矩阵（feature × platform），前端按矩阵隐藏页面/卡片（MITM、系统代理、核心管理在 Android 隐藏）
- [ ] Rust 命令层对移动端不可用命令返回明确的 `unsupported_platform` 错误（而非行为未定义）
- [ ] 收敛 `cfg(target_os)` 散布：系统代理/提权/核心下载归入 `SystemProxy`、`PrivilegeChecker`、`CoreDownloader` 等 trait，安卓侧为 no-op 实现
- [ ] `pp-mitm`/`pp-script` 加 Cargo feature，安卓构建裁剪以减小体积
- [ ] `commands.rs` 纯逻辑（校验/视图转换）下沉 pp-client

## 3. 决策二：Android MITM 还值得做吗？

**结论：不值得，正式放弃。桌面端 MITM 保留为差异化卖点。**

技术判定（无 root）：
- Android 7+ 应用默认不信任用户 CA；系统 CA 需 root
- VpnService 拿到的是 IP 层流，现有 pp-mitm（hudsucker HTTP 代理）架构没有流量路径可把流量导进去
- 桌面"系统代理→mixed 入口→MITM outbound"模式在 Android 无等效机制

行动项：
- [ ] Android 隐藏 MITM 页面入口（导航与路由守卫）
- [ ] 评估把 **cron 定时任务从 MITM 链路解耦**：脚本签到类任务只需 HTTP 客户端（`http_exec.rs` 已有 reqwest 实现），不依赖抓包；解耦后安卓端也能跑定时任务（QX 生态里移动端最实用的一块）
- [ ] docs 中明确标注 MITM 为桌面端能力

## 4. 决策三：移动端双内核去留与重设计

**结论：收敛为"sing-box 主核 + mihomo 静默 fallback"，不对用户暴露内核选择。**

依据：
- 移动端没有成功的"用户自选内核"案例；单核深耕是业界共识
- sing-box 安卓 TUN 实现成熟、libbox 的 PlatformInterface 抽象完整；mihomo 侧 TUN 是 spike
- mihomo 存在的唯一硬理由是 Clash 订阅格式兼容（sing-box 解析部分 clash 节点有已知问题）——用自动检测 + 静默切换解决，而非让用户选

### 交互重设计（融合 husi + FlClash 调研）

**分层渐进式规则管理：**
1. 场景模板（回国/海外/广告过滤，一键生成规则集）
2. 可视化规则卡片：摘要截断显示、拖拽排序、滑动删除（5s 撤销）、一键开关；编辑走 Sheet 渐进导航（类型→目标→高级折叠）
3. JSON 兜底：每条规则可自定义 JSON 覆写（husi ConfigEditScreen 式 schema 补全编辑器）

**配置生成（人类友好）：**
- Hub 服务端生成基础配置（节点/路由/DNS 齐全）
- 客户端只维护轻量本地 Override（如 per-app 直连、自定义 hosts），启动时合并
- 规则集订阅化：内置社区规则集市场（勾选即订阅、自动更新），不向普通用户暴露 rule_providers/rule_set 语法差异

行动项：
- [ ] 移动端隐藏内核切换 UI，默认 sing-box；订阅嗅探到 sing-box 不兼容的 clash 节点时自动切 mihomo（日志记录原因）
- [ ] `check_subscription_core_compat` 的硬限制改为自动降级而非报错
- [ ] 本地 Override 层设计（schema + 合并策略 + UI）
- [ ] 规则卡片列表 + 场景模板（移动端优先，桌面端复用）
- [ ] mihomo Kotlin 侧补全或冻结：若走 sing-box 主核路线，`MihomoVpnService` 标记为 fallback 维护模式（只修崩溃，不加功能）

---

## 5. 参考案例索引

| 项目 | 借鉴点 | 源码位置 |
|---|---|---|
| husi | 规则编辑 Sheet、JSON schema 编辑器、协议插件化、路由规则摘要 | `.reference/husi/composeApp/.../RouteSettingsScreen.kt`、`libcore/distro/registry.go` |
| FlClash | 三级覆写模式（standard/script/custom）、全局规则库跨配置复用 | `.reference/FlClash/lib/views/profiles/overwrite/` |
| Karing/Streisand | sing-box 单核的稳定性收益 | — |
| NekoBox | 多核插件化的反面教材（用户困惑） | — |

*生成于 2026-08，基于 kimi-for-coding 子代理的四份并行审计。*
