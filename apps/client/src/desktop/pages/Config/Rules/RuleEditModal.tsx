import { useCallback, useMemo, useState } from "react";
import { Button, Checkbox, Input, Label, ListBox, Modal, Select } from "@heroui/react";
import type { LocalRuleInput, LocalRuleView, RuleOutboundOption } from "@pp/client-core";
import { buildOutboundAction, isOutboundAction, outboundTagFromAction, parseRuleSetTags } from "@pp/client-core";
import { RULE_ACTIONS } from "./types";

/** 规则集选择器选项（rule_set 匹配目标）：规则集是纯资源无启停，全部可用。 */
export interface RuleSetOption {
  /** 写入 `rule.target` 的原始值：自定义规则集 `tag`。 */
  value: string;
  /** 显示名（自定义规则集 tag）。 */
  label: string;
  /** 附加提示（如「原值保留」标注）。 */
  hint?: string;
}

export interface RuleEditModalProps {
  isOpen: boolean;
  onClose: () => void;
  initial?: LocalRuleView | null;
  onSave: (rule: LocalRuleInput) => void;
  /** 规则集选择器候选（仅 `rule_set` 匹配类型使用）；空数组时提示先添加规则集。 */
  ruleSetOptions?: RuleSetOption[];
  /** 指定出站动作的候选 tag（订阅节点 + 模板分组 + 切片出站并集，经共享 hook 装配）。 */
  outboundOptions?: RuleOutboundOption[];
  /** 生效订阅是否存在节点缓存：候选为空时据此区分「先同步订阅」与「先添加切片出站」引导。 */
  subscriptionCacheAvailable?: boolean;
}

/** 规则表单内容，key 由调用方控制，确保 initial 变化时重新挂载、状态重置。 */
function RuleEditForm({
  initial,
  onSave,
  onClose,
  ruleSetOptions,
  outboundOptions,
  subscriptionCacheAvailable,
}: {
  initial?: LocalRuleView | null;
  onSave: (rule: LocalRuleInput) => void;
  onClose: () => void;
  ruleSetOptions: RuleSetOption[];
  outboundOptions: RuleOutboundOption[];
  subscriptionCacheAvailable: boolean;
}) {
  const [matchType, setMatchType] = useState(initial?.match_type ?? "domain");
  const [target, setTarget] = useState(initial?.target ?? "");
  /** `rule_set` 匹配的已选 tag（多选）；其余匹配类型走 `target` 文本框。 */
  const [ruleSetTags, setRuleSetTags] = useState<string[]>(() =>
    initial?.match_type === "rule_set" ? parseRuleSetTags(initial.target) : [],
  );
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
   * 规则集多选候选：编辑已有 `rule_set` 规则时若其原 tag 不在候选中
   * （自定义规则集已删除/尚未添加），逐个追加为「原值保留」项——保持勾选态并标注，
   * 由用户决定取消或保留（保留且规则集不存在时，引用该 tag 的规则集不会注入）。
   */
  const effectiveRuleSetOptions = useMemo<RuleSetOption[]>(() => {
    if (matchType !== "rule_set") return ruleSetOptions;
    const known = new Set(ruleSetOptions.map((opt) => opt.value));
    const stale = ruleSetTags.filter((tag) => !known.has(tag));
    if (stale.length === 0) return ruleSetOptions;
    return [...ruleSetOptions, ...stale.map((tag) => ({ value: tag, label: tag, hint: "当前无此规则集（原值保留）" }))];
  }, [matchType, ruleSetOptions, ruleSetTags]);

  const handleActionChange = useCallback(
    (id: string) => {
      // 切到「指定出站」保留已选 tag（重新切回不丢选择）；其余动作直接写入 id。
      if (id === "outbound") {
        if (!isOutboundAction(action)) setAction(buildOutboundAction(""));
      } else {
        setAction(id);
      }
    },
    [action],
  );

  const isOutbound = isOutboundAction(action);
  const selectedOutboundTag = outboundTagFromAction(action);

  /**
   * 指定出站候选：编辑已有 outbound 规则时若其原 tag 不在候选中（订阅缓存为空 /
   * 出站已删除），追加「原值保留」项，与移动端语义一致。
   */
  const effectiveOutboundOptions = useMemo<RuleOutboundOption[]>(() => {
    const tag = selectedOutboundTag;
    if (!isOutbound || tag === "" || outboundOptions.some((opt) => opt.value === tag)) {
      return outboundOptions;
    }
    return [...outboundOptions, { value: tag, label: tag, hint: "当前无此出站（原值保留）" }];
  }, [isOutbound, selectedOutboundTag, outboundOptions]);

  /** 切换规则集勾选：追加保持点击顺序；取消移除该项。 */
  const toggleRuleSetTag = useCallback((tag: string) => {
    setRuleSetTags((tags) => (tags.includes(tag) ? tags.filter((item) => item !== tag) : [...tags, tag]));
  }, []);

  const handleSave = useCallback(() => {
    const now = Math.floor(Date.now() / 1000);
    onSave({
      id: initial?.id ?? crypto.randomUUID(),
      name: name.trim(),
      enabled: initial?.enabled ?? true,
      match_type: matchType,
      // rule_set 多选序列化为逗号分隔 target（对齐 Rust parse_rule_set_tags）。
      target: matchType === "final" ? "" : matchType === "rule_set" ? ruleSetTags.join(",") : target.trim(),
      action,
      no_resolve: noResolve,
      invert,
      note: note.trim(),
      created_at: initial?.created_at ?? now,
      sort_order: initial?.sort_order ?? 0,
    });
    onClose();
  }, [initial, matchType, target, ruleSetTags, action, name, noResolve, invert, note, onSave, onClose]);

  const isFinal = matchType === "final";
  const targetOk = isFinal || (matchType === "rule_set" ? ruleSetTags.length > 0 : target.trim().length > 0);
  const actionOk = !isOutbound || selectedOutboundTag.trim().length > 0;
  const canSave = targetOk && actionOk;

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
                {effectiveRuleSetOptions.length === 0 ? (
                  <span className="text-xs text-muted">当前没有可用规则集，请先在「规则集管理」中添加</span>
                ) : (
                  <div className="flex max-h-56 flex-col gap-2 overflow-y-auto rounded-lg border border-border/40 p-3">
                    {effectiveRuleSetOptions.map((opt) => (
                      <Checkbox
                        key={opt.value}
                        isSelected={ruleSetTags.includes(opt.value)}
                        onChange={() => toggleRuleSetTag(opt.value)}
                      >
                        <Checkbox.Content>
                          <Checkbox.Control>
                            <Checkbox.Indicator />
                          </Checkbox.Control>
                          <span className="flex min-w-0 flex-col">
                            <span className="truncate">{opt.label}</span>
                            {opt.hint && <span className="truncate text-xs text-muted">{opt.hint}</span>}
                          </span>
                        </Checkbox.Content>
                      </Checkbox>
                    ))}
                  </div>
                )}
                {effectiveRuleSetOptions.length === 0 ? null : ruleSetTags.length === 0 ? (
                  <span className="text-xs text-amber-500">请至少选择一个规则集</span>
                ) : (
                  <span className="text-xs text-muted">可多选；规则集随引用它的规则一同注入</span>
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
            value={isOutbound ? "outbound" : action}
            onChange={(value) => handleActionChange(String(value ?? "proxy"))}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {RULE_ACTIONS.map((opt) => (
                  <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
                    {opt.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
        </div>

        {isOutbound && (
          <div className="flex flex-col gap-1">
            <Label>出站</Label>
            {effectiveOutboundOptions.length === 0 ? (
              <span className="text-xs text-muted">
                {subscriptionCacheAvailable
                  ? "暂无可用出站，可先在 配置→自定义出站 添加"
                  : "未找到订阅节点缓存，请先同步订阅"}
              </span>
            ) : (
              <>
                <Select
                  aria-label="出站"
                  value={selectedOutboundTag}
                  onChange={(value) => setAction(buildOutboundAction(String(value ?? "")))}
                  placeholder="请选择出站"
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {effectiveOutboundOptions.map((opt) => (
                        <ListBox.Item key={opt.value} id={opt.value} textValue={opt.label}>
                          <span className="flex min-w-0 flex-col">
                            <span className="truncate">{opt.label}</span>
                            {opt.hint && <span className="truncate text-xs text-muted">{opt.hint}</span>}
                          </span>
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
                <span className="text-xs text-muted">命中该规则的流量将转发到所选出站</span>
              </>
            )}
          </div>
        )}

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

export function RuleEditModal({
  isOpen,
  onClose,
  initial,
  onSave,
  ruleSetOptions = [],
  outboundOptions = [],
  subscriptionCacheAvailable = false,
}: RuleEditModalProps) {
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
            outboundOptions={outboundOptions}
            subscriptionCacheAvailable={subscriptionCacheAvailable}
          />
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
