import { useState } from "react";
import { Button, Card } from "@pp/ui";
import { toastError, toastSuccess } from "@pp/client-core";
import { checkForUpdate } from "../../../lib/updater";
import { APP_VERSION } from "./useSettingsConfig";

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
export default function AboutSection() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  const onCheck = async () => {
    setState({ kind: "checking" });
    try {
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
        <Card.Title>关于应用</Card.Title>
        <Card.Description>ProxyPanel 客户端信息</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <span className="text-sm text-muted">版本号</span>
          <span className="text-sm font-medium">{APP_VERSION}</span>
        </div>
        <div className="flex items-center justify-between">
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
        <div className="flex items-center justify-between">
          <span className="text-sm text-muted">项目链接</span>
          <Button
            variant="secondary"
            size="sm"
            onPress={() => {
              window.open("https://github.com/ybakiame/multi-proxy-panel", "_blank");
            }}
          >
            GitHub
          </Button>
        </div>
      </Card.Content>
    </Card>
  );
}
