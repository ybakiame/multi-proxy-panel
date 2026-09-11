import { BackHeader } from "../../components/BackHeader";
import { DeveloperToolsCard } from "./DeveloperToolsCard";

/**
 * 开发者工具子页（路由 `/settings/dev-tools`）。
 *
 * 迁移原设置主页开发者工具分组：日志入口 + 当前配置预览入口（含配置预览 Modal）。
 */
export default function DevToolsPage() {
  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="开发者工具" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <DeveloperToolsCard />
      </div>
    </div>
  );
}
