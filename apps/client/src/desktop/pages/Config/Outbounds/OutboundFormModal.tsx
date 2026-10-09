import { useState } from "react";
import { Button, Label, ListBox, Modal, Select } from "@heroui/react";
import { TrashIcon } from "@heroicons/react/24/outline";
import { outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import {
  type OutboundFormFields,
  defaultOutboundForm,
  formToOutbound,
  isOutboundFormValid,
  outboundToForm,
  validateOutboundForm,
} from "@pp/client-core";
import { isGroupFormValid, validateGroupFields, type GroupMemberCandidate } from "@pp/client-core";
import { OUTBOUND_PROTOCOL_OPTIONS, isGroupProtocol, type OutboundProtocolType } from "@pp/client-core";
import { GroupFields } from "./GroupFields";
import { OutboundProtocolFields } from "./OutboundProtocolFields";
import { OutboundTlsFields } from "./OutboundTlsFields";
import { OutboundTransportFields } from "./OutboundTransportFields";
import { SwitchRow } from "./OutboundField";

interface OutboundFormModalProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该出站（表单预填）。 */
  editing: CustomOutbound | null;
  /** 除自身外的已有出站名称（编辑时排除自身），用于 tag 冲突校验。 */
  otherNames: readonly string[];
  /** 新建时预选协议（分组区「添加分组」→ selector，节点区「添加节点」→ vless）。 */
  defaultProtocol: OutboundProtocolType;
  /** 分组成员候选（静态订阅节点 + 切片节点 + direct，不含其它分组）。 */
  memberCandidates: GroupMemberCandidate[];
  /** 内置静态成员分组（global/final）为 true：成员可编辑。 */
  builtinMembersEditable?: boolean;
  /** 生效订阅是否存在节点缓存（决定成员候选提示文案）。 */
  subscriptionCacheAvailable: boolean;
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (item: CustomOutbound) => void;
  /** 编辑模式点「删除出站」：父层收起弹窗并弹 AlertDialog 确认。 */
  onDeleteRequest: (item: CustomOutbound) => void;
}

/**
 * 自定义出站编辑弹窗（桌面端；语义对齐移动端 `OutboundFormSheet`）。
 *
 * 通用字段：name / 协议类型 / enabled；协议字段按类型条件渲染（切换协议时重置为
 * 该协议默认值）；TLS 区块用于 vless / vmess / trojan / hysteria2，传输区块用于
 * vless / vmess / trojan。校验即时进行，非法时禁用保存并给出行内错误。
 */
export function OutboundFormModal({
  isOpen,
  editing,
  otherNames,
  defaultProtocol,
  memberCandidates,
  builtinMembersEditable = false,
  subscriptionCacheAvailable,
  onClose,
  onSave,
  onDeleteRequest,
}: OutboundFormModalProps) {
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
            <Modal.Heading>
              {editing?.builtin ? "编辑内置分组" : editing ? "编辑自定义出站" : "添加自定义出站"}
            </Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            <OutboundForm
              key={editing?.id ?? `__new__:${defaultProtocol}`}
              editing={editing}
              otherNames={otherNames}
              defaultProtocol={defaultProtocol}
              memberCandidates={memberCandidates}
              builtinMembersEditable={builtinMembersEditable}
              subscriptionCacheAvailable={subscriptionCacheAvailable}
              onSave={onSave}
              onClose={onClose}
              onDeleteRequest={onDeleteRequest}
            />
          </Modal.Body>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

function OutboundForm({
  editing,
  otherNames,
  defaultProtocol,
  memberCandidates,
  builtinMembersEditable,
  subscriptionCacheAvailable,
  onSave,
  onClose,
  onDeleteRequest,
}: {
  editing: CustomOutbound | null;
  otherNames: readonly string[];
  defaultProtocol: OutboundProtocolType;
  memberCandidates: GroupMemberCandidate[];
  builtinMembersEditable: boolean;
  subscriptionCacheAvailable: boolean;
  onSave: (item: CustomOutbound) => void;
  onClose: () => void;
  onDeleteRequest: (item: CustomOutbound) => void;
}) {
  const [fields, setFields] = useState<OutboundFormFields>(() =>
    editing ? outboundToForm(editing) : defaultOutboundForm(defaultProtocol),
  );

  const errors = validateOutboundForm(fields, otherNames);
  const groupErrors = validateGroupFields(fields);
  const isGroup = isGroupProtocol(fields.protocol);
  const canSave = isOutboundFormValid(errors) && (!isGroup || isGroupFormValid(groupErrors));

  const patch = (next: Partial<OutboundFormFields>) => setFields((current) => ({ ...current, ...next }));

  /** 切换协议：重置协议字段为该协议默认值，保留 name / enabled（分组字段与节点字段互不残留）。 */
  const handleProtocolChange = (value: string) => {
    const type = value as OutboundProtocolType;
    setFields((current) => ({ ...defaultOutboundForm(type), name: current.name, enabled: current.enabled }));
  };

  const showTls = !isGroup && fields.protocol !== "shadowsocks";
  const showTransport =
    !isGroup && (fields.protocol === "vless" || fields.protocol === "vmess" || fields.protocol === "trojan");

  const handleSave = () => {
    if (!canSave) return;
    onSave(formToOutbound(fields, editing?.id ?? crypto.randomUUID()));
    onClose();
  };

  return (
    <>
      {/* 名称（内置分组只读） */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="outbound-name">名称</Label>
        {fields.builtin ? (
          <>
            <span className="font-mono text-sm text-foreground">{fields.name}</span>
            <span className="text-xs text-muted">内置分组名称不可修改</span>
          </>
        ) : (
          <>
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
              className="rounded-lg border border-border/60 bg-surface px-3 py-2 text-sm"
            />
            {errors.name ? (
              <span className="text-xs text-amber-500">{errors.name}</span>
            ) : (
              <span className="text-xs text-muted">
                渲染 tag：<code className="font-mono text-foreground">{outboundTag(fields.name)}</code>
              </span>
            )}
          </>
        )}
      </div>

      {/* 协议类型（内置分组只读） */}
      {!fields.builtin && (
        <div className="flex flex-col gap-1.5">
          <Label>协议类型</Label>
          <ProtocolSelect value={fields.protocol} onChange={handleProtocolChange} />
          <span className="text-xs text-muted">切换协议会重置该出站的协议字段</span>
        </div>
      )}

      {/* 协议字段：分组出站渲染成员/分组字段，节点出站渲染协议字段 */}
      {isGroup ? (
        <GroupFields
          fields={fields}
          errors={groupErrors}
          candidates={memberCandidates}
          subscriptionCacheAvailable={subscriptionCacheAvailable}
          builtinMembersEditable={builtinMembersEditable}
          onChange={patch}
        />
      ) : (
        <OutboundProtocolFields fields={fields} errors={errors} onChange={patch} />
      )}

      {/* TLS（shadowsocks 无 TLS） */}
      {showTls && <OutboundTlsFields fields={fields} onChange={patch} />}

      {/* 传输方式（仅 vless / vmess / trojan） */}
      {showTransport && <OutboundTransportFields fields={fields} onChange={patch} />}

      {/* 启用开关 */}
      <SwitchRow
        label="启用该出站"
        ariaLabel="启用该出站"
        isSelected={fields.enabled}
        onChange={(enabled) => patch({ enabled })}
      />

      <div className="flex gap-2">
        {editing && !editing.builtin && (
          <Button variant="danger" onPress={() => onDeleteRequest(editing)}>
            <TrashIcon className="size-4" aria-hidden="true" />
            删除出站
          </Button>
        )}
        <div className="flex-1" />
        <Button variant="tertiary" onPress={onClose}>
          取消
        </Button>
        <Button variant="primary" isDisabled={!canSave} onPress={handleSave}>
          保存
        </Button>
      </div>
    </>
  );
}

/** 协议类型下拉（独立小组件控制主表单规模）。 */
function ProtocolSelect({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <Select aria-label="协议类型" value={value} onChange={(key) => onChange(String(key ?? ""))} fullWidth>
      <Select.Trigger>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {OUTBOUND_PROTOCOL_OPTIONS.map((option) => (
            <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
              {option.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
