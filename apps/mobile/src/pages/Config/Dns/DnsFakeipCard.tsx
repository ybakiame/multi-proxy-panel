import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Card, Switch } from "@heroui/react";
import {
  CONFIG_KEY,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
  useClientConfig,
  useSaveConfig,
} from "@pp/client-core";
import type { ClientConfig } from "@pp/client-core";

/**
 * DNS 页 FakeIP 模式开关卡（W2，`ClientConfig.dns_fakeip_enabled`）。
 *
 * 独立于 DNS 切片草稿：数据源 `useClientConfig`（CONFIG_KEY），切换即读缓存最新基底
 * 叠加补丁并经 `useSaveConfig` 落盘（对齐 useSettingsConfig 的即时保存模式，避免
 * 展开渲染期快照造成 lost update）。仅跟随系统模式可见；接管模式由切片正文全权接管
 * （Rust 侧同样以非 takeover 为 FakeIP 生效前提）。
 */
export function DnsFakeipCard() {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const saveConfigMutation = useSaveConfig();
  const [saving, setSaving] = useState(false);

  const enabled = config?.dns_fakeip_enabled ?? false;

  const handleToggle = async (next: boolean) => {
    if (saving) return;
    // 读缓存最新基底叠加补丁，避免并发保存互相覆盖。
    const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
    if (!current) return;
    setSaving(true);
    try {
      const { warning } = await saveConfigMutation.mutateAsync({ ...current, dns_fakeip_enabled: next });
      if (warning) {
        toastWarning(warning);
      } else {
        toastSuccess("FakeIP 设置已保存");
      }
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card>
      <Card.Header>
        <Card.Title>FakeIP 模式</Card.Title>
        <Card.Description>
          <div className="flex w-full items-start justify-between gap-3">
            <span className="min-w-0 flex-1">
              DNS 查询立即返回虚拟 IP，连接时才按域名分流解析——可规避 DNS 污染与远程 DNS 故障，绝大多数场景推荐开启
            </span>
            <Switch
              aria-label="启用 FakeIP 模式"
              isSelected={enabled}
              isDisabled={!config || saving}
              onChange={(next) => void handleToggle(next)}
              className="shrink-0"
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
              </Switch.Content>
            </Switch>
          </div>
        </Card.Description>
      </Card.Header>
    </Card>
  );
}
