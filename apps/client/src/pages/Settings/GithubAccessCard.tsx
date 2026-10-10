import { Card } from "@pp/ui";
import { SettingsInput } from "./fields";

interface GithubAccessCardProps {
  settings: { ready: boolean; githubProxyPrefixDraft: string; onGithubProxyPrefixChange: (value: string) => void };
}

/**
 * GitHub 访问（ADR-0003 M5.3）：代理前缀 `github_proxy_prefix`，留空直连。
 */
export function GithubAccessCard({ settings }: GithubAccessCardProps) {
  return (
    <Card>
      <Card.Content className="flex flex-col gap-4">
        <GithubPrefixField settings={settings} />
      </Card.Content>
    </Card>
  );
}

/** 双端共享代理前缀字段，保存由页面适配器提供。 */
export function GithubPrefixField({ settings }: GithubAccessCardProps) {
  return (
    <SettingsInput
      id="settings-github-proxy-prefix"
      label="GitHub 代理前缀"
      value={settings.githubProxyPrefixDraft}
      onChange={settings.onGithubProxyPrefixChange}
      placeholder="https://gh-proxy.com"
      disabled={!settings.ready}
      hint="GitHub 链接拼接此前缀访问，留空直连"
    />
  );
}
