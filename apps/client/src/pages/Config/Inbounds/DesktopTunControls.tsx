import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSetAtom } from "jotai";
import { Alert, Button, Chip, Switch } from "@pp/ui";
import {
  authorizeTun,
  tunAuthStatus,
  TUN_AUTH_KEY,
  toErrorMessage,
  useClientConfig,
  useSettingsPersist,
  lastActionErrorAtom,
} from "@pp/client-core";

/** 桌面可选 TUN 与系统授权，Android 构建裁剪整个组件。 */
export default function DesktopTunControls() {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const engine = useSettingsPersist();
  const tunEnabled = config?.tun_enabled ?? false;
  const { data: tunAuth = null } = useQuery({
    queryKey: TUN_AUTH_KEY,
    queryFn: tunAuthStatus,
    enabled: tunEnabled,
    retry: false,
  });
  const [tunAuthBusy, setTunAuthBusy] = useState(false);
  const [tunAuthError, setTunAuthError] = useState<string | null>(null);
  const setLastError = useSetAtom(lastActionErrorAtom);
  const tunAuthReason = tunAuth?.startsWith("unsupported:") ? tunAuth.slice("unsupported:".length) : null;
  const toggleTun = async (next: boolean) => {
    if (await engine.persist({ tun_enabled: next }, "inbounds")) {
      if (!next) setLastError((current) => (current?.includes("tun_auth_required") ? null : current));
    }
  };
  const handleAuthorizeTun = async () => {
    setTunAuthBusy(true);
    setTunAuthError(null);
    try {
      queryClient.setQueryData(TUN_AUTH_KEY, await authorizeTun());
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    }
    setTunAuthBusy(false);
  };
  return (
    <>
      <Switch isSelected={tunEnabled} onChange={(next) => void toggleTun(next)} isDisabled={!config || engine.saving}>
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
          启用 TUN 模式
        </Switch.Content>
      </Switch>

      {/* TUN 授权区：仅启用时展示（未启用 TUN 不需要任何提权）。 */}
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
                  当前核心未获得 TUN 提权，授权后才能接管全部流量（失败时按错误提示处理：Linux 安装 polkit、Windows
                  以管理员身份重启应用）。
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

      <Alert status="default">
        <Alert.Content>
          <Alert.Title>权限说明</Alert.Title>
          <Alert.Description>TUN 模式需要管理员 / root 权限；仅开启 TUN 后启动代理才需要授权。</Alert.Description>
        </Alert.Content>
      </Alert>
    </>
  );
}
