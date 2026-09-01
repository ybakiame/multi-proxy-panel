import { Button, Card, Input, Label, Switch } from "@heroui/react";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { testGithubProxy, toErrorMessage } from "../../api";

interface GithubSettingsProps {
  settings: UseSettingsConfigReturn;
}

export default function GithubSettings({ settings }: GithubSettingsProps) {
  const {
    fetchViaLocalProxy,
    setFetchViaLocalProxy,
    githubProxyPrefix,
    setGithubProxyPrefix,
    proxyTestPending,
    proxyTestResult,
    proxyTestError,
    setProxyTestPending,
    setProxyTestResult,
    setProxyTestError,
    persist,
    persistDebounced,
  } = settings;

  const runProxyTest = async () => {
    setProxyTestPending(true);
    setProxyTestResult(null);
    setProxyTestError(null);
    try {
      const result = await testGithubProxy();
      const ms = result.match(/（(\d+) ms）/)?.[1];
      setProxyTestResult(ms ? `代理可用（${ms} ms）` : result);
    } catch (err) {
      setProxyTestError(toErrorMessage(err));
    } finally {
      setProxyTestPending(false);
    }
  };

  return (
    <Card>
      <Card.Header>
        <Card.Title>GitHub 访问</Card.Title>
        <Card.Description>中国大陆网络下远程资源拉取失败时的代理配置</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-3 rounded-xl border border-border/60 bg-surface p-4">
          <Switch
            isSelected={fetchViaLocalProxy}
            onChange={(next) => {
              setFetchViaLocalProxy(next);
              void persist({ fetch_via_local_proxy: next });
            }}
          >
            <Switch.Content>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
              远程资源拉取走本地代理
            </Switch.Content>
          </Switch>
          <span className="text-xs text-muted">经本机核心 mixed 端口转发拉取请求，需核心运行中</span>
        </div>

        <div className="flex flex-col gap-2">
          <Label htmlFor="settings-github-proxy-prefix">GitHub 代理前缀</Label>
          <Input
            id="settings-github-proxy-prefix"
            aria-label="GitHub 代理前缀"
            value={githubProxyPrefix}
            onChange={(event) => {
              setGithubProxyPrefix(event.target.value);
              persistDebounced({ github_proxy_prefix: event.target.value });
            }}
            placeholder="https://gh-proxy.com"
            fullWidth
          />
          <span className="break-words text-xs text-muted">
            GitHub 链接将拼接前缀访问，例如 https://gh-proxy.com/https://raw.githubusercontent.com/…；留空则直连
          </span>
          <div className="flex flex-wrap items-center gap-3">
            <Button variant="secondary" size="sm" isPending={proxyTestPending} onPress={() => void runProxyTest()}>
              测试连通性
            </Button>
            {proxyTestResult && <span className="break-words text-sm text-success">{proxyTestResult}</span>}
            {proxyTestError && <span className="break-words text-sm text-warning">{proxyTestError}</span>}
          </div>
        </div>
      </Card.Content>
    </Card>
  );
}
