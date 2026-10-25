# ProxyPanel Panel 开发约定

- UI 使用 shadcn/ui 的 Base UI 组件与 Tailwind CSS v4；不要重新引入 HeroUI。
- 基础组件源码位于 `src/components/ui/`，由根目录 `components.json` 配置。新增组件前阅读官方文档，并在仓库根目录使用 `bunx shadcn@latest add <component> -c apps/panel`。
- 可按面板需求修改仓库内组件源码；保持 Base UI 的键盘导航、焦点管理和语义化 HTML，不用样式覆盖破坏可访问性。
- 表单共享组件保留现有 `FormInput`、`FormSelect`、`FormCheckbox` 等调用接口；页面优先复用这些组件。
- 颜色使用 `src/index.css` 的语义变量，明暗主题都必须可读。图标沿用现有 Heroicons；shadcn 生成组件使用 Lucide。
- 修改后运行 `bun run --filter pp-web verify`；前端无单元测试运行器，关键交互另做浏览器验证。
