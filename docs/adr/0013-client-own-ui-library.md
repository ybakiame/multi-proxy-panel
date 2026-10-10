# ADR-0013: 客户端自研组件库与页面树合一（@pp/ui，编译期分发）

- **Status:** Accepted
- **Date:** 2026-10-10
- **Deciders:** ProxyPanel Contributors
- **Scope:** `packages/ui`（新增）、`packages/client-core`、`apps/client`
- **Related:** ADR-0007（单壳双目标）、ADR-0011（共享视图模型层，U1/U2/U3 分析）、设计文档 `docs/plans/2026-10-10-client-ui-library-design.md`

## Context

ADR-0007 合并单壳后，桌面（HeroUI v3）与移动（Konsta UI）维持两棵镜像但独立的页面树。ADR-0011 采纳 U2 将视图模型下沉至 `@pp/client-core`，但 UI 层仍双份实现，漂移持续发生。

最新实证：移动端规则管理具备完整「指定出站」能力（三源并集出站候选 + 动作分段控件），桌面端 `RuleEditModal` 显式过滤 `outbound` 动作（ADR-0005 D3 后置）。数据层早已统一，漂移纯在 UI 层。

ADR-0011 对比过 U1（Konsta 全端单 UI 树，暂缓——桌面观感退让不可接受）与 U3（`.desktop/.mobile` 后缀覆盖，暂缓——两套平台分发机制并存）。本次提出第四条路线：**自研组件库**——基于 headless 库 + Tailwind CSS v4 二次开发，只实现当前实际用到的组件，编译期决定组件形态，UI 完全自控，最终页面树合一。

驱动因素：

- HeroUI/Konsta 双库 = 两套 API 记忆 + 两份形态实现，功能上线天然双端异步；
- 移动端已出现自有适配层（`mobile/components/ui.tsx` 以 HeroUI 风格 props 包 Konsta），说明「自控 UI 接口」的需求已在实践中自发出现；
- 实际用到的组件子集有限（桌面约 20 种、移动壳层惯用法约 10 处），自研成本可控；
- 自研后形态分叉从「整棵页面树」收敛到「少数 T2 组件」，页面树合一成为可能——这是 U1 的维护收益，但不付 U1 的观感代价（桌面形态不退让）。

## Decision

新增 `packages/ui`（`@pp/ui`），React + Tailwind CSS v4 自研组件库，**Base UI** 为 headless 底座（验证不通过时以 React Aria Components 为后备，底座隔离在包内部）。

**D1 三层组件分类**：

- T0 纯样式（Card/Chip/Spinner/Label 等）：双端同实现；
- T1 headless 封装（Button/Switch/Input/Tabs/Toast 等）：共享逻辑 + 平台皮肤；
- T2 形态分叉（Overlay=Modal/BottomSheet、Select=dropdown/sheet-picker、DataList=Table/卡片、Shell=Sidebar/TabBar+Navbar+FAB）：同一 props 契约，编译期选形态。

**D2 编译期分发**：vite `define` 注入 `__PP_PLATFORM__`，组件内 `IS_MOBILE` 常量分支 + 内部子模块拆分，DCE 裁死分支。不引入 `.desktop/.mobile` 后缀 resolve 插件（维持 ADR-0011 对 U3 的拒绝）。

**D3 设计令牌**：沿用 HeroUI 语义令牌名（`bg-surface`/`text-muted` 等，Tailwind v4 `@theme` 实现），桌面迁移换 import 不换 class。三层作用域：`:root` 浅色 / `.dark` 深色（不动 client-core 主题模块）/ `[data-ui-style=ios|material]` 移动双主题（沿用 `pp-ui-style` 存储键）。Material 波纹后置，首期统一按压态。

**D4 依赖收敛**：HeroUI 与 Konsta **全部移除**；Konsta 壳层惯用法（TabBar/Navbar/FAB/Segmented/List）自研。Base UI 无 Table；BottomSheet 使用 Base UI 1.9 Drawer（原生滑动关闭）；DataList 列定义形状镜像 TanStack Table 以备日后接入。

**D5 页面树合一**：单一权威路由表 `sharedRoutes + IS_MOBILE 平台专属数组`（旧路径双端保留重定向）；Shell 作 T2 分发；平台专属能力编译期走 `IS_MOBILE`、运行时数据依赖走 `get_capabilities`。按页原子迁移。**不引入 TanStack Router**（路由非漂移源，不与本次迁移捆绑）。

**D6 里程碑**：M0 桌面规则出站选择器独立修复（先行交付，遵循 ADR-0011 D1/D3 下沉共享 hook）→ M1 脚手架+Base UI spike → M2 Overlay 族+a11y 对照清单 → M3 桌面迁移删 HeroUI → M4 移动迁移删 Konsta → M5 页面树合一 → M6 收尾文档。详见设计文档 §5。

## Consequences

**正面**：

- 漂移根治：一个功能只写一遍页面，形态分叉收敛到 T2 组件；U1 的维护收益、无 U1 的观感代价；
- 依赖面收敛：移除 HeroUI + Konsta 两套 UI 库，供应链与升级成本减半；
- UI 行为完全自控（按压态、safe-area、发丝线等平台细节可逐像素打磨）。

**代价**：

- 自研组件库是一次性大投入（M1-M5 五个里程碑），迁移期三件套并存；
- 可访问性从「HeroUI/RAC 免费提供」变为「自担」——M2 建 a11y 对照清单兜底，不合规不合并；
- Base UI 无 Table/BottomSheet，最难的组件恰需自研；
- 共享组件不得依赖 react-compiler 语义（compiler 仅桌面开启），热路径需显式 `memo`。

## Alternatives considered

- **维持 HeroUI + Konsta 双库 + 纪律约束**：漂移已反复实证（ADR-0011 的 useSettingsConfig 事故、本次出站选择器），拒绝。
- **U1 Konsta 全端**：桌面观感退让，维持 ADR-0011 的拒绝。
- **U3 后缀文件分发**：两套平台机制并存，维持 ADR-0011 的拒绝；路由/页面分叉统一走 D2 的 define 常量。
- **引入 TanStack Router / TanStack Table**：路由非漂移源不捆绑；表格排序为服务端驱动用不上客户端状态机，DataList 契约对齐其列定义形状以备日后接入。

## Implementation status（2026-10-10）

M3/M4 已完成：客户端仅依赖 `@pp/ui`，HeroUI/Konsta 与对应 CSS 已移除。入口、路由和
页面目录统一于 `src/App.tsx`、`src/routes.tsx`、`src/pages`，`@app` alias 已退役。
Config / DNS / 出站 / 路由 / 规则 / 规则集 / Experimental / 日志 / 统计 / 入站 / 设置主页 / 订阅已共用实现；部分平台能力和
交互差异仍保留页内编译期分支，M5 的全面去重与 M6 的最终关闭仍待后续按页完成。
参见 [实施进展](../plans/2026-10-10-client-ui-library-design.md)。
