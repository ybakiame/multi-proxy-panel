import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Chip, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import {
  authorizeTun,
  deleteCore,
  detectSystemCores,
  downloadCore,
  listCores,
  listRemoteCoreVersions,
  platformInfo,
  toErrorMessage,
  tunAuthStatus,
} from "../api";
import type { ClientConfig, CoreType, LocalCoreView } from "../api";
import { useAppStore } from "../store";
import { toastError, toastSuccess, toastWarning } from "../toast";

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

export default function Settings() {
  const { config, error, loadConfig, saveConfig } = useAppStore();
  // 运行平台（Android 由 VpnService 接管，网络设置隐藏 TUN 部分）。
  const [os, setOs] = useState<string | null>(null);
  const [mixedPort, setMixedPort] = useState(1080);
  // TUN 模式
  const [tunEnabled, setTunEnabled] = useState(false);
  const [tunStack, setTunStack] = useState<string>("mixed");
  const [tunAutoRoute, setTunAutoRoute] = useState(true);
  // TUN 提权状态（`authorized` / `needs_auth` / `unsupported:<reason>`；关闭开关时为空）。
  const [tunAuth, setTunAuth] = useState<string | null>(null);
  const [tunAuthError, setTunAuthError] = useState<string | null>(null);
  const [tunAuthBusy, setTunAuthBusy] = useState(false);
  // Clash 面板
  const [clashApiEnabled, setClashApiEnabled] = useState(false);
  const [clashApiPort, setClashApiPort] = useState(9090);
  const [clashApiSecret, setClashApiSecret] = useState("");
  const [clashApiUi, setClashApiUi] = useState<string>("zashboard");
  // GitHub 访问
  const [githubProxyPrefix, setGithubProxyPrefix] = useState("");
  const [fetchViaLocalProxy, setFetchViaLocalProxy] = useState(false);
  // 输入类控件的防抖保存（500ms）。
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ---------- 核心管理 ----------
  const [cores, setCores] = useState<LocalCoreView[]>([]);
  const [remoteVersions, setRemoteVersions] = useState<string[]>([]);
  const [downloadType, setDownloadType] = useState<CoreType>("singbox");
  const [downloadVersion, setDownloadVersion] = useState("");
  const [coresBusy, setCoresBusy] = useState(false);
  const [coresError, setCoresError] = useState<string | null>(null);
  const [coresMessage, setCoresMessage] = useState<string | null>(null);

  useEffect(() => {
    void loadConfig();
    void platformInfo()
      .then((info) => setOs(info.os))
      .catch(() => {
        // 命令失败保持未知平台（按桌面渲染）。
      });
  }, [loadConfig]);

  useEffect(() => {
    if (!config) {
      return;
    }
    setMixedPort(config.mixed_port);
    setTunEnabled(config.tun_enabled);
    setTunStack(config.tun_stack);
    setTunAutoRoute(config.tun_auto_route);
    setClashApiEnabled(config.clash_api_enabled);
    setClashApiPort(config.clash_api_port);
    setClashApiSecret(config.clash_api_secret);
    setClashApiUi(config.clash_api_ui || "zashboard");
    setGithubProxyPrefix(config.github_proxy_prefix || "");
    setFetchViaLocalProxy(config.fetch_via_local_proxy);
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

  useEffect(() => {
    void refreshCores();
  }, [refreshCores]);

  useEffect(() => {
    void refreshRemoteVersions(downloadType);
  }, [downloadType, refreshRemoteVersions]);

  /**
   * 即改即存：以最新持久化配置为基底叠加补丁后调用 `save_config`。
   *
   * 读取 `useAppStore.getState().config` 而非闭包捕获，避免防抖保存时覆盖
   * 期间其它控件的更新；失败时回滚控件值（重新 loadConfig 同步）。保存结果
   * 通过全局 toast 反馈：有后端非阻塞提示走 warning，否则成功提示。
   */
  const persist = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = useAppStore.getState().config;
      if (!current) {
        return;
      }
      try {
        const warning = await saveConfig({ ...current, ...patch });
        if (warning) {
          toastWarning(warning);
        } else {
          toastSuccess("设置已保存");
        }
        await loadConfig();
      } catch (err) {
        toastError(toErrorMessage(err));
        await loadConfig();
      }
    },
    [saveConfig, loadConfig],
  );

  /** 输入类控件的防抖保存（500ms）。 */
  const persistDebounced = useCallback(
    (patch: Partial<ClientConfig>) => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => void persist(patch), 500);
    },
    [persist],
  );

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
      const coreLabel = CORE_LABELS[core.core_type as CoreType] ?? core.core_type;
      setCoresMessage(`已删除 ${coreLabel} ${core.version}`);
      await refreshCores();
    } catch (err) {
      setCoresError(toErrorMessage(err));
    } finally {
      setCoresBusy(false);
    }
  };

  // ---------- TUN 提权 ----------

  /** 查询当前核心的 TUN 提权状态（`authorized` / `needs_auth` / `unsupported:<reason>`）。 */
  const refreshTunAuth = useCallback(async () => {
    try {
      setTunAuth(await tunAuthStatus());
      setTunAuthError(null);
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    }
  }, []);

  // 开关启用时查询提权状态；关闭时清空展示。
  useEffect(() => {
    if (!tunEnabled) {
      setTunAuth(null);
      setTunAuthError(null);
      return;
    }
    void refreshTunAuth();
  }, [tunEnabled, refreshTunAuth]);

  /** 一键授权：成功后以返回的新状态刷新展示；失败展示后端错误（含 Linux 装 polkit / Windows 管理员重启指引）。 */
  const handleAuthorizeTun = async () => {
    setTunAuthBusy(true);
    setTunAuthError(null);
    try {
      setTunAuth(await authorizeTun());
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    } finally {
      setTunAuthBusy(false);
    }
  };

  const activeCore = cores.find((core) => core.active) ?? null;
  // 核心类型 / 二进制路径直接取配置（选择已迁移到首页，本页仅展示）。
  const normalizedCoreType = normalizeCoreType(config?.core_type ?? "singbox") as CoreType;
  const coreBinary = config?.core_binary ?? "";
  // `unsupported:<reason>` 状态中的不可用原因（仅展示用，无则保持 null）。
  const tunAuthReason = tunAuth?.startsWith("unsupported:") ? tunAuth.slice("unsupported:".length) : null;
  // Android 由 VpnService 接管 TUN：网络设置卡片隐藏 TUN 部分（保留混合端口）。
  const isAndroid = os === "android";

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端连接与核心运行配置</p>
      </div>

      <div className="flex items-center gap-3">
        <span className="text-xs text-muted">所有修改即时保存</span>
      </div>

      {/* 网络设置 */}
      <Card>
        <Card.Header>
          <Card.Title>网络设置</Card.Title>
          <Card.Description>{isAndroid ? "本地混合端口配置" : "本地混合端口与虚拟网卡（TUN）配置"}</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
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
                const next = Number.isFinite(parsed) ? parsed : 0;
                setMixedPort(next);
                persistDebounced({ mixed_port: next });
              }}
              fullWidth
            />
          </div>

          {/* TUN 为桌面专属设置：Android 由 VpnService 接管，隐藏 TUN 部分。 */}
          {!isAndroid && (
            <>
              <Switch
                isSelected={tunEnabled}
                onChange={(next) => {
                  setTunEnabled(next);
                  void persist({ tun_enabled: next });
                }}
              >
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用 TUN 模式
                </Switch.Content>
              </Switch>

              {tunEnabled && (
                <div className="flex flex-col gap-3">
                  {tunAuth === "authorized" && (
                    <div className="flex items-center gap-2">
                      <Chip size="sm" variant="soft" color="success">
                        已授权
                      </Chip>
                      <span className="text-xs text-muted">核心已具备 TUN 提权能力</span>
                    </div>
                  )}

                  {tunAuth === "needs_auth" && (
                    <Alert status="warning">
                      <Alert.Indicator />
                      <Alert.Content>
                        <Alert.Title>TUN 需要系统授权</Alert.Title>
                        <Alert.Description>
                          当前核心未获得 TUN 提权，授权后才能接管全部流量（失败时按错误提示处理：Linux 安装
                          polkit、Windows 以管理员身份重启应用）。
                        </Alert.Description>
                        <div className="mt-2">
                          <Button
                            variant="secondary"
                            size="sm"
                            isPending={tunAuthBusy}
                            onPress={() => void handleAuthorizeTun()}
                          >
                            立即授权
                          </Button>
                        </div>
                      </Alert.Content>
                    </Alert>
                  )}

                  {tunAuthReason && (
                    <Alert status="default">
                      <Alert.Indicator />
                      <Alert.Content>
                        <Alert.Title>TUN 授权不可用</Alert.Title>
                        <Alert.Description>{tunAuthReason}</Alert.Description>
                      </Alert.Content>
                    </Alert>
                  )}

                  {tunAuthError && (
                    <Alert status="danger">
                      <Alert.Indicator />
                      <Alert.Content>
                        <Alert.Title>授权失败</Alert.Title>
                        <Alert.Description>{tunAuthError}</Alert.Description>
                      </Alert.Content>
                    </Alert>
                  )}
                </div>
              )}

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="flex flex-col gap-2">
                  <Label htmlFor="settings-tun-stack">协议栈</Label>
                  <Select
                    id="settings-tun-stack"
                    value={tunStack}
                    onChange={(value) => {
                      const next = String(value ?? "mixed");
                      setTunStack(next);
                      void persist({ tun_stack: next });
                    }}
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
                  <Switch
                    isSelected={tunAutoRoute}
                    onChange={(next) => {
                      setTunAutoRoute(next);
                      void persist({ tun_auto_route: next });
                    }}
                  >
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
                    TUN 模式需要管理员 / root 权限；设置页的 TUN / Clash 面板配置优先级高于协议配置中的覆写
                  </Alert.Description>
                </Alert.Content>
              </Alert>
            </>
          )}
        </Card.Content>
      </Card>

      {/* GitHub 访问 */}
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
              value={githubProxyPrefix}
              onChange={(event) => {
                setGithubProxyPrefix(event.target.value);
                persistDebounced({ github_proxy_prefix: event.target.value });
              }}
              placeholder="https://gh-proxy.com"
              fullWidth
            />
            <span className="text-xs text-muted">
              GitHub 链接将拼接前缀访问，例如 https://gh-proxy.com/https://raw.githubusercontent.com/…；留空则直连
            </span>
          </div>
        </Card.Content>
      </Card>

      {/* Clash 面板 */}
      <Card>
        <Card.Header>
          <Card.Title>Clash 面板</Card.Title>
          <Card.Description>通过本地面板 API 查看连接与切换节点</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <Switch
            isSelected={clashApiEnabled}
            onChange={(next) => {
              setClashApiEnabled(next);
              void persist({ clash_api_enabled: next });
            }}
          >
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
                  const next = Number.isFinite(parsed) ? parsed : 0;
                  setClashApiPort(next);
                  persistDebounced({ clash_api_port: next });
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
                onChange={(event) => {
                  setClashApiSecret(event.target.value);
                  persistDebounced({ clash_api_secret: event.target.value });
                }}
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
              onChange={(value) => {
                const next = String(value ?? "zashboard");
                setClashApiUi(next);
                void persist({ clash_api_ui: next });
              }}
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

      {/* 核心管理：Android 核心为内置 libbox，无「选择核心二进制 / 下载 / 删除」概念，整卡隐藏。 */}
      {!isAndroid && (
        <Card>
          <Card.Header>
            <Card.Title>核心管理</Card.Title>
            <Card.Description>
              下载与管理核心二进制；在首页选择要使用的核心（下载/删除后需重启代理生效）
            </Card.Description>
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
                          <td className="py-2 pr-3">{core.version}</td>
                          <td className="py-2 pr-3">
                            <Chip size="sm" variant="soft" color={core.source === "downloaded" ? "accent" : "warning"}>
                              {core.source === "downloaded" ? "下载" : "系统"}
                            </Chip>
                          </td>
                          <td className="max-w-[180px] truncate py-2 pr-3 text-xs text-muted">{core.path}</td>
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
      )}

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
