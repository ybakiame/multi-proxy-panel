# ProxyPanel Panel 开发约定

- UI 使用 shadcn/ui 的 Base UI 组件与 Tailwind CSS v4；不要重新引入 HeroUI。
- 基础组件源码位于 `src/components/ui/`，由根目录 `components.json` 配置。新增组件前阅读官方文档，并在仓库根目录使用 `bunx shadcn@latest add <component> -c apps/panel`。
- 可按面板需求修改仓库内组件源码；保持 Base UI 的键盘导航、焦点管理和语义化 HTML，不用样式覆盖破坏可访问性。
- 表单共享组件保留现有 `FormInput`、`FormSelect`、`FormCheckbox` 等调用接口；页面优先复用这些组件。
- 颜色使用 `src/index.css` 的语义变量，明暗主题都必须可读。图标沿用现有 Heroicons；shadcn 生成组件使用 Lucide。
- 修改后运行 `bun run --filter pp-web verify`；前端无单元测试运行器，关键交互另做浏览器验证。

## 设计约定（design skeleton）

- **颜色全部为 OKLCH**，禁止 hex。`src/index.css` 按 primitive（`@theme` 湖水青 `--color-lake-*` 色阶）→ semantic（`:root`/`.dark` 的 surface/文字/边框/状态/chart/`--ring` 焦点）→ component（`--shadow-card`、`--gradient-brand`）三层组织；组件只引用语义 token 或 Tailwind 语义工具类。
- 亮主题为灰白工作区（`--background`）+ 纯白卡片（`--card`）+ 紧凑细分割线（`--border`）；暗主题保持同 hue 的蓝灰，对比度以正文可读为准。
- **渐变仅允许品牌区（登录页品牌面板）与重点 KPI 状态卡**，使用 `--gradient-brand`；主按钮等交互控件一律纯色 `--primary`，不做大面积渐变。
- 品牌标识用 `src/components/BrandMark.tsx` 的 SVG（节点链路图形 + 湖水青渐变），登录页与 Layout 侧栏共用。
- Hub/UI 双版本展示用 `src/components/VersionBadge.tsx`（`useHubVersion` 查询 `/health`，UI 版本取自 `apps/panel/package.json`），Loading/失败有降级文案，不阻塞页面。
- 导航按 `nav.ts` 的 `navSections` 分组（概览 / 节点与配置 / 用户与订阅 / 观测 / 系统）；当前项为实心 `--primary` 圆角块（`aria-current="page"`）。
- 数据密集页面卡片统一 `shadow-(--shadow-card)`、`rounded-xl`、`size="sm"` 紧凑内距；Dashboard 类页面每个查询独立 loading/error 重试，禁止全局 spinner  gate。
- 尊重 `prefers-reduced-motion`（`src/index.css` 已全局降级动画）。
