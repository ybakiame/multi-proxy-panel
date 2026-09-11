import { BackHeader } from "../../components/BackHeader";
import { GithubAccessCard } from "./GithubAccessCard";
import { useSettingsConfig } from "./useSettingsConfig";

/**
 * GitHub 访问子页（路由 `/settings/github`）。
 *
 * 迁移原设置主页 GitHub 访问分组：代理前缀（`github_proxy_prefix`，留空直连）。
 */
export default function GithubPage() {
  const settings = useSettingsConfig();

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="GitHub 访问" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <GithubAccessCard settings={settings} />
      </div>
    </div>
  );
}
