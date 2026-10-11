import { useState } from "react";
import { Button, Card, IS_MOBILE } from "@pp/ui";
import { openReleaseUrl, toastError, toastSuccess } from "@pp/client-core";
import { checkForUpdate } from "../../lib/updater";
import { AboutInfo } from "./AboutInfo";

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; install: () => Promise<void> }
  | { kind: "installing" };

/**
 * 关于应用卡（版本号 / 检查更新 / 项目链接）。
 *
 * 日志入口已移除（2026-10）：侧边栏主导航本就有「日志」页，设置页内的跳转卡片
 * 只是重复入口，无独立意义。
 *
 * 检查更新（ADR-0008 D2）：经 GitHub Releases latest.json + ed25519 签名校验；
 * 有更新时行内出现「下载并重启」按钮，安装完成自动重启应用。
 */
export default function ProductCard() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  const onCheck = async () => {
    setState({ kind: "checking" });
    try {
      if (IS_MOBILE) {
        await openReleaseUrl();
        setState({ kind: "idle" });
        return;
      }
      const result = await checkForUpdate();
      if (result.version && result.install) {
        setState({ kind: "available", version: result.version, install: result.install });
      } else {
        setState({ kind: "idle" });
        toastSuccess("当前已是最新版本");
      }
    } catch (e) {
      setState({ kind: "idle" });
      toastError(`检查更新失败：${String(e)}`);
    }
  };

  const onInstall = async (install: () => Promise<void>) => {
    setState({ kind: "installing" });
    try {
      // 成功路径不再返回：安装完成后应用自动重启。
      await install();
    } catch (e) {
      setState({ kind: "idle" });
      toastError(`更新安装失败：${String(e)}`);
    }
  };

  const checking = state.kind === "checking";
  const installing = state.kind === "installing";

  return (
    <Card>
      <Card.Header>
        <Card.Title>ProxyPanel</Card.Title>
        <Card.Description>管理订阅、代理策略与本地规则的跨平台代理客户端</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <AboutInfo />
        {IS_MOBILE && <p className="text-xs text-muted">检查更新将在系统浏览器打开最新发布页。</p>}
        <div className="flex min-h-8 items-center justify-between gap-4">
          <span className="text-sm text-muted">检查更新</span>
          {state.kind === "available" ? (
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">新版本 {state.version}</span>
              <Button variant="primary" size="sm" isDisabled={installing} onPress={() => void onInstall(state.install)}>
                {installing ? "下载安装中…" : "下载并重启"}
              </Button>
            </div>
          ) : (
            <Button variant="secondary" size="sm" isDisabled={checking || installing} onPress={() => void onCheck()}>
              {checking ? "检查中…" : installing ? "下载安装中…" : "检查更新"}
            </Button>
          )}
        </div>
      </Card.Content>
    </Card>
  );
}
