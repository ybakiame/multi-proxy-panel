import { useCallback, useEffect, useState } from "react";
import { Alert, Button, Card, Chip, Label, ListBox, Select } from "@heroui/react";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { CORE_LABELS, CORE_CHIP_COLORS, normalizeCoreType } from "./useSettingsConfig";
import {
  deleteCore,
  detectSystemCores,
  downloadCore,
  listCores,
  listRemoteCoreVersions,
  toErrorMessage,
} from "../../api";
import type { CoreType, LocalCoreView } from "../../api";

interface CoreManagementProps {
  settings: UseSettingsConfigReturn;
}

export default function CoreManagement({ settings }: CoreManagementProps) {
  const { config } = settings;
  const [cores, setCores] = useState<LocalCoreView[]>([]);
  const [remoteVersions, setRemoteVersions] = useState<string[]>([]);
  const [downloadType, setDownloadType] = useState<CoreType>("singbox");
  const [downloadVersion, setDownloadVersion] = useState("");
  const [coresBusy, setCoresBusy] = useState(false);
  const [coresError, setCoresError] = useState<string | null>(null);
  const [coresMessage, setCoresMessage] = useState<string | null>(null);

  const refreshCores = useCallback(async () => {
    try {
      setCores(await listCores());
      setCoresError(null);
    } catch (err) {
      setCoresError(toErrorMessage(err));
    }
  }, []);

  const refreshRemoteVersions = useCallback(async (coreType: CoreType) => {
    try {
      const versions = await listRemoteCoreVersions(coreType);
      setRemoteVersions(versions);
      setDownloadVersion(versions[0] ?? "");
      setCoresError(null);
    } catch (err) {
      setCoresError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshCores();
  }, [refreshCores]);

  useEffect(() => {
    void refreshRemoteVersions(downloadType);
  }, [downloadType, refreshRemoteVersions]);

  const handleDownload = async () => {
    if (!downloadVersion) {
      return;
    }
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      await downloadCore(downloadType, downloadVersion);
      setCoresMessage(`已下载 ${CORE_LABELS[downloadType]} ${downloadVersion}`);
      await refreshCores();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    } finally {
      setCoresBusy(false);
    }
  };

  const handleDetectSystem = async () => {
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      const detected = await detectSystemCores();
      setCoresMessage(`探测到 ${detected.length} 个系统核心`);
      await refreshCores();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    } finally {
      setCoresBusy(false);
    }
  };

  const handleDeleteCore = async (core: LocalCoreView) => {
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      await deleteCore(core.path);
      const coreLabel = CORE_LABELS[core.core_type] ?? core.core_type;
      setCoresMessage(`已删除 ${coreLabel} ${core.version}`);
      await refreshCores();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    } finally {
      setCoresBusy(false);
    }
  };

  const activeCore = cores.find((core) => core.active) ?? null;
  const normalizedCoreType = normalizeCoreType(config?.core_type ?? "singbox") as CoreType;
  const coreBinary = config?.core_binary ?? "";

  return (
    <Card>
      <Card.Header>
        <Card.Title>核心管理</Card.Title>
        <Card.Description>下载与管理核心二进制；在首页选择要使用的核心（下载/删除后需重启代理生效）</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        {/* 当前核心 */}
        <div className="rounded-xl border border-border/60 bg-surface p-4">
          <div className="flex items-center justify-between gap-3">
            <div className="flex min-w-0 flex-col gap-1">
              <span className="text-xs text-muted">当前核心</span>
              <span className="flex flex-wrap items-center gap-2 text-sm font-medium">
                {CORE_LABELS[normalizedCoreType]}
                {activeCore && (
                  <Chip size="sm" variant="soft" color={CORE_CHIP_COLORS[activeCore.core_type]}>
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

        {/* 已安装核心 */}
        <div className="overflow-x-auto">
          <table className="w-full min-w-[560px] text-sm">
            <thead>
              <tr className="border-b border-border/60 text-left text-xs text-muted">
                <th className="py-2 pr-3 font-normal">类型</th>
                <th className="py-2 pr-3 font-normal">版本</th>
                <th className="py-2 pr-3 font-normal">来源</th>
                <th className="py-2 pr-3 font-normal">路径</th>
                <th className="py-2 text-right font-normal">操作</th>
              </tr>
            </thead>
            <tbody>
              {cores.length === 0 ? (
                <tr>
                  <td colSpan={5} className="py-8 text-center text-sm text-muted">
                    暂无可用核心，可下载新版本或探测系统核心
                  </td>
                </tr>
              ) : (
                cores.map((core) => {
                  const coreLabel = CORE_LABELS[core.core_type] ?? core.core_type;
                  return (
                    <tr key={core.path} className="border-b border-border/40">
                      <td className="py-2 pr-3">
                        <span className="flex items-center gap-2">
                          {coreLabel}
                          {core.active && (
                            <Chip size="sm" variant="soft" color="success">
                              使用中
                            </Chip>
                          )}
                        </span>
                      </td>
                      <td className="max-w-[160px] truncate py-2 pr-3">
                        <span title={core.version}>{core.version}</span>
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
                  );
                })
              )}
            </tbody>
          </table>
        </div>

        {/* 下载新版本 */}
        <div className="flex flex-col gap-3 rounded-xl border border-border/60 bg-surface p-4">
          <span className="text-sm font-medium">下载新版本</span>
          <div className="flex flex-wrap items-end gap-3">
            <div className="flex flex-col gap-1">
              <Label>核心类型</Label>
              <Select
                aria-label="下载核心类型"
                value={downloadType}
                onChange={(value) => setDownloadType((value as CoreType | null) ?? "singbox")}
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    <ListBox.Item id="singbox" textValue="sing-box">
                      sing-box
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                    <ListBox.Item id="mihomo" textValue="mihomo">
                      mihomo
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <div className="flex flex-col gap-1">
              <Label>版本</Label>
              <Select
                aria-label="下载版本"
                value={downloadVersion}
                onChange={(value) => setDownloadVersion(String(value ?? ""))}
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {remoteVersions.length === 0 ? (
                      <ListBox.Item id="__empty" textValue="暂无远端版本">
                        暂无远端版本
                      </ListBox.Item>
                    ) : (
                      remoteVersions.map((version) => (
                        <ListBox.Item key={version} id={version} textValue={version}>
                          {version}
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))
                    )}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <Button variant="tertiary" isDisabled={coresBusy} onPress={() => void refreshRemoteVersions(downloadType)}>
              刷新版本
            </Button>
            <Button
              variant="primary"
              isPending={coresBusy}
              isDisabled={!downloadVersion}
              onPress={() => void handleDownload()}
            >
              下载
            </Button>
          </div>
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
