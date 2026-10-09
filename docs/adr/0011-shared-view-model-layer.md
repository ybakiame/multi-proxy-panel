# ADR-0011: 客户端 UI 共享视图模型层（View-Model 下沉至 client-core）

- **Status:** Accepted
- **Date:** 2026-10-08
- **Deciders:** ProxyPanel Contributors
- **Scope:** `packages/client-core`、`apps/client/src/desktop`、`apps/client/src/mobile`
- **Related:** ADR-0007（单壳双目标，双 UI 树与编译期分发）、ADR-0002（规则管理交互）

---

## 1. Context

ADR-0007 合并为单壳后，桌面（HeroUI）与移动（Konsta）维持**镜像但独立的两棵页面树**
（实测 desktop 58 tsx / 10.8k 行，mobile 52 tsx / 7.8k 行，仅 17 个同名文件）。
`@pp/client-core` 已集中 api 封装、基础 hooks 与 atoms，但**页面级视图模型**
（表单状态、草稿同步、persist 链、脏标记上报等）仍双端各写一份。

双份视图模型的漂移成本已被实证：桌面 `useSettingsConfig` 的渲染期同步哨兵 bug
（移动端早已修复，桌面端漂移未跟进），表现为设置表单显示与持久化值不符
（2026-10 的「TUN 显示关闭但实际开启」事件）。路由管理等页面组织也在靠人工
双端对齐。

2026-10-08 的方向讨论对比了三条路线：U1 单 UI 库统一（Konsta 全端，真·单 UI 树）、
U2 共享视图模型层（逻辑单源、标记双端）、U3 细粒度 `.desktop.tsx/.mobile.tsx`
覆盖约定。结论：**先做 U2**；U1 属产品审美决策（桌面观感退让换维护收益），待
spike 评估后另立 ADR；U3 引入第二套平台分发机制，暂缓。

## 2. Decision

**页面级视图模型单源化**：双端同构的页面逻辑（query/mutation 编排、表单草稿与
渲染期同步、persist 串行链、重启脏标记、toast 语义）下沉到 `@pp/client-core`，
平台组件只保留 UI 库标记与平台专属字段。

### D1 归属约定

满足以下全部条件的逻辑**必须**放 client-core 共享 hook：

- 双端页面语义相同（同一配置键、同一命令、同一校验规则）；
- 不含 UI 库类型依赖（不 import HeroUI/Konsta）；
- 以 client-core 既有 api/atoms/keys 为输入输出。

平台专属字段（如桌面 TUN 授权态、移动 VPN 通知偏好）留在平台 hook，经**组合**
（平台 hook 调用共享 hook 并扩展返回值）而非复制共享逻辑。

### D2 试点与迁移顺序

以 `useSettingsConfig`（设置页视图模型，双份 296/354 行，漂移实证现场）为试点
完成首次下沉，验证「共享核心机制 + 平台扩展字段组合」的形态；之后按页面逐步
迁移（路由/出站/DNS 切片编辑、规则管理、核心状态等），不做一次性大迁移。

### D3 双端同名 hook 单一权威

同一语义 hook 在 client-core 落地后，平台侧旧实现删除；禁止再出现"移动端修过、
桌面端没修"类漂移（此类历史：2026-10 桌面 useSettingsConfig 同步哨兵 bug）。

## 3. Consequences

**正面**：

- 直接消灭已实证的漂移类 bug；视图模型修复双端同时生效；
- 平台页面变薄（预计页面行数再降 20-30%），双端对齐从"人工同步"变为"默认一致"；
- 为可能的 U1（单 UI 库）铺路：U1 若成行，视图模型层无需再动。

**代价**：

- client-core 体积与职责扩大，需要维持"无 UI 依赖"纪律（oxlint import 规则或
  review 约定）；
- 组合模式下平台 hook 的返回值类型需要手工保持向后兼容（页面消费面不随之大改）；
- 迁移是逐页进行的，过渡期内新旧形态并存。

## 4. Alternatives considered

- **U1 单 UI 库统一（Konsta 全端）**：维护收益上限最高，但桌面失去密集表格/多列/
  悬停等桌面向交互，观感接近"放大的手机 App"，且是对 ADR-0007「双端交互体系
  不回退」的直接回摆——属产品审美决策，需 spike 验证观感后另立 ADR，本期不做。
- **U3 细粒度 `.desktop.tsx/.mobile.tsx` 覆盖约定**：对 80%+ 同构页面有用，但与
  现有 vite mode 整树分发形成两套平台机制并存，复杂度收益比不佳，U2 完成后再评估。
- **维持双份 + 纪律约束**：已被漂移实证不可行，拒绝。
