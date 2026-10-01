import { useMemo, useState } from "react";
import { Button, Checkbox, Input, Label, ListBox, Modal, Select, Switch } from "@heroui/react";
import { TrashIcon } from "@heroicons/react/24/outline";
import type { DnsMatchType, DnsRule, DnsRuleAction, RuleSetOption } from "@pp/client-core";
import { parseRuleSetTags } from "@pp/client-core";
import {
  DEFAULT_DNS_RCODE,
  DNS_CLASH_MODE_OPTIONS,
  DNS_MATCH_TYPE_OPTIONS,
  DNS_RCODE_OPTIONS,
  DNS_RULE_ACTION_OPTIONS,
  DNS_TARGET_PLACEHOLDER,
  firstInvalidQueryType,
  isValidRcode,
} from "@pp/client-core";

/** server_tag 下拉选项（页面从已定义 server tag 生成）。 */
export interface DnsServerOption {
  value: string;
  label: string;
  description?: string;
}

interface DnsRuleFormModalProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该规则（表单预填）。 */
  editing: DnsRule | null;
  /** 已定义 server tag 选项；空数组时提示先添加服务器。 */
  serverOptions: DnsServerOption[];
  /** 规则集选择器候选（仅 `rule_set` 匹配类型使用）；空数组时提示先在「规则」页添加。 */
  ruleSetOptions?: RuleSetOption[];
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (rule: DnsRule) => void;
  /** 编辑模式点「删除规则」：父层收起 Modal 并弹 AlertDialog 确认。 */
  onDeleteRequest: (rule: DnsRule) => void;
}

/**
 * DNS 分流规则编辑弹窗（桌面端；语义对齐移动端 `DnsRuleFormSheet`）。
 *
 * 字段：match_type / target / action / server_tag / rcode / enabled。`rule_set`
 * 匹配为多选勾选列表（逗号分隔 target 渲染为 sing-box 数组）；校验：target 非空
 * （rule_set 至少一项）、route 动作 server_tag 必须指向已定义 server，非法时禁用
 * 保存并给出行内错误。
 */
export function DnsRuleFormModal({
  isOpen,
  editing,
  serverOptions,
  ruleSetOptions = [],
  onClose,
  onSave,
  onDeleteRequest,
}: DnsRuleFormModalProps) {
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
        <Modal.Dialog className="sm:max-w-[520px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{editing ? "编辑 DNS 规则" : "添加 DNS 规则"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            <DnsRuleForm
              key={editing?.id ?? "__new__"}
              editing={editing}
              serverOptions={serverOptions}
              ruleSetOptions={ruleSetOptions}
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

function DnsRuleForm({
  editing,
  serverOptions,
  ruleSetOptions,
  onSave,
  onClose,
  onDeleteRequest,
}: {
  editing: DnsRule | null;
  serverOptions: DnsServerOption[];
  ruleSetOptions: RuleSetOption[];
  onSave: (rule: DnsRule) => void;
  onClose: () => void;
  onDeleteRequest: (rule: DnsRule) => void;
}) {
  const [matchType, setMatchType] = useState<DnsMatchType>(editing?.match_type ?? "domain");
  const [target, setTarget] = useState(editing?.target ?? "");
  /** `rule_set` 匹配的已选 tag（多选，对齐路由规则编辑）；其余匹配类型走 `target`。 */
  const [ruleSetTags, setRuleSetTags] = useState<string[]>(
    editing?.match_type === "rule_set" ? parseRuleSetTags(editing.target) : [],
  );
  const [serverTag, setServerTag] = useState(editing?.server_tag ?? "");
  const [action, setAction] = useState<DnsRuleAction>(editing?.action ?? "route");
  const [rcode, setRcode] = useState(editing?.rcode?.trim() ? editing.rcode.trim().toUpperCase() : DEFAULT_DNS_RCODE);
  const [enabled, setEnabled] = useState(editing?.enabled ?? true);

  /**
   * 编辑已有规则时若其 server_tag 不在候选中（服务器已删除/尚未添加），追加
   * 「原值保留」项，避免选择器显示占位并强制清空；用户可改选或保留。
   */
  const effectiveServerOptions = useMemo<DnsServerOption[]>(() => {
    const stale =
      editing != null &&
      editing.server_tag.trim() !== "" &&
      !serverOptions.some((option) => option.value === editing.server_tag);
    if (!stale) {
      return serverOptions;
    }
    return [
      ...serverOptions,
      { value: editing.server_tag, label: editing.server_tag, description: "当前无此服务器（原值保留）" },
    ];
  }, [editing, serverOptions]);

  /**
   * 规则集多选候选：编辑存量 rule_set 规则时若其原 tag 不在候选中（自定义规则集已删
   * 除/尚未添加），追加「原值保留」项——保持勾选态并标注，由用户决定取消或保留。
   */
  const effectiveRuleSetOptions = useMemo<RuleSetOption[]>(() => {
    if (matchType !== "rule_set") return ruleSetOptions;
    const known = new Set(ruleSetOptions.map((option) => option.value));
    const stale = ruleSetTags.filter((tag) => !known.has(tag));
    if (stale.length === 0) return ruleSetOptions;
    return [...ruleSetOptions, ...stale.map((tag) => ({ value: tag, label: tag, hint: "当前无此规则集（原值保留）" }))];
  }, [matchType, ruleSetOptions, ruleSetTags]);

  /** 切换规则集勾选：追加保持点击顺序；取消移除该项。 */
  const toggleRuleSetTag = (tag: string) => {
    setRuleSetTags((tags) => (tags.includes(tag) ? tags.filter((item) => item !== tag) : [...tags, tag]));
  };

  const targetError =
    matchType === "clash_mode"
      ? target.trim() === ""
        ? "请选择出站模式"
        : DNS_CLASH_MODE_OPTIONS.some((option) => option.value === target.trim())
          ? null
          : "出站模式无效"
      : matchType === "rule_set"
        ? ruleSetTags.length === 0
          ? "请至少选择一个规则集"
          : null
        : matchType === "query_type"
          ? target.trim() === ""
            ? "请输入查询类型"
            : (() => {
                const invalid = firstInvalidQueryType(target);
                return invalid === null ? null : invalid === "" ? "查询类型项不能为空" : `查询类型「${invalid}」无效`;
              })()
          : target.trim() === ""
            ? "请输入匹配目标"
            : null;
  const serverError =
    action !== "route"
      ? null
      : serverTag.trim() === ""
        ? "请选择目标 DNS 服务器"
        : serverOptions.some((option) => option.value === serverTag)
          ? null
          : "目标 DNS 服务器不存在";
  const rcodeError = action === "predefined" && !isValidRcode(rcode) ? "应答码无效" : null;
  const canSave = targetError === null && serverError === null && rcodeError === null;

  const handleSave = () => {
    if (!canSave) return;
    onSave({
      id: editing?.id ?? crypto.randomUUID(),
      enabled,
      match_type: matchType,
      // rule_set 多 tag 以英文逗号连接（渲染为数组，与路由规则同语义）。
      target: matchType === "rule_set" ? ruleSetTags.join(",") : target.trim(),
      server_tag: action === "route" ? serverTag.trim() : "",
      action,
      rcode: action === "predefined" ? (rcode.trim() !== "" ? rcode.trim().toUpperCase() : DEFAULT_DNS_RCODE) : "",
    });
    onClose();
  };

  return (
    <>
      {/* 匹配类型 */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-rule-match-type">匹配类型</Label>
        <Select
          id="dns-rule-match-type"
          aria-label="匹配类型"
          value={matchType}
          onChange={(key) => setMatchType(String(key) as DnsMatchType)}
          fullWidth
        >
          <Select.Trigger>
            <Select.Value />
            <Select.Indicator />
          </Select.Trigger>
          <Select.Popover>
            <ListBox>
              {DNS_MATCH_TYPE_OPTIONS.map((option) => (
                <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                  {option.label}
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
          </Select.Popover>
        </Select>
      </div>

      {/* 匹配目标（rule_set / clash_mode 走选择器，其余类型文本框输入） */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-rule-target">
          {matchType === "rule_set" ? "规则集" : matchType === "clash_mode" ? "出站模式" : "匹配目标"}
        </Label>
        {matchType === "clash_mode" ? (
          <Select
            id="dns-rule-target"
            aria-label="出站模式"
            value={target}
            onChange={(key) => setTarget(String(key ?? ""))}
            placeholder="请选择出站模式"
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {DNS_CLASH_MODE_OPTIONS.map((option) => (
                  <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                    {option.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
        ) : matchType === "rule_set" ? (
          <div className="flex flex-col gap-1.5">
            {effectiveRuleSetOptions.map((option) => (
              <Checkbox
                key={option.value}
                isSelected={ruleSetTags.includes(option.value)}
                onChange={() => toggleRuleSetTag(option.value)}
              >
                <Checkbox.Control>
                  <Checkbox.Indicator />
                </Checkbox.Control>
                <Checkbox.Content>
                  <span className="text-sm">{option.label}</span>
                  {option.hint && <span className="block text-xs text-muted">{option.hint}</span>}
                </Checkbox.Content>
              </Checkbox>
            ))}
          </div>
        ) : (
          <Input
            id="dns-rule-target"
            aria-label="匹配目标"
            aria-required="true"
            value={target}
            onChange={(event) => setTarget(event.target.value)}
            placeholder={DNS_TARGET_PLACEHOLDER[matchType]}
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className="font-mono"
          />
        )}
        {matchType === "rule_set" && effectiveRuleSetOptions.length === 0 ? (
          <span className="text-xs text-muted">当前没有可用规则集，可先在「规则」页的规则集管理中添加</span>
        ) : targetError ? (
          <span className="text-xs text-amber-500">{targetError}</span>
        ) : matchType === "rule_set" ? (
          <span className="text-xs text-muted">可多选；规则集随引用它的规则一同注入</span>
        ) : matchType === "clash_mode" ? (
          <span className="text-xs text-muted">仅在对应出站模式下命中（需启用 Clash API）</span>
        ) : matchType === "query_type" ? (
          <span className="text-xs text-muted">多个查询类型用英文逗号分隔，如 A,AAAA</span>
        ) : (
          <span className="text-xs text-muted">按所选匹配类型填写目标值</span>
        )}
      </div>

      {/* 规则动作 */}
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="dns-rule-action">动作</Label>
        <Select
          id="dns-rule-action"
          aria-label="动作"
          value={action}
          onChange={(key) => setAction(String(key) as DnsRuleAction)}
          fullWidth
        >
          <Select.Trigger>
            <Select.Value />
            <Select.Indicator />
          </Select.Trigger>
          <Select.Popover>
            <ListBox>
              {DNS_RULE_ACTION_OPTIONS.map((option) => (
                <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                  {option.label}
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
          </Select.Popover>
        </Select>
      </div>

      {/* route：目标服务器 */}
      {action === "route" && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-rule-server">目标 DNS 服务器</Label>
          <Select
            id="dns-rule-server"
            aria-label="目标 DNS 服务器"
            value={serverTag}
            onChange={(key) => setServerTag(String(key ?? ""))}
            placeholder={serverOptions.length === 0 ? "请先添加 DNS 服务器" : "请选择"}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {effectiveServerOptions.map((option) => (
                  <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                    {option.label}
                    {option.description ? <span className="text-xs text-muted">（{option.description}）</span> : null}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
          {serverError ? (
            <span className="text-xs text-amber-500">{serverError}</span>
          ) : (
            <span className="text-xs text-muted">命中后使用该 DNS 服务器解析</span>
          )}
        </div>
      )}

      {/* predefined：应答码 */}
      {action === "predefined" && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-rule-rcode">应答码</Label>
          <Select
            id="dns-rule-rcode"
            aria-label="应答码"
            value={rcode}
            onChange={(key) => setRcode(String(key ?? DEFAULT_DNS_RCODE))}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {DNS_RCODE_OPTIONS.map((option) => (
                  <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                    {option.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
          {rcodeError ? (
            <span className="text-xs text-amber-500">{rcodeError}</span>
          ) : (
            <span className="text-xs text-muted">命中后直接返回该应答码，不再向上游查询</span>
          )}
        </div>
      )}

      {/* reject：无额外字段 */}
      {action === "reject" && <span className="text-xs text-muted">命中后直接拒绝该 DNS 查询</span>}

      {/* 启用开关 */}
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm">启用该规则</span>
        <Switch aria-label="启用该规则" isSelected={enabled} onChange={setEnabled}>
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
      </div>

      <div className="flex gap-2">
        {editing && (
          <Button variant="danger" onPress={() => onDeleteRequest(editing)}>
            <TrashIcon className="size-4" aria-hidden="true" />
            删除规则
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
