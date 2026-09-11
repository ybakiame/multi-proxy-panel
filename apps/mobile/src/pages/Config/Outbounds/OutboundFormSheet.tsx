import { useState } from "react";
import { TrashIcon } from "@heroicons/react/24/outline";
import { Button, Modal, Switch } from "@heroui/react";
import { outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { OutboundProtocolFields } from "./OutboundProtocolFields";
import { OutboundTlsFields } from "./OutboundTlsFields";
import { OutboundTransportFields } from "./OutboundTransportFields";
import {
  type OutboundFormFields,
  defaultOutboundForm,
  formToOutbound,
  isOutboundFormValid,
  outboundToForm,
  validateOutboundForm,
} from "./outboundForm";
import { OUTBOUND_PROTOCOL_OPTIONS, type OutboundProtocolType } from "./outboundOptions";

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

interface OutboundFormSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该出站（表单预填）。 */
  editing: CustomOutbound | null;
  /** 除自身外的已有出站名称（编辑时排除自身），用于 tag 冲突校验。 */
  otherNames: readonly string[];
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (item: CustomOutbound) => void;
  /** 编辑模式点「删除出站」：父层收起 Sheet 并弹 AlertDialog 确认。 */
  onDeleteRequest: (item: CustomOutbound) => void;
}

/**
 * 自定义出站编辑底部 Sheet（ADR-0005 P0-4c）。
 *
 * 通用字段：name / 协议类型 / enabled；协议字段按类型条件渲染（切换协议时重置为该
 * 协议默认值）；TLS 区块用于 vless / vmess / trojan / hysteria2，传输区块用于
 * vless / vmess / trojan。校验即时进行，非法时禁用保存并给出行内错误。
 */
export function OutboundFormSheet({
  isOpen,
  editing,
  otherNames,
  onClose,
  onSave,
  onDeleteRequest,
}: OutboundFormSheetProps) {
  const [fields, setFields] = useState<OutboundFormFields>(defaultOutboundForm);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建默认表单 / 编辑预填）时同步草稿（adjust-state-during-render）。
  const key = isOpen ? (editing?.id ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setFields(editing ? outboundToForm(editing) : defaultOutboundForm());
  }

  const errors = validateOutboundForm(fields, otherNames);
  const canSave = isOutboundFormValid(errors);

  const patch = (next: Partial<OutboundFormFields>) => setFields((current) => ({ ...current, ...next }));

  /** 切换协议：重置协议字段为该协议默认值，保留 name / enabled。 */
  const handleProtocolChange = (value: string) => {
    const type = value as OutboundProtocolType;
    setFields((current) => ({ ...defaultOutboundForm(type), name: current.name, enabled: current.enabled }));
  };

  const showTls = fields.protocol !== "shadowsocks";
  const showTransport = fields.protocol === "vless" || fields.protocol === "vmess" || fields.protocol === "trojan";

  const handleSave = () => {
    if (!canSave) return;
    onSave(formToOutbound(fields, editing?.id ?? crypto.randomUUID()));
    onClose();
  };

  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable
    >
      <Modal.Container placement="bottom">
        <Modal.Dialog>
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{editing ? "编辑自定义出站" : "添加自定义出站"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* 名称 */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="outbound-name" className="text-sm font-medium text-foreground">
                名称
              </label>
              <input
                id="outbound-name"
                aria-label="出站名称"
                aria-required="true"
                value={fields.name}
                onChange={(event) => patch({ name: event.target.value })}
                placeholder="例如：my-node"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className={inputClass}
              />
              {errors.name ? (
                <span className="text-xs text-warning">{errors.name}</span>
              ) : (
                <span className="text-xs text-muted">
                  渲染 tag：
                  <code className="font-mono text-foreground">{outboundTag(fields.name)}</code>
                </span>
              )}
            </div>

            {/* 协议类型 */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">协议类型</span>
              <MobileSelectSheet
                label="协议类型"
                value={fields.protocol}
                onChange={handleProtocolChange}
                options={OUTBOUND_PROTOCOL_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
              <span className="text-xs text-muted">切换协议会重置该出站的协议字段</span>
            </div>

            {/* 协议字段 */}
            <OutboundProtocolFields fields={fields} errors={errors} onChange={patch} />

            {/* TLS（shadowsocks 无 TLS） */}
            {showTls && <OutboundTlsFields fields={fields} onChange={patch} />}

            {/* 传输方式（仅 vless / vmess / trojan） */}
            {showTransport && <OutboundTransportFields fields={fields} onChange={patch} />}

            {/* 启用开关 */}
            <div className="flex min-h-11 items-center justify-between gap-3">
              <span className="text-sm text-foreground">启用该出站</span>
              <Switch
                aria-label="启用该出站"
                isSelected={fields.enabled}
                onChange={(enabled) => patch({ enabled })}
                className="shrink-0"
              >
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                </Switch.Content>
              </Switch>
            </div>

            {/* 编辑模式删除入口 */}
            {editing && (
              <Button variant="danger" className="min-h-12 w-full" onPress={() => onDeleteRequest(editing)}>
                <TrashIcon className="size-4" aria-hidden="true" />
                删除出站
              </Button>
            )}
          </Modal.Body>
          <Modal.Footer>
            <Button variant="tertiary" className="min-h-12 flex-1" onPress={onClose}>
              取消
            </Button>
            <Button variant="primary" className="min-h-12 flex-1" isDisabled={!canSave} onPress={handleSave}>
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
