import { useCallback, useMemo, useState } from "react";
import { Button, Checkbox, Input, Label, ListBox, Modal, Select } from "@heroui/react";
import type { LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { RULE_ACTIONS } from "./types";

/**
 * 桌面端规则动作列表：过滤掉「指定出站」。
 *
 * 共享的 `RULE_ACTIONS` 已新增 `outbound`（供移动端规则卡片消费），但桌面表单尚无
 * 出站 tag 选择器，直接展示会出现可保存却被后端拒绝的无效项。桌面出站选择器随 D3
 * 后置实现，届时再放开此项。
 */
const DESKTOP_RULE_ACTIONS = RULE_ACTIONS.filter((opt) => opt.id !== "outbound");

/** 规则集选择器选项（rule_set 匹配目标）：规则集是纯资源无启停，全部可用。 */
export interface RuleSetOption {
  /** 写入 `rule.target` 的原始值：自定义规则集 `tag`。 */
  value: string;
  /** 下拉显示名（自定义规则集 tag）。 */
  label: string;
}

export interface RuleEditModalProps {
  isOpen: boolean;
  onClose: () => void;
  initial?: LocalRuleView | null;
  onSave: (rule: LocalRuleInput) => void;
  /** 规则集选择器候选（仅 `rule_set` 匹配类型使用）；空数组时提示先添加规则集。 */
  ruleSetOptions?: RuleSetOption[];
}

/** 规则表单内容，key 由调用方控制，确保 initial 变化时重新挂载、状态重置。 */
function RuleEditForm({
  initial,
  onSave,
  onClose,
  ruleSetOptions,
}: {
  initial?: LocalRuleView | null;
  onSave: (rule: LocalRuleInput) => void;
  onClose: () => void;
  ruleSetOptions: RuleSetOption[];
}) {
  const [matchType, setMatchType] = useState(initial?.match_type ?? "domain");
  const [target, setTarget] = useState(initial?.target ?? "");
  const [action, setAction] = useState(initial?.action ?? "proxy");
  const [name, setName] = useState(initial?.name ?? "");
  const [note, setNote] = useState(initial?.note ?? "");
  const [noResolve, setNoResolve] = useState(initial?.no_resolve ?? false);
  const [invert, setInvert] = useState(initial?.invert ?? false);

  const matchTypeOptions = useMemo(
    () => [
      { id: "domain", label: "域名 (domain)" },
      { id: "domain_suffix", label: "域名后缀 (domain_suffix)" },
      { id: "domain_keyword", label: "域名关键词 (domain_keyword)" },
      { id: "ip_cidr", label: "IP 段 (ip_cidr)" },
      { id: "source_ip_cidr", label: "源 IP 段 (source_ip_cidr)" },
      { id: "rule_set", label: "规则集 (rule_set)" },
      { id: "port", label: "端口 (port)" },
      { id: "final", label: "最终规则 (final)" },
      { id: "process_name", label: "进程名 (process_name)" },
    ],
    [],
  );

  /**
   * 规则集选择器候选：编辑已有 `rule_set` 规则时若其原 target 不在候选中
   * （自定义规则集已删除/尚未添加），追加为「原值保留」项——选择器显示原值且不强清，
   * 由用户决定改选或保留。
   */
  const effectiveRuleSetOptions = useMemo<RuleSetOption[]>(() => {
    const stale =
      matchType === "rule_set" &&
      initial?.match_type === "rule_set" &&
      target.trim() !== "" &&
      !ruleSetOptions.some((opt) => opt.value === target);
    if (!stale) return ruleSetOptions;
    return [...ruleSetOptions, { value: target, label: target }];
  }, [matchType, initial, target, ruleSetOptions]);

  const handleSave = useCallback(() => {
    const now = Math.floor(Date.now() / 1000);
    onSave({
      id: initial?.id ?? crypto.randomUUID(),
      name: name.trim(),
      enabled: initial?.enabled ?? true,
      match_type: matchType,
      target: matchType === "final" ? "" : target.trim(),
      action,
      no_resolve: noResolve,
      invert,
      note: note.trim(),
      created_at: initial?.created_at ?? now,
      sort_order: initial?.sort_order ?? 0,
    });
    onClose();
  }, [initial, matchType, target, action, name, noResolve, invert, note, onSave, onClose]);

  const isFinal = matchType === "final";
  const canSave = isFinal ? true : target.trim().length > 0;

  return (
    <>
      <Modal.Body className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <Label>匹配类型</Label>
          <Select
            aria-label="匹配类型"
            value={matchType}
            onChange={(value) => setMatchType(String(value ?? "domain"))}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {matchTypeOptions.map((opt) => (
                  <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                    {opt.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
        </div>

        {!isFinal && (
          <div className="flex flex-col gap-1">
            <Label>{matchType === "rule_set" ? "规则集" : "匹配目标"}</Label>
            {matchType === "rule_set" ? (
              <>
                <Select
                  aria-label="规则集"
                  placeholder="请选择规则集"
                  value={target}
                  onChange={(value) => setTarget(String(value ?? ""))}
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {effectiveRuleSetOptions.map((opt) => (
                        <ListBox.Item key={opt.value} id={opt.value} textValue={opt.label}>
                          {opt.label}
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
                {effectiveRuleSetOptions.length === 0 ? (
                  <span className="text-xs text-muted">当前没有可用规则集，请先在「规则集管理」中添加</span>
                ) : (
                  <span className="text-xs text-muted">选择规则集 tag（规则集随引用它的规则一同注入）</span>
                )}
              </>
            ) : (
              <Input
                id="rule-target"
                aria-label="匹配目标"
                value={target}
                onChange={(e) => setTarget(e.target.value)}
                placeholder="例如：googleapis.com"
                fullWidth
              />
            )}
          </div>
        )}

        <div className="flex flex-col gap-1">
          <Label>路由动作</Label>
          <Select
            aria-label="路由动作"
            value={action}
            onChange={(value) => setAction(String(value ?? "proxy"))}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {DESKTOP_RULE_ACTIONS.map((opt) => (
                  <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                    {opt.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
        </div>

        <div className="flex flex-col gap-1">
          <Label htmlFor="rule-name">规则名称（可选）</Label>
          <Input
            id="rule-name"
            aria-label="规则名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="留空则自动生成摘要"
            fullWidth
          />
        </div>

        <div className="rounded-lg border border-border/40 p-3">
          <span className="text-xs font-medium text-muted">高级选项</span>
          <div className="mt-2 flex flex-col gap-2">
            <Checkbox isSelected={noResolve} onChange={(next) => setNoResolve(next)}>
              <Checkbox.Content>
                <Checkbox.Control>
                  <Checkbox.Indicator />
                </Checkbox.Control>
                跳过 DNS 解析 (no-resolve)
              </Checkbox.Content>
            </Checkbox>
            <Checkbox isSelected={invert} onChange={(next) => setInvert(next)}>
              <Checkbox.Content>
                <Checkbox.Control>
                  <Checkbox.Indicator />
                </Checkbox.Control>
                反选 (invert)
              </Checkbox.Content>
            </Checkbox>
            <div className="flex flex-col gap-1">
              <Label htmlFor="rule-note">备注</Label>
              <Input
                id="rule-note"
                aria-label="备注"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder="可选备注"
                fullWidth
              />
            </div>
          </div>
        </div>
      </Modal.Body>
      <Modal.Footer>
        <Button slot="close" variant="tertiary" onPress={onClose}>
          取消
        </Button>
        <Button variant="primary" isDisabled={!canSave} onPress={handleSave}>
          保存
        </Button>
      </Modal.Footer>
    </>
  );
}

export function RuleEditModal({ isOpen, onClose, initial, onSave, ruleSetOptions = [] }: RuleEditModalProps) {
  // key 确保 initial 变化时表单重新挂载、状态重置
  const formKey = initial?.id ?? "__new__";

  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[480px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{initial ? "编辑规则" : "新增规则"}</Modal.Heading>
          </Modal.Header>
          <RuleEditForm
            key={formKey}
            initial={initial}
            onSave={onSave}
            onClose={onClose}
            ruleSetOptions={ruleSetOptions}
          />
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
