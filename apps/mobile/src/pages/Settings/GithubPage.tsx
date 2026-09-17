import { SubPageShell } from "../../components/SubPageShell";
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
    <SubPageShell title="GitHub 访问">
      <GithubAccessCard settings={settings} />
    </SubPageShell>
  );
}
