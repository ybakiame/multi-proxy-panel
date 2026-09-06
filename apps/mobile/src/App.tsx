import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Alert, Button, Card, Chip } from "@heroui/react";
import {
  PROXY_STATUS_KEY,
  VPN_ERROR_KEY,
  isTauriEnv,
  requestVpnPermission,
  startProxy,
  stopProxy,
  toErrorMessage,
  useCapabilities,
  useProxyStatus,
  vpnLastError,
  type ClientStatus,
} from "@pp/client-core";

/** start_proxy 未获系统 VPN 授权时的错误前缀（Kotlin `vpn_not_authorized` reject，与 desktop 识别一致）。 */
const VPN_AUTH_MARKER = "vpn_not_authorized";

/** 非 Tauri 环境拦截：浏览器打开 devUrl 无 IPC 桥，任何 invoke 都失败；用原生元素渲染避免轮询失败刷屏（与 desktop 同理）。 */
function TauriRequired() {
  return (
    <div className="flex h-screen w-full flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
      <h1 className="text-xl font-semibold">请在客户端内运行</h1>
      <p className="max-w-md text-center text-sm text-muted">
        当前页面通过浏览器直接访问，缺少 Tauri 运行环境，所有本地命令不可用。 请使用{" "}
        <code className="rounded bg-default-100 px-1 py-0.5">bun run tauri dev</code> 启动移动客户端（端口
        1430），或运行已构建的 ProxyPanel 应用。
      </p>
    </div>
  );
}

/**
 * 移动端最小可用首页（ADR-0003 M3.6）：核心启停 + VPN 授权引导。
 *
 * - 状态：`useProxyStatus` 2s 轮询（desktop 同频）；Android 下核心在 Kotlin 后台异步启动，
 *   start_proxy resolve 不代表成功，真正失败仅写入 vpn_last_error，以同频 Query 兜底展示；
 * - 启停：大号主按钮 startProxy / stopProxy，成功后回写 PROXY_STATUS_KEY；
 * - VPN 授权：错误含 `vpn_not_authorized` →「去授权」（requestVpnPermission）。
 * - 完整交互（Router/订阅/核心管理）归 M5，此处不做。
 */
function Home() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const { data: capabilities } = useCapabilities();
  // capabilities 异步返回前为 undefined（移动壳仅 Android 目标）。
  const isAndroid = capabilities?.is_android ?? false;
  const running = status?.core_running ?? false;
  // 最近启停/授权的同步错误与授权成功标记（展示于下方面板）。
  const [actionError, setActionError] = useState<string | null>(null);
  const [authGranted, setAuthGranted] = useState(false);

  // Android 后台启动失败兜底：2s 轮询 vpn_last_error（与 proxy_status 同频）。
  const { data: vpnErrorData } = useQuery<string | null>({
    queryKey: VPN_ERROR_KEY,
    queryFn: vpnLastError,
    enabled: isAndroid,
    refetchInterval: 2000,
    retry: false,
  });
  const vpnError = vpnErrorData ?? null;

  const reportError = (err: unknown) => setActionError(toErrorMessage(err));
  const writeStatus = (next: ClientStatus) => {
    queryClient.setQueryData(PROXY_STATUS_KEY, next);
    setActionError(null);
  };

  const startMutation = useMutation({
    mutationFn: startProxy,
    onSuccess: writeStatus,
    onError: reportError,
  });
  const stopMutation = useMutation({
    mutationFn: stopProxy,
    onSuccess: writeStatus,
    onError: reportError,
  });
  // 系统 VPN 授权（request_vpn_permission → VpnService.prepare），成功后引导重试启动。
  const vpnAuthMutation = useMutation({
    mutationFn: requestVpnPermission,
    onSuccess: () => {
      setActionError(null);
      queryClient.setQueryData<string | null>(VPN_ERROR_KEY, null);
      setAuthGranted(true);
    },
    onError: reportError,
  });

  const handleStart = () => {
    // 新一轮启动先清空上轮失败展示（服务侧成功启动后也会清空 vpn_last_error）。
    setActionError(null);
    setAuthGranted(false);
    queryClient.setQueryData<string | null>(VPN_ERROR_KEY, null);
    startMutation.mutate();
  };

  const message = actionError ?? "";
  // 错误含 `vpn_not_authorized` → 仅显示「需要 VPN 授权」引导（对齐 desktop Dashboard）。
  const vpnAuthRequired = message.includes(VPN_AUTH_MARKER) || (vpnError ?? "").includes(VPN_AUTH_MARKER);
  const showActionError = message !== "" && !vpnAuthRequired;
  const showVpnError = vpnError !== null && !vpnAuthRequired;

  return (
    <div
      className="flex min-h-screen w-full flex-col bg-background text-foreground"
      style={{
        paddingTop: "max(1rem, env(safe-area-inset-top))",
        paddingBottom: "max(1rem, env(safe-area-inset-bottom))",
        paddingLeft: "max(1rem, env(safe-area-inset-left))",
        paddingRight: "max(1rem, env(safe-area-inset-right))",
      }}
    >
      <header className="px-2 pt-1">
        <h1 className="text-xl font-semibold">ProxyPanel</h1>
      </header>

      <main className="flex w-full flex-1 flex-col items-center justify-center gap-5 py-6">
        <div className="flex w-full max-w-sm flex-col gap-4">
          <Card>
            <Card.Header>
              <Card.Title>核心状态</Card.Title>
              <Card.Description>sing-box 内置核心 · 系统 VPN</Card.Description>
            </Card.Header>
            <Card.Content className="flex items-center justify-between gap-3">
              {running ? <Chip color="success">运行中</Chip> : <Chip color="danger">已停止</Chip>}
              <span className="text-sm text-muted">{running ? "代理流量经 VPN 隧道转发" : "点击下方按钮启动代理"}</span>
            </Card.Content>
          </Card>

          {vpnAuthRequired && (
            <Alert status="warning">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>需要 VPN 授权</Alert.Title>
                <Alert.Description>
                  代理启动失败：Android 系统尚未授权本应用创建 VPN。点击「去授权」完成系统授权后重新启动代理。
                </Alert.Description>
                <div className="mt-3">
                  <Button
                    variant="secondary"
                    size="lg"
                    className="min-h-11"
                    isPending={vpnAuthMutation.isPending}
                    onPress={() => vpnAuthMutation.mutate()}
                  >
                    去授权
                  </Button>
                </div>
              </Alert.Content>
            </Alert>
          )}

          {/* 启动失败：同步错误文本 + vpn_last_error（若有） */}
          {(showActionError || showVpnError) && (
            <Alert status="danger">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>启动失败</Alert.Title>
                {showActionError && <Alert.Description className="break-all">{actionError}</Alert.Description>}
                {showVpnError && <Alert.Description className="break-all">{vpnError}</Alert.Description>}
              </Alert.Content>
            </Alert>
          )}

          {authGranted && (
            <p className="text-center text-sm text-success">VPN 授权成功，请再次点击下方按钮启动代理。</p>
          )}

          {running ? (
            <Button
              variant="danger"
              size="lg"
              className="min-h-11"
              isPending={stopMutation.isPending}
              isDisabled={startMutation.isPending}
              onPress={() => stopMutation.mutate()}
            >
              停止代理
            </Button>
          ) : (
            <Button
              variant="primary"
              size="lg"
              className="min-h-11"
              isPending={startMutation.isPending}
              isDisabled={stopMutation.isPending || vpnAuthMutation.isPending}
              onPress={handleStart}
            >
              启动代理
            </Button>
          )}
        </div>
      </main>
    </div>
  );
}

export default function App() {
  if (!isTauriEnv) {
    return <TauriRequired />;
  }
  return <Home />;
}
