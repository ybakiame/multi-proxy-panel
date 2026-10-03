import { SubPageShell } from "../../components/SubPageShell";
import { DeveloperToolsCard } from "./DeveloperToolsCard";

/**
 * 开发者工具子页（路由 `/settings/dev-tools`）。
 *
 * 迁移原设置主页开发者工具分组：日志入口 + 当前配置预览入口（含配置预览 Modal）。
 */
export default function DevToolsPage() {
  return (
    <SubPageShell title="开发者工具">
      <DeveloperToolsCard />
    </SubPageShell>
  );
}
