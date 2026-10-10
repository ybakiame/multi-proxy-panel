import { Card } from "@pp/ui";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { SettingsInput } from "./fields";

interface GithubAccessCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * GitHub 访问（ADR-0003 M5.3）：代理前缀 `github_proxy_prefix`，留空直连。
 */
export function GithubAccessCard({ settings }: GithubAccessCardProps) {
  return (
    <Card>
      <Card.Content className="flex flex-col gap-4">
        <SettingsInput
          id="settings-github-proxy-prefix"
          label="GitHub 代理前缀"
          value={settings.githubProxyPrefixDraft}
          onChange={settings.onGithubProxyPrefixChange}
          placeholder="https://gh-proxy.com"
          disabled={!settings.ready}
          hint="从 Github 下载资源时自动拼接此代理地址进行加下载速"
        />
      </Card.Content>
    </Card>
  );
}
