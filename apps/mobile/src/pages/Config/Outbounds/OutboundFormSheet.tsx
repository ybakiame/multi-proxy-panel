import { useState } from "react";
import { TrashIcon } from "@heroicons/react/24/outline";
import { outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { BottomSheet, Button, Switch, inputClassName } from "../../../components/ui";
import { GroupFields } from "./GroupFields";
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
} from "@pp/client-core";
import { isGroupFormValid, validateGroupFields, type GroupMemberCandidate } from "@pp/client-core";
import { OUTBOUND_PROTOCOL_OPTIONS, isGroupProtocol, type OutboundProtocolType } from "@pp/client-core";

const inputClass = inputClassName;

interface OutboundFormSheetProps {
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
  defaultProtocol,
  memberCandidates,
  builtinMembersEditable = false,
  subscriptionCacheAvailable,
  onClose,
  onSave,
  onDeleteRequest,
}: OutboundFormSheetProps) {
  const [fields, setFields] = useState<OutboundFormFields>(defaultOutboundForm);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建默认表单 / 编辑预填）时同步草稿（adjust-state-during-render）。
  // 新建时把预选协议并入 key，保证「添加分组」与「添加节点」各自重置。
  const key = isOpen ? (editing?.id ?? `__new__:${defaultProtocol}`) : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setFields(editing ? outboundToForm(editing) : defaultOutboundForm(defaultProtocol));
  }

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
    <BottomSheet
      opened={isOpen}
      onClose={onClose}
      title={editing?.builtin ? "编辑内置分组" : editing ? "编辑自定义出站" : "添加自定义出站"}
      footer={
        <div className="flex gap-2">
          <Button variant="tertiary" className="min-h-12 flex-1" onPress={onClose}>
            取消
          </Button>
          <Button variant="primary" className="min-h-12 flex-1" isDisabled={!canSave} onPress={handleSave}>
            保存
          </Button>
        </div>
      }
    >
      <div className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
        {/* 名称（内置分组只读） */}
        <div className="flex flex-col gap-1.5">
          <label htmlFor="outbound-name" className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
            名称
          </label>
          {fields.builtin ? (
            <>
              <span className="font-mono text-sm text-zinc-900 dark:text-zinc-100">{fields.name}</span>
              <span className="text-xs text-zinc-500 dark:text-zinc-400">内置分组名称不可修改</span>
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
                className={inputClass}
              />
              {errors.name ? (
                <span className="text-xs text-amber-500">{errors.name}</span>
              ) : (
                <span className="text-xs text-zinc-500 dark:text-zinc-400">
                  渲染 tag：
                  <code className="font-mono text-zinc-900 dark:text-zinc-100">{outboundTag(fields.name)}</code>
                </span>
              )}
            </>
          )}
        </div>

        {/* 协议类型（内置分组只读） */}
        {!fields.builtin && (
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">协议类型</span>
            <MobileSelectSheet
              label="协议类型"
              value={fields.protocol}
              onChange={handleProtocolChange}
              options={OUTBOUND_PROTOCOL_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
            />
            <span className="text-xs text-zinc-500 dark:text-zinc-400">切换协议会重置该出站的协议字段</span>
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
        <div className="flex min-h-11 items-center justify-between gap-3">
          <span className="text-sm text-zinc-900 dark:text-zinc-100">启用该出站</span>
          <Switch aria-label="启用该出站" isSelected={fields.enabled} onValueChange={(enabled) => patch({ enabled })} />
        </div>

        {/* 编辑模式删除入口（内置分组不可删除） */}
        {editing && !editing.builtin && (
          <Button variant="danger" className="min-h-12 w-full" onPress={() => onDeleteRequest(editing)}>
            <TrashIcon className="size-4" aria-hidden="true" />
            删除出站
          </Button>
        )}
      </div>
    </BottomSheet>
  );
}
