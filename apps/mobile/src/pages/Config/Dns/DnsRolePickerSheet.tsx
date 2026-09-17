import { GlobeAltIcon, HomeModernIcon, Square2StackIcon } from "@heroicons/react/24/outline";
import { Chip, Modal } from "@heroui/react";
import { DNS_SERVER_PRESETS } from "@pp/client-core";
import type { DnsServer, DnsServerPreset } from "@pp/client-core";
import { DNS_ROLE_LABELS, DNS_ROLE_PRESET_GROUPS, dnsRoleTag, dnsServerSummary, dnsServerTypeLabel } from "./dnsUtils";

const GROUP_META: Record<DnsServerPreset["group"], { label: string; icon: typeof HomeModernIcon }> = {
  builtin: { label: "内置默认", icon: Square2StackIcon },
  cn: { label: "境内常用", icon: HomeModernIcon },
  global: { label: "境外常用", icon: GlobeAltIcon },
};

interface DnsRolePickerSheetProps {
  /** 目标角色服务器（tag 为 `local` / `proxy`）；`null` = 关闭。 */
  target: DnsServer | null;
  onClose: () => void;
  /** 点选预置项：父层原位替换该服务器（tag 与引用它的规则、final 均不变）。 */
  onPick: (target: DnsServer, preset: DnsServerPreset) => void;
}

/** 预置项是否即当前生效地址（类型 + 地址 + 端口一致 → 标注「当前使用」）。 */
function isCurrentServer(preset: DnsServerPreset, server: DnsServer): boolean {
  return (
    preset.serverType === server.server_type &&
    preset.server === server.server &&
    preset.serverPort === server.server_port
  );
}

/**
 * 内置角色服务器（`local` / `proxy`）更换 Sheet。
 *
 * 点击这两行不再进入编辑表单：弹出按角色过滤的 DNS 服务器目录
 * （local → 内置默认 + 境内常用；proxy → 内置默认 + 境外常用），点选即原位替换——
 * tag 与引用它的分流规则、final 保持不变，保存草稿后生效。
 */
export function DnsRolePickerSheet({ target, onClose, onPick }: DnsRolePickerSheetProps) {
  const role = target ? dnsRoleTag(target.tag) : null;
  const groups = role ? DNS_ROLE_PRESET_GROUPS[role] : [];

  return (
    <Modal.Backdrop
      isOpen={target !== null && role !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable
    >
      <Modal.Container placement="bottom">
        <Modal.Dialog>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{role ? `更换${DNS_ROLE_LABELS[role]}解析服务器` : "选择 DNS 服务器"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {target && (
              <p className="text-xs leading-relaxed text-muted">
                当前：{dnsServerSummary(target)}。点选下方服务器后原位替换（名称与地址更新，tag 不变，引用它的
                分流规则与 final 不受影响），保存后生效。
              </p>
            )}
            {groups.map((group) => {
              const { label, icon: Icon } = GROUP_META[group];
              return (
                <div key={group} className="flex flex-col gap-2">
                  <span className="flex items-center gap-1.5 text-xs font-medium text-muted">
                    <Icon className="size-4" aria-hidden="true" />
                    {label}
                  </span>
                  {DNS_SERVER_PRESETS.filter((preset) => preset.group === group).map((preset) => {
                    const current = target !== null && isCurrentServer(preset, target);
                    return (
                      <button
                        key={`${preset.tag}-${preset.server}`}
                        type="button"
                        onClick={() => target && onPick(target, preset)}
                        aria-label={`更换为 ${preset.isp} ${preset.server}`}
                        className="flex min-h-12 items-center gap-3 rounded-xl border border-border/60 px-3 text-left active:opacity-70"
                      >
                        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                          <span className="flex min-w-0 items-center gap-2">
                            <span className="truncate text-sm font-medium text-foreground">{preset.isp}</span>
                            <span className="shrink-0 rounded-full bg-accent/10 px-2 py-0.5 text-xs font-medium leading-5 text-accent">
                              {dnsServerTypeLabel(preset.serverType)}
                            </span>
                            {preset.detour && <span className="shrink-0 text-xs text-muted">经代理</span>}
                            {current && (
                              <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                                当前使用
                              </Chip>
                            )}
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
              );
            })}
          </Modal.Body>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
