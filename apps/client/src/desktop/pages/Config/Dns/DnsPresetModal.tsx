import { Button, Chip, Modal } from "@heroui/react";
import { GlobeAltIcon, HomeModernIcon, PencilSquareIcon, Square2StackIcon } from "@heroicons/react/24/outline";
import { DNS_SERVER_PRESETS, dnsServerTypeLabel } from "@pp/client-core";
import type { DnsServerPreset } from "@pp/client-core";

const GROUPS: { key: DnsServerPreset["group"]; label: string; icon: typeof HomeModernIcon }[] = [
  { key: "builtin", label: "内置默认", icon: Square2StackIcon },
  { key: "cn", label: "境内常用", icon: HomeModernIcon },
  { key: "global", label: "境外常用", icon: GlobeAltIcon },
];

interface DnsPresetModalProps {
  isOpen: boolean;
  /** 已有服务器 tag（预置 tag 冲突时追加序号，由父层处理；此处仅标注已添加项）。 */
  existingTags: readonly string[];
  onClose: () => void;
  /** 点选预置项（父层物化为切片服务器并关闭本弹窗）。 */
  onPick: (preset: DnsServerPreset) => void;
  /** 切换为自定义添加（父层关闭本弹窗并打开编辑表单）。 */
  onCustom: () => void;
}

/**
 * 添加 DNS 服务器入口弹窗（桌面端；语义对齐移动端 `DnsServerPresetSheet`）：
 * 内置常用服务器目录（内置默认 / 境内 / 境外分组）+ 底部「自定义添加」切换。
 *
 * 预置项点选即加入（tag 冲突由父层追加序号）；已存在相同 tag 的预置项标注
 * 「已添加」仍可重复点选（自动得 `-2` 序号 tag）。
 */
export function DnsPresetModal({ isOpen, existingTags, onClose, onPick, onCustom }: DnsPresetModalProps) {
  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[560px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>添加 DNS 服务器</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {GROUPS.map(({ key, label, icon: Icon }) => (
              <div key={key} className="flex flex-col gap-2">
                <span className="flex items-center gap-1.5 text-xs font-medium text-muted">
                  <Icon className="size-4" aria-hidden="true" />
                  {label}
                </span>
                {DNS_SERVER_PRESETS.filter((preset) => preset.group === key).map((preset) => {
                  const added = existingTags.includes(preset.tag);
                  return (
                    <button
                      key={`${preset.tag}-${preset.server}`}
                      type="button"
                      onClick={() => onPick(preset)}
                      aria-label={`添加 ${preset.isp} ${preset.server}`}
                      className="flex items-center gap-3 rounded-xl border border-border/60 bg-surface px-3 py-2.5 text-left transition-colors hover:bg-surface-secondary/40"
                    >
                      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="truncate text-sm font-medium text-foreground">{preset.isp}</span>
                          <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                            {dnsServerTypeLabel(preset.serverType)}
                          </Chip>
                          {added && <span className="shrink-0 text-xs text-muted">已添加</span>}
                        </span>
                        <span className="truncate font-mono text-xs text-muted">
                          {preset.server}
                          {preset.serverPort !== null ? `:${preset.serverPort}` : ""}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>
            ))}

            <Button variant="secondary" className="w-full gap-1.5" onPress={onCustom}>
              <PencilSquareIcon className="size-4" aria-hidden="true" />
              自定义添加
            </Button>
          </Modal.Body>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
