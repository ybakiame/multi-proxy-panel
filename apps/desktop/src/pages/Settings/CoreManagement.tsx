import { useState } from "react";
import { Alert, Button, Card, Chip } from "@heroui/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import {
  CORE_CHANNELS,
  deleteCore,
  detectSystemCores,
  downloadCore,
  latestVersionOfChannel,
  listCores,
  listRemoteCoreChannels,
  toErrorMessage,
} from "@pp/client-core";
import type { CoreChannel, LocalCoreView, RemoteCoreChannel } from "@pp/client-core";
import { CONFIG_KEY, CORES_KEY, CORES_LIST_KEY, REMOTE_CHANNELS_KEY } from "@pp/client-core";

interface CoreManagementProps {
  settings: UseSettingsConfigReturn;
}

/**
 * 核心管理（参考 `.reference/GUI.for.SingBox` 的分支模型重设计）：
 *
 * - 版本通道：稳定版 / 测试版 / 预发布版三通道，各通道展示远端最新版本与本地
 *   已装版本，一键下载/更新到该通道最新版；不再让用户手动挑选具体版本号。
 * - 本地核心清单：已下载 + 系统探测核心列表（使用中标记、删除）；「当前核心」
 *   由首页（Dashboard）选择生效。
 */
export default function CoreManagement({ settings }: CoreManagementProps) {
  const { config } = settings;
  const queryClient = useQueryClient();
  const [coresBusy, setCoresBusy] = useState(false);
  const [coresError, setCoresError] = useState<string | null>(null);
  const [coresMessage, setCoresMessage] = useState<string | null>(null);
  const [channelBusy, setChannelBusy] = useState<CoreChannel | null>(null);

  const { data: cores = [] } = useQuery<LocalCoreView[]>({
    queryKey: CORES_LIST_KEY,
    queryFn: listCores,
    retry: false,
  });

  const { data: remoteChannels = [] } = useQuery<RemoteCoreChannel[]>({
    queryKey: REMOTE_CHANNELS_KEY,
    queryFn: listRemoteCoreChannels,
    retry: false,
  });

  const invalidateCoreQueries = async () => {
    await queryClient.invalidateQueries({ queryKey: CORES_LIST_KEY });
    await queryClient.invalidateQueries({ queryKey: CORES_KEY });
    // 下载成功后后端会把新核心自动选为 active（auto_select_downloaded_core）。
    await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
  };

  /** 下载指定通道的最新版本（版本号由远端通道查询决定，用户不手动选择）。 */
  const handleDownloadChannel = async (channel: CoreChannel, version: string) => {
    setChannelBusy(channel);
    setCoresError(null);
    setCoresMessage(null);
    try {
      await downloadCore(version);
      setCoresMessage(`已下载 sing-box ${version}（已设为当前核心，重启代理后生效）`);
      await invalidateCoreQueries();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    }
    setChannelBusy(null);
  };

  const handleDetectSystem = async () => {
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      const detected = await detectSystemCores();
      setCoresMessage(`探测到 ${detected.length} 个系统核心`);
      await queryClient.invalidateQueries({ queryKey: CORES_LIST_KEY });
      await queryClient.invalidateQueries({ queryKey: CORES_KEY });
    } catch (err) {
      setCoresError(toErrorMessage(err));
    }
    setCoresBusy(false);
  };

  const handleDeleteCore = async (core: LocalCoreView) => {
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      await deleteCore(core.path);
      setCoresMessage(`已删除 sing-box ${core.version}`);
      await invalidateCoreQueries();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    }
    setCoresBusy(false);
  };

  const activeCore = cores.find((core) => core.active) ?? null;
  const coreBinary = config?.core_binary ?? "";
  const installedVersions = cores.map((core) => core.version);

  return (
    <Card>
      <Card.Header>
        <Card.Title>核心管理</Card.Title>
        <Card.Description>
          按通道下载 / 更新 sing-box 核心；在首页选择要使用的核心（下载后需重启代理生效）
        </Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        {/* 当前核心 */}
        <div className="rounded-xl border border-border/60 bg-surface p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="flex min-w-0 flex-col gap-1">
              <span className="text-xs text-muted">当前核心</span>
              <span className="flex flex-wrap items-center gap-2 text-sm font-medium">
                sing-box
                {activeCore && (
                  <Chip size="sm" variant="soft" color="accent">
                    {activeCore.version}
                  </Chip>
                )}
              </span>
              <span className="truncate text-xs text-muted" title={coreBinary || "未设置二进制路径"}>
                {coreBinary || "未设置二进制路径"}
              </span>
            </div>
          </div>
        </div>

        {/* 版本通道（稳定版 / 测试版 / 预发布版）：远端最新 vs 本地已装，一键下载/更新 */}
        <div className="flex flex-col gap-3 rounded-xl border border-border/60 bg-surface p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-medium">版本通道</span>
              <span className="text-xs text-muted">每个通道自动跟踪远端最新版本，无需手动选择版本号</span>
            </div>
            <Button
              size="sm"
              variant="tertiary"
              isDisabled={channelBusy !== null}
              onPress={() => void queryClient.invalidateQueries({ queryKey: REMOTE_CHANNELS_KEY })}
            >
              检查更新
            </Button>
          </div>
          <div className="flex flex-col gap-2">
            {CORE_CHANNELS.map(({ id, label, description }) => {
              const remote = remoteChannels.find((c) => c.channel === id)?.version;
              const local = latestVersionOfChannel(installedVersions, id);
              const upToDate = Boolean(remote) && local === remote;
              return (
                <div
                  key={id}
                  className="flex items-center justify-between gap-3 rounded-lg border border-border/40 px-3 py-2"
                >
                  <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="flex items-center gap-2 text-sm font-medium">
                      {label}
                      {upToDate && (
                        <Chip size="sm" variant="soft" color="success">
                          已是最新
                        </Chip>
                      )}
                    </span>
                    <span className="text-xs text-muted">{description}</span>
                    <span className="text-xs text-muted">
                      本地 {local ?? "未安装"} · 远端 {remote ?? "未知"}
                    </span>
                  </div>
                  <Button
                    size="sm"
                    variant={upToDate ? "secondary" : "primary"}
                    isPending={channelBusy === id}
                    isDisabled={!remote || upToDate || channelBusy !== null}
                    onPress={() => {
                      if (remote) {
                        void handleDownloadChannel(id, remote);
                      }
                    }}
                  >
                    {local ? "更新" : "下载"}
                  </Button>
                </div>
              );
            })}
          </div>
        </div>

        {/* 已安装核心 */}
        <div className="overflow-x-auto">
          <table className="w-full min-w-[560px] text-sm">
            <thead>
              <tr className="border-b border-border/60 text-left text-xs text-muted">
                <th className="py-2 pr-3 font-normal">版本</th>
                <th className="py-2 pr-3 font-normal">来源</th>
                <th className="py-2 pr-3 font-normal">路径</th>
                <th className="py-2 text-right font-normal">操作</th>
              </tr>
            </thead>
            <tbody>
              {cores.length === 0 ? (
                <tr>
                  <td colSpan={4} className="py-8 text-center text-sm text-muted">
                    暂无可用核心，可从上方通道下载或探测系统核心
                  </td>
                </tr>
              ) : (
                cores.map((core) => (
                  <tr key={core.path} className="border-b border-border/40">
                    <td className="max-w-[160px] truncate py-2 pr-3">
                      <span className="flex items-center gap-2" title={core.version}>
                        {core.version}
                        {core.active && (
                          <Chip size="sm" variant="soft" color="success">
                            使用中
                          </Chip>
                        )}
                      </span>
                    </td>
                    <td className="py-2 pr-3">
                      <Chip size="sm" variant="soft" color={core.source === "downloaded" ? "accent" : "warning"}>
                        {core.source === "downloaded" ? "下载" : "系统"}
                      </Chip>
                    </td>
                    <td className="max-w-[180px] truncate py-2 pr-3 text-xs text-muted">
                      <span title={core.path}>{core.path}</span>
                    </td>
                    <td className="py-2 text-right">
                      <Button
                        size="sm"
                        variant="tertiary"
                        isDisabled={coresBusy || core.source === "system" || core.active}
                        {...{
                          title:
                            core.source === "system"
                              ? "系统核心不可删除"
                              : core.active
                                ? "正在使用的核心不可删除"
                                : undefined,
                        }}
                        onPress={() => void handleDeleteCore(core)}
                      >
                        删除
                      </Button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* 探测系统核心 */}
        <Button variant="secondary" isPending={coresBusy} onPress={() => void handleDetectSystem()}>
          探测系统核心
        </Button>

        {coresMessage && <span className="text-sm text-success">{coresMessage}</span>}
        {coresError && (
          <Alert status="danger">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>核心管理出错</Alert.Title>
              <Alert.Description>{coresError}</Alert.Description>
            </Alert.Content>
          </Alert>
        )}
      </Card.Content>
    </Card>
  );
}
