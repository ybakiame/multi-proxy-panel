import { Card } from "@heroui/react";
import type { DnsMode } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { DNS_MODE_OPTIONS } from "./dnsUtils";

interface DnsModeCardProps {
  mode: DnsMode;
  /** 仅 Android 展示模式行（桌面恒 takeover 语义）。 */
  isAndroid: boolean;
  onChangeMode: (mode: DnsMode) => void;
}

/**
 * DNS 页 2 区：平台 DNS 模式（ADR-0005 D1）。
 *
 * - Android：显式「跟随系统 / 接管」选择；默认跟随系统（维持 `inject_android_dns`）。
 *   跟随系统时下方 DNS 正文不生效，给出提示但正文仍允许编辑保存。
 * - 桌面：恒 takeover 语义，不渲染该卡。
 */
export function DnsModeCard({ mode, isAndroid, onChangeMode }: DnsModeCardProps) {
  if (!isAndroid) {
    return null;
  }

  return (
    <Card>
      <Card.Header>
        <Card.Title>DNS 模式</Card.Title>
        <Card.Description>Android 上选择跟随系统或由本切片接管</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-2">
        <MobileSelectSheet
          label="DNS 模式"
          value={mode}
          onChange={(value) => onChangeMode(value as DnsMode)}
          options={DNS_MODE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
        />
        {mode === "follow_system" ? (
          <span className="text-xs text-warning">跟随系统模式下，下方 DNS 配置不会生效</span>
        ) : (
          <span className="text-xs text-muted">接管模式下本切片 DNS 正文生效，请确保配置有效</span>
        )}
      </Card.Content>
    </Card>
  );
}
