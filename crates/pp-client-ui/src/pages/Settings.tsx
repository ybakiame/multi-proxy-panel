import { useCallback, useEffect, useState } from "react";
import { Alert, Badge, Button, Card, Chip, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import {
  detectSystemCores,
  downloadCore,
  listCores,
  listDownloadedVersions,
  listRemoteCoreVersions,
  setActiveCore,
  toErrorMessage,
} from "../api";
import type { ClientConfig, CoreType, LocalCoreView } from "../api";
import { useAppStore } from "../store";

const CORE_TYPE_OPTIONS = [
  { id: "singbox", label: "SingBox" },
  { id: "mihomo", label: "Mihomo" },
] as const;

/** 兼容任务描述中的 PascalCase 值（`SingBox`/`Mihomo`）与后端 serde 值（`singbox`/`mihomo`）。 */
function normalizeCoreType(value: string): string {
  if (value === "SingBox") {
    return "singbox";
  }
  if (value === "Mihomo") {
    return "mihomo";
  }
  return value;
}

/** 核心类型展示名（与复写页的 CORE_LABELS 一致）。 */
const CORE_LABELS: Record<CoreType, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 核心类型 Chip 配色：sing-box 用强调色、mihomo 用警告色区分。 */
const CORE_CHIP_COLORS: Record<CoreType, "accent" | "warning"> = {
  singbox: "accent",
  mihomo: "warning",
};

/** TUN 协议栈选项（与后端 `ClientConfigView.tun_stack` 的 serde 值一致）。 */
const TUN_STACK_OPTIONS = [
  { id: "mixed", label: "mixed" },
  { id: "gvisor", label: "gvisor" },
  { id: "system", label: "system" },
] as const;

/** Clash 面板 UI 选项（与后端 `ClientConfigView.clash_api_ui` 的 serde 值一致，默认 zashboard）。 */
const CLASH_UI_OPTIONS = [
  { id: "zashboard", label: "zashboard" },
  { id: "yacd", label: "yacd" },
  { id: "metacubexd", label: "metacubexd" },
] as const;

/**
 * 按后端 `preferred_binary` 语义取某类型首选本地核心：已下载优先，其次系统探测。
 * 仅用于 core_type 变更时的联动结果预览（实际回填由 `save_config` 完成）。
 */
function preferredCoreFor(cores: LocalCoreView[], coreType: CoreType): LocalCoreView | null {
  const byType = cores.filter((core) => normalizeCoreType(core.core_type) === coreType);
  if (byType.length === 0) {
    return null;
  }
  return byType.find((core) => core.source === "downloaded") ?? byType[0];
}

export default function Settings() {
  const { config, loading, error, clearError, loadConfig, saveConfig } = useAppStore();
  const [coreType, setCoreType] = useState<string>("singbox");
  const [coreBinary, setCoreBinary] = useState("");
  const [mixedPort, setMixedPort] = useState(1080);
  const [mitmEnabled, setMitmEnabled] = useState(false);
  const [systemProxyEnabled, setSystemProxyEnabled] = useState(false);
  const [saved, setSaved] = useState(false);
  // TUN 模式
  const [tunEnabled, setTunEnabled] = useState(false);
  const [tunStack, setTunStack] = useState<string>("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  // Clash 面板
  const [clashApiEnabled, setClashApiEnabled] = useState(false);
  const [clashApiPort, setClashApiPort] = useState(9090);
  const [clashApiSecret, setClashApiSecret] = useState("");
  const [clashApiUi, setClashApiUi] = useState<string>("zashboard");
  // 保存后的非阻塞提示（后端 SaveConfigView.warning）
  const [saveWarning, setSaveWarning] = useState<string | null>(null);

  // ---------- 核心管理 ----------
  const [cores, setCores] = useState<LocalCoreView[]>([]);
  const [coreVersions, setCoreVersions] = useState<string[]>([]);
  const [remoteVersions, setRemoteVersions] = useState<string[]>([]);
  const [downloadType, setDownloadType] = useState<CoreType>("singbox");
  const [downloadVersion, setDownloadVersion] = useState("");
  const [coresBusy, setCoresBusy] = useState(false);
  const [coresError, setCoresError] = useState<string | null>(null);
  const [coresMessage, setCoresMessage] = useState<string | null>(null);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (!config) {
      return;
    }
    setCoreType(normalizeCoreType(config.core_type));
    setCoreBinary(config.core_binary);
    setMixedPort(config.mixed_port);
    setMitmEnabled(config.mitm_enabled);
    setSystemProxyEnabled(config.system_proxy_enabled);
    setTunEnabled(config.tun_enabled);
    setTunStack(config.tun_stack);
    setTunAutoRoute(config.tun_auto_route);
    setClashApiEnabled(config.clash_api_enabled);
    setClashApiPort(config.clash_api_port);
    setClashApiSecret(config.clash_api_secret);
    setClashApiUi(config.clash_api_ui || "zashboard");
  }, [config]);

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

  /** 核心版本 Select 选项来源：当前类型已下载版本（语义化倒序）。 */
  const refreshDownloadedVersions = useCallback(async (coreType: CoreType) => {
    try {
      const versions = await listDownloadedVersions(coreType);
      setCoreVersions(versions);
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

  const handleSave = async () => {
    if (!config) {
      return;
    }
    setSaved(false);
    setSaveWarning(null);
    clearError();
    const payload: ClientConfig = {
      ...config,
      core_type: coreType,
      core_binary: coreBinary,
      mixed_port: mixedPort,
      mitm_enabled: mitmEnabled,
      system_proxy_enabled: systemProxyEnabled,
      tun_enabled: tunEnabled,
      tun_stack: tunStack,
      tun_auto_route: tunAutoRoute,
      clash_api_enabled: clashApiEnabled,
      clash_api_port: clashApiPort,
      clash_api_secret: clashApiSecret,
      clash_api_ui: clashApiUi,
    };
    try {
      const warning = await saveConfig(payload);
      setSaved(true);
      setSaveWarning(warning);
      // 重新加载以反映后端 core_type 联动后的 core_binary 回填。
      await loadConfig();
    } catch {
      // 错误已由 store 记录并展示。
    }
  };

  const handleUseCore = async (core: LocalCoreView) => {
    setCoresBusy(true);
    setCoresError(null);
    setCoresMessage(null);
    try {
      await setActiveCore(core.path);
      setCoresMessage(
        `已启用 ${CORE_LABELS[normalizeCoreType(core.core_type) as CoreType] ?? core.core_type} ${core.version}`,
      );
      await refreshCores();
      await loadConfig();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    } finally {
      setCoresBusy(false);
    }
  };

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

  /** 版本 Select 选中即启用：按「类型 + 版本」在本地清单中定位核心并调用 set_active_core。 */
  const handleSelectCoreVersion = async (version: string) => {
    if (!version) {
      return;
    }
    const core = cores.find((c) => normalizeCoreType(c.core_type) === normalizedCoreType && c.version === version);
    if (core) {
      await handleUseCore(core);
    }
  };

  // 后端已放宽校验：hub_url / sub_token 为空仅降级为 warning，不再阻塞开关/端口等基本设置保存。
  const canSave = true;
  const activeCore = cores.find((core) => core.active) ?? null;
  const normalizedCoreType = normalizeCoreType(coreType) as CoreType;
  // core_type 变更时的联动预览（实际回填由 save_config 完成后端按 preferred_binary 执行）。
  const coreTypeChanged = config ? normalizeCoreType(config.core_type) !== coreType : false;
  const linkedCore = coreTypeChanged ? preferredCoreFor(cores, normalizedCoreType) : null;

  useEffect(() => {
    void refreshDownloadedVersions(normalizedCoreType);
  }, [normalizedCoreType, refreshDownloadedVersions]);

  // 版本 Select 选项 = 当前类型已下载版本 + 当前 core_binary 版本（系统核心等不在下载列表时补入）。
  const activeCoreForType =
    activeCore && normalizeCoreType(activeCore.core_type) === normalizedCoreType ? activeCore : null;
  const coreVersionOptions = [...coreVersions];
  if (activeCoreForType && !coreVersionOptions.includes(activeCoreForType.version)) {
    coreVersionOptions.push(activeCoreForType.version);
  }
  const selectedCoreVersion = activeCoreForType?.version ?? "";

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端连接与核心运行配置</p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>基本配置</Card.Title>
          <Card.Description>保存后写入数据目录的 client.json</Card.Description>
        </Card.Header>
        <Card.Content>
          <div className="flex flex-col gap-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="flex flex-col gap-2">
                <Label>核心类型</Label>
                <Select value={coreType} onChange={(key) => setCoreType(String(key))} fullWidth>
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {CORE_TYPE_OPTIONS.map((option) => (
                        <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                          {option.label}
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
              </div>

              <div className="flex flex-col gap-2">
                <Label htmlFor="settings-core-version">核心版本</Label>
                <Select
                  id="settings-core-version"
                  value={selectedCoreVersion}
                  isDisabled={coreVersionOptions.length === 0}
                  onChange={(value) => void handleSelectCoreVersion(String(value ?? ""))}
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {coreVersionOptions.length === 0 ? (
                        <ListBox.Item id="__empty" textValue="暂无已下载版本">
                          暂无已下载版本
                        </ListBox.Item>
                      ) : (
                        coreVersionOptions.map((version) => (
                          <ListBox.Item key={version} id={version} textValue={version}>
                            {version}
                            <ListBox.ItemIndicator />
                          </ListBox.Item>
                        ))
                      )}
                    </ListBox>
                  </Select.Popover>
                </Select>
                <span className="text-xs text-muted">选择后立即启用该版本</span>
              </div>
            </div>

            {coreTypeChanged &&
              (linkedCore ? (
                <span className="text-xs text-muted">
                  保存后将自动使用 {CORE_LABELS[normalizedCoreType]} {linkedCore.version}（core_binary 联动回填）
                </span>
              ) : (
                <span className="text-xs text-warning">该类型暂无本地核心，保存后请到下方「核心管理」下载</span>
              ))}

            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-mixed-port">混合端口</Label>
              <Input
                id="settings-mixed-port"
                type="number"
                min={1}
                max={65535}
                value={String(mixedPort)}
                onChange={(event) => {
                  const parsed = Number(event.target.value);
                  setMixedPort(Number.isFinite(parsed) ? parsed : 0);
                }}
                fullWidth
              />
            </div>

            <div className="flex flex-col gap-3 rounded-xl border border-border/60 bg-surface p-4">
              <Switch isSelected={mitmEnabled} onChange={setMitmEnabled}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用 MITM
                </Switch.Content>
              </Switch>
              <Switch isSelected={systemProxyEnabled} onChange={setSystemProxyEnabled}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用系统代理
                </Switch.Content>
              </Switch>
            </div>

            {saveWarning && (
              <Alert status="warning">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>已保存，但有提示</Alert.Title>
                  <Alert.Description>{saveWarning}</Alert.Description>
                </Alert.Content>
              </Alert>
            )}
          </div>
        </Card.Content>
        <Card.Footer>
          <Button variant="primary" isPending={loading} isDisabled={!canSave} onPress={() => void handleSave()}>
            保存配置
          </Button>
          {saved && <span className="text-sm text-success">已保存</span>}
        </Card.Footer>
      </Card>

      {/* TUN 模式 */}
      <Card>
        <Card.Header>
          <Card.Title>TUN 模式</Card.Title>
          <Card.Description>虚拟网卡接管全部流量，需管理员/root 权限</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Switch isSelected={tunEnabled} onChange={setTunEnabled}>
            <Switch.Content>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
              启用 TUN 模式
            </Switch.Content>
          </Switch>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-tun-stack">协议栈</Label>
              <Select
                id="settings-tun-stack"
                value={tunStack}
                onChange={(value) => setTunStack(String(value ?? "mixed"))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {TUN_STACK_OPTIONS.map((option) => (
                      <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                        {option.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <div className="flex items-end">
              <Switch isSelected={tunAutoRoute} onChange={setTunAutoRoute}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  自动路由
                </Switch.Content>
              </Switch>
            </div>
          </div>

          <Alert status="default">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>权限说明</Alert.Title>
              <Alert.Description>
                TUN 模式需要管理员 / root 权限；设置页的 TUN / Clash 面板配置优先级高于协议配置中的复写
              </Alert.Description>
            </Alert.Content>
          </Alert>
        </Card.Content>
      </Card>

      {/* Clash 面板 */}
      <Card>
        <Card.Header>
          <Card.Title>Clash 面板</Card.Title>
          <Card.Description>通过本地面板 API 查看连接与切换节点</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Switch isSelected={clashApiEnabled} onChange={setClashApiEnabled}>
            <Switch.Content>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
              启用 Clash 面板 API
            </Switch.Content>
          </Switch>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-clash-port">端口</Label>
              <Input
                id="settings-clash-port"
                type="number"
                min={1}
                max={65535}
                value={String(clashApiPort)}
                onChange={(event) => {
                  const parsed = Number(event.target.value);
                  setClashApiPort(Number.isFinite(parsed) ? parsed : 0);
                }}
                fullWidth
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-clash-secret">密钥（可选）</Label>
              <Input
                id="settings-clash-secret"
                type="password"
                value={clashApiSecret}
                onChange={(event) => setClashApiSecret(event.target.value)}
                placeholder="留空则不鉴权"
                fullWidth
              />
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="settings-clash-ui">面板 UI</Label>
            <Select
              id="settings-clash-ui"
              value={clashApiUi}
              onChange={(value) => setClashApiUi(String(value ?? "zashboard"))}
              fullWidth
            >
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {CLASH_UI_OPTIONS.map((option) => (
                    <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                      {option.label}
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  ))}
                </ListBox>
              </Select.Popover>
            </Select>
            <span className="text-xs text-muted">首次访问面板地址时自动下载所选面板资源</span>
          </div>

          <Alert status="default">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>访问方式</Alert.Title>
              <Alert.Description>
                面板地址 http://127.0.0.1:{clashApiPort}/ui，默认 {clashApiUi}，可切换 yacd / metacubexd
              </Alert.Description>
            </Alert.Content>
          </Alert>
        </Card.Content>
      </Card>

      {/* 核心管理 */}
      <Card>
        <Card.Header>
          <Card.Title>核心管理</Card.Title>
          <Card.Description>下载、探测并选择启用的核心二进制（下载后需重新启动代理生效）</Card.Description>
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
                    <Chip size="sm" variant="soft" color={CORE_CHIP_COLORS[activeCore.core_type as CoreType]}>
                      {activeCore.version}
                    </Chip>
                  )}
                </span>
                <span className="truncate text-xs text-muted">{coreBinary || "未设置二进制路径"}</span>
              </div>
              {activeCore && (
                <Badge color="success" variant="soft" size="sm">
                  <Badge.Label>使用中</Badge.Label>
                </Badge>
              )}
            </div>
          </div>

          {/* 已安装核心 */}
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
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
                    const coreLabel = CORE_LABELS[core.core_type as CoreType] ?? core.core_type;
                    return (
                      <tr key={core.path} className="border-b border-border/40">
                        <td className="py-2 pr-3">{coreLabel}</td>
                        <td className="py-2 pr-3">{core.version}</td>
                        <td className="py-2 pr-3">
                          <Chip size="sm" variant="soft" color={core.source === "downloaded" ? "accent" : "warning"}>
                            {core.source === "downloaded" ? "下载" : "系统"}
                          </Chip>
                        </td>
                        <td className="max-w-[180px] truncate py-2 pr-3 text-xs text-muted">{core.path}</td>
                        <td className="py-2 text-right">
                          {core.active ? (
                            <Badge color="success" variant="soft" size="sm">
                              <Badge.Label>使用中</Badge.Label>
                            </Badge>
                          ) : (
                            <Button
                              size="sm"
                              variant="secondary"
                              isDisabled={coresBusy}
                              onPress={() => void handleUseCore(core)}
                            >
                              使用
                            </Button>
                          )}
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
              <Button
                variant="tertiary"
                isDisabled={coresBusy}
                onPress={() => void refreshRemoteVersions(downloadType)}
              >
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

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>保存失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
