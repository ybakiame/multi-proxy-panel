import { Card } from "@heroui/react";
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
      <Card.Header>
        <Card.Title>GitHub 访问</Card.Title>
        <Card.Description>加速订阅与远程资源拉取</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <SettingsInput
          id="settings-github-proxy-prefix"
          label="GitHub 代理前缀"
          value={settings.githubProxyPrefixDraft}
          onChange={settings.onGithubProxyPrefixChange}
          placeholder="https://gh-proxy.com"
          disabled={!settings.ready}
          hint="如 https://gh-proxy.com，资源链接将拼接此前缀访问；留空则直连 GitHub"
        />
      </Card.Content>
    </Card>
  );
}
