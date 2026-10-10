# 客户端自研组件库与页面树合一 — 设计文档

- 日期：2026-10-10
- 状态：已批准（用户确认 2026-10-10）
- 关联：ADR-0011（U2 共享视图模型层）、ADR-0007（单壳双目标）、ADR-0005（配置切片，D3 桌面后置）
- 决策载体：ADR-0013

## 背景与问题

ADR-0007 将客户端合并为单壳双目标后，桌面（`src/desktop`，HeroUI v3 + react-compiler）与移动（`src/mobile`，Konsta UI）维持两棵镜像但独立的页面树。ADR-0011 已将**视图模型层**下沉至 `@pp/client-core`（U2 路线），但 **UI 层**仍是双份：同一功能在两棵页面树里各实现一遍，漂移持续发生。

最新实证：移动端规则管理具备完整「指定出站」能力（`RuleEditSheet.tsx` 动作分段控件 + 出站选择器，`CustomRulesPage.tsx` 构建订阅节点 + 模板分组 + 切片出站三源并集），桌面端 `RuleEditModal.tsx` 却显式过滤 `outbound` 动作（`DESKTOP_RULE_ACTIONS = RULE_ACTIONS.filter(opt => opt.id !== "outbound")`，注释「随 D3 后置实现」）。数据层早已统一（`RULE_ACTIONS`/`buildOutboundAction` 等在 client-core），缺的只是桌面 UI 与数据装配——漂移不在数据层，在 UI 层。

ADR-0011 当时对比过 U1（单 UI 库统一 Konsta 全端）/U2（共享视图模型层）/U3（`.desktop/.mobile` 后缀覆盖），采纳 U2、暂缓 U1/U3。本次用户提出第四条路线并被采纳：**基于 headless 库 + Tailwind 二次开发自研组件库，按当前实际用到的组件针对性实现，编译期决定组件形态，UI 完全自控，最终页面树合一**。

### 与 ADR-0011 的关系

本设计是 ADR-0011 的**后续而非推翻**：U2（视图模型下沉）已落地且继续有效；U1 的「Konsta 全端」方案仍被拒绝（桌面观感退让不可接受），但「UI 自控 + 页面树合一」的目标以自研组件库形式达成——形态分叉收敛到组件层（T2），而非 ADR-0011 假设的整棵页面树。U3 后缀机制维持拒绝（见 §2）。

## §1 组件库定位与分层

新包 `packages/ui`（`@pp/ui`），Bun workspace 成员，React + Tailwind CSS v4。只实现**当前实际用到的组件**，不追求通用组件库完备性。

组件按平台分叉程度分三层：

| 层 | 定义 | 组件 |
|----|------|------|
| T0 纯样式 | 无交互逻辑、双端同形态 | Card、Chip、Spinner、Label、Separator、InlineAlert |
| T1 headless 封装 | Base UI 提供交互原语，共享逻辑 + 平台皮肤 | Button、Switch、Checkbox、Radio、Input、Field、Tabs、Toast、Meter、Avatar、Tooltip、Progress、Menu |
| T2 形态分叉 | 同一 props 契约，桌面/移动两种形态实现 | **Overlay**（桌面 Modal / 移动 BottomSheet）、**Select**（桌面 dropdown / 移动 sheet-picker）、**DataList**（桌面 Table / 移动卡片列表）、**Shell**（桌面 Sidebar / 移动 TabBar+Navbar+FAB） |

现存用量依据（迁移工作量参照）：

- 桌面 HeroUI v3：Button×44、Card×26、Alert×24、Label×21、Input×17、Chip×17、Switch×16、Select×16、ListBox×16、Modal×11、Table×9、AlertDialog×7、Spinner×4、Tabs/Checkbox×3、TextArea×2、toast、RadioGroup、Meter、Avatar
- 移动 Konsta 直接用（适配层 `mobile/components/ui.tsx` 之外的残留，约 10 处壳层惯用法）：Segmented×3、ToolbarPane/Toast/Tabbar/Navbar/List/KonstaProvider/Fab 等各 1

底座选型：**Base UI**（用户决策）。注意 Base UI 无 Table、无 BottomSheet、无壳层组件——这些自研（Table 见 §4 DataList；BottomSheet 属 M2 自研 sheet）。HeroUI v3 的底座是 React Aria Components，RAC 为 Base UI 验证不通过时的后备（见 §6）。

**Konsta 去留：全部自研**（用户决策）。HeroUI 与 Konsta 都移除；移动 iOS/Material 双主题用 Tailwind 自研（见 §3）。

## §2 编译期分发机制（方案 A：define 常量）

机制选型对比后用户选定**方案 A**：

- vite `define` 注入 `__PP_PLATFORM__`（desktop/mobile），组件内以 `IS_MOBILE` 常量分支 + 内部子模块拆分（如 `select/Dropdown.tsx` | `select/SheetPicker.tsx`），靠 DCE 裁掉死分支
- 拒绝方案 B（`.desktop.tsx`/`.mobile.tsx` 后缀 + vite resolve 插件）：引入第二套平台分发机制（与 define 并存），tsc/声明文件编排复杂——与 ADR-0011 拒 U3 同理

T2 组件形态示例：

```tsx
// packages/ui/src/overlay/Overlay.tsx
export function Overlay(props: OverlayProps) {
  return IS_MOBILE ? <BottomSheet {...props} /> : <Modal {...props} />;
}
```

约束：共享组件**不得依赖 react-compiler 语义**（compiler 仅桌面开启），需要时显式 `memo`。

## §3 设计令牌与移动双主题

- **沿用 HeroUI 语义令牌名**（`bg-surface`/`text-muted`/`border-border` 等），以 Tailwind v4 `@theme` 自定义实现。桌面迁移时**换 import 不换 class**， diff 最小化。
- 三层作用域：
  1. `:root` 浅色令牌
  2. `.dark` 深色（沿用 client-core 既有主题模块 `pp-ui-theme`，不动）
  3. `[data-ui-style="ios"|"material"]` 移动双主题域（沿用存储键 `pp-ui-style`，默认 ios）
- Konsta 移除后需自研的平台细节：`env()` safe-area 内边距、发丝线边框（hairline）、按压态。

**Material 波纹决策（用户选定）**：首期双端统一按压态（active scale/透明度），**波纹效果后置**。

## §4 页面树合一

目标（用户决策）：**页面树合一**——最终不再有 `src/desktop`、`src/mobile` 两棵树。

- **路由合一**：单一权威路由表 `src/routes.tsx` = `sharedRoutes` + `IS_MOBILE` 三元展开的平台专属数组（桌面 `/mitm`、`/scripts`、`/tools`、`/override`；移动 `/settings/vpn-notify`、`/settings/dev-tools/diagnose` 及旧路径重定向 shim）。ternary 折叠后另一平台的页面 import 成为死引用被 tree-shake。命名分叉（`/nodes` vs `/subscriptions`）合一时定规范名，旧名双端保留重定向（HashRouter 书签兼容）。同路由但结构实在无法抽象的页面（应极少）在单路由组件内 `IS_MOBILE ? <A/> : <B/>`，路由表保持干净。
- **Shell 作 T2 分发**：桌面侧边栏 / 移动 TabBar+Navbar+FAB 由 Shell 组件按平台渲染。
- **平台专属能力**：编译期分支走 `IS_MOBILE`；数据/能力依赖走 `get_capabilities` 运行时开关（ADR-0007 既定）。
- **DataList 契约**：列定义 + 行数据 + 行操作；列定义形状镜像 TanStack Table（`accessorKey` + `cell` renderer），未来需要客户端大表格能力时可无痛接入。试点页 = 规则管理页。
- **迁移节奏**：按页原子迁移（每页一个提交），迁移期新旧并存可接受。

**不引入 TanStack Router**：路由不是漂移源（~20 条扁平路由、无 search-param 需求、数据加载归 react-query、HashRouter 是 Tauri+书签的刻意选择），不与本次迁移捆绑；未来出现 modal-as-URL 类需求再单独评估。

**暂不引入 TanStack Table**：现有表格排序是服务端驱动（`search/sort/desc` 进查询参数），用不上其客户端状态机；DataList 契约自研约百行，列定义形状对齐以便日后接入。

## §5 里程碑

| 里程碑 | 内容 | 出口标准 |
|--------|------|----------|
| **M0** 独立修复 | 桌面规则出站选择器：outboundOptions 三源并集下沉 client-core 共享 hook，移动端改消费，桌面 RuleEditModal 放开 outbound + 出站 Select | 三包 verify 绿；双端规则出站能力对齐（独立于组件库，先行交付） |
| **M1** 脚手架 | `@pp/ui` 建包 + 令牌体系 + vite define + T0 全部 + T1 基础（Button/Switch/Input/Field）+ 移动双主题令牌域 | Base UI spike 通过（不过则转 RAC 后备并记录）；桌面某页试点换装 |
| **M2** Overlay 族 ✅(2026-10-10) | Base UI Dialog/Select 封装 + ~~自研 BottomSheet~~（实际采用 Base UI 1.9 原生 Drawer，获得滑动关闭手势）；建 a11y 对照清单（焦点陷阱/Esc/aria 属性逐项核对） | 清单全绿；桌面 Modal/移动 Sheet 各一处试点 |
| **M3** 桌面迁移 ✅(2026-10-10) | 全桌面换 import（T0/T1/T2），自研 Table 9 处，删 `@heroui/*` 依赖 | 桌面 verify 绿；HeroUI 依赖移除 |
| **M4** 移动迁移 ✅(2026-10-10) | 壳层惯用法自研（TabBar/Navbar/FAB/Segmented/List），删 `konsta` 依赖 | 移动 verify 绿；Konsta 依赖移除 |
| **M5** 页面树合一（主体完成，页内差异待进一步收敛） | 试点 Config/Rules 合一 → Shell/路由表合一 → 推广全部页面；删 `src/desktop`、`src/mobile` 树 | 单一 `src/pages` + `@pp/ui` 分发；vite `@app` alias 退役 |
| **M6** 收尾（结构文档已同步） | vite 配置简化、ADR-0013 落定、AGENTS.md §4.5/架构文档/CHANGELOG 同步 | 文档与实现一致 |

## §6 风险与对策

1. **Base UI 成熟度**：M1 首周 spike 验证（Dialog/Select/Menu 在 Tauri webview 的行为、a11y 基线、Tailwind 集成）；不通过则转 React Aria Components 后备——底座隔离在 `@pp/ui` 内部，业务页面无感。
2. **a11y 回退**：自研 BottomSheet/Table 无现成可访问性保证；M2 建 Overlay 对照清单逐项核对，不合规不合并。
3. **迁移期三套并存**（HeroUI/Konsta/@pp/ui）：按页原子提交，每页独立可回滚；不搞大爆炸切换。
4. **Material 保真度**：无波纹是已知妥协（首期按压态代替），后置版本再评估波纹。
5. **react-compiler 仅桌面开启**：共享组件不得依赖 compiler 自动记忆化，热路径显式 `memo`。

## 附：M0 修复细节（先行实施）

漂移根因：`outboundOptions` 三源并集构建写死在移动 `CustomRulesPage.tsx` 页面内，桌面补 UI 时无数据源可用。修法遵循 ADR-0011 D1/D3：

1. client-core 新增共享 hook：封装订阅节点（`subscriptionNodeTags`）+ 模板分组（`proxiesList().groups`）+ 切片出站（`configSlicesGet().outbounds.items` 仅 enabled，`tag=outboundTag(name)`）三源并集按 tag 去重；附「原值保留」项逻辑与 `subscriptionCacheAvailable` 空候选引导
2. 移动 `CustomRulesPage` 删除页面内实现，改消费共享 hook（D3 单一权威）
3. 桌面 `Rules/index.tsx` 接入共享 hook；`RuleEditModal` 删除 `DESKTOP_RULE_ACTIONS` 过滤，新增出站 `Select`（空候选引导文案与移动对齐）

## 实施进展（2026-10-10，M3–M5）

- M3/M4：补齐 Checkbox / Radio / Tabs / Avatar / Meter / TextArea / Table / Segmented /
  Navbar / FAB / ToastRegion；客户端全部改用 `@pp/ui`，移除 HeroUI/Konsta 依赖与 CSS。
- T2：新增 `SelectField`（Base UI dropdown / Drawer picker）、`DataList`（原生 table /
  卡片列表）及 `Shell`；规则列表率先采用 DataList，规则编辑共用保存、原值保留与出站候选。
- M5：入口 `src/App.tsx`、样式 `src/index.css`、路由 `src/routes.tsx` 单源化；旧
  `src/desktop`、`src/mobile` 与 vite/tsc `@app` alias 已移除。配置入口、DNS、出站、
  路由、规则、规则集/市场、Experimental 共用实现。订阅规范路径为 `/subscriptions`。
- 仪表盘、订阅、统计、日志、设置、入站目前仍有**页内平台实现**：分别涉及 VPN 授权与
  桌面进程控制、订阅生效/覆写交互、统计卡片/表格、磁盘/内存日志、核心管理/VPN 通知、
  TUN 提权/Android 恒启用。它们位于同一页面目录，由页面入口编译期选择，未声称所有
  页面逻辑已完全去重；后续按页继续收敛。因此 M5/M6 尚未全部关闭。
- 保留桌面 Clash API 的启用开关、可选密钥和面板 UI 选择；Android 恒启用/必填密钥的
  自动纠正仅在移动构建执行。平台行为沿用既有 ADR，不把移动约束扩散至桌面。
- 验证记录见 [UI 迁移验证](../testing/2026-10-10-client-ui-refactor.md)。
