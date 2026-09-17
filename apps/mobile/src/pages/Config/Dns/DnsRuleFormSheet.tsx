import { useMemo, useState } from "react";
import { CheckIcon, TrashIcon } from "@heroicons/react/24/outline";
import { Button, Modal, Switch } from "@heroui/react";
import type { DnsMatchType, DnsRule, DnsRuleAction } from "@pp/client-core";
import { parseRuleSetTags } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import type { RuleSetOption } from "../ruleSetOptions";
import {
  DEFAULT_DNS_RCODE,
  DNS_CLASH_MODE_OPTIONS,
  DNS_MATCH_TYPE_OPTIONS,
  DNS_RCODE_OPTIONS,
  DNS_RULE_ACTION_OPTIONS,
  DNS_TARGET_PLACEHOLDER,
  firstInvalidQueryType,
  isValidRcode,
} from "./dnsUtils";

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/** server_tag 下拉选项（页面从已定义 server tag 生成）。 */
export interface DnsServerOption {
  value: string;
  label: string;
  description?: string;
}

interface DnsRuleFormSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该规则（表单预填）。 */
  editing: DnsRule | null;
  /** 已定义 server tag 选项；空数组时提示先添加服务器。 */
  serverOptions: DnsServerOption[];
  /** 规则集选择器候选（仅 `rule_set` 匹配类型使用）；空数组时提示先在「规则集管理」中添加。 */
  ruleSetOptions?: RuleSetOption[];
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (rule: DnsRule) => void;
  /** 编辑模式点「删除规则」：父层收起 Sheet 并弹 AlertDialog 确认。 */
  onDeleteRequest: (rule: DnsRule) => void;
}

/**
 * DNS 分流规则编辑底部 Sheet（ADR-0005 P0-4b）。
 *
 * 字段：match_type / target / server_tag / enabled。`rule_set` 匹配为多选勾选列表
 * （逗号分隔 target 渲染为 sing-box 数组，对齐路由规则编辑）；校验：target 非空
 * （rule_set 至少一项）、server_tag 必须指向已定义 server，非法时禁用保存并给出行内
 * 错误。
 */
export function DnsRuleFormSheet({
  isOpen,
  editing,
  serverOptions,
  ruleSetOptions = [],
  onClose,
  onSave,
  onDeleteRequest,
}: DnsRuleFormSheetProps) {
  const [matchType, setMatchType] = useState<DnsMatchType>("domain");
  const [target, setTarget] = useState("");
  /** `rule_set` 匹配的已选 tag（多选，对齐路由规则编辑）；其余匹配类型走 `target`。 */
  const [ruleSetTags, setRuleSetTags] = useState<string[]>([]);
  const [serverTag, setServerTag] = useState("");
  const [action, setAction] = useState<DnsRuleAction>("route");
  const [rcode, setRcode] = useState(DEFAULT_DNS_RCODE);
  const [enabled, setEnabled] = useState(true);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建空表单 / 编辑预填）时同步表单初始值（adjust-state-during-render）。
  const key = isOpen ? (editing?.id ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setMatchType(editing?.match_type ?? "domain");
    setTarget(editing?.target ?? "");
    // 存量 rule_set target 为逗号分隔多 tag（或单值）：拆分回填勾选。
    setRuleSetTags(editing?.match_type === "rule_set" ? parseRuleSetTags(editing.target) : []);
    setServerTag(editing?.server_tag ?? "");
    setAction(editing?.action ?? "route");
    setRcode(editing?.rcode?.trim() !== "" && editing?.rcode ? editing.rcode.trim().toUpperCase() : DEFAULT_DNS_RCODE);
    setEnabled(editing?.enabled ?? true);
  }

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
   * 除/尚未添加），追加「原值保留」项——保持勾选态并标注，由用户决定取消或保留
   * （对齐路由 RuleEditSheet）。
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
            <Modal.Heading>{editing ? "编辑 DNS 规则" : "添加 DNS 规则"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* 匹配类型 */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">匹配类型</span>
              <MobileSelectSheet
                label="匹配类型"
                value={matchType}
                onChange={(value) => setMatchType(value as DnsMatchType)}
                options={DNS_MATCH_TYPE_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
            </div>

            {/* 匹配目标（rule_set / clash_mode 走选择器，其余类型文本框输入） */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">
                {matchType === "rule_set" ? "规则集" : matchType === "clash_mode" ? "出站模式" : "匹配目标"}
              </span>
              {matchType === "clash_mode" ? (
                <MobileSelectSheet
                  label="出站模式"
                  value={target}
                  onChange={setTarget}
                  placeholder="请选择出站模式"
                  options={DNS_CLASH_MODE_OPTIONS}
                />
              ) : matchType === "rule_set" ? (
                <div className="flex flex-col gap-2">
                  {effectiveRuleSetOptions.length === 0
                    ? null
                    : effectiveRuleSetOptions.map((option) => {
                        const selected = ruleSetTags.includes(option.value);
                        return (
                          <button
                            key={option.value}
                            type="button"
                            aria-pressed={selected}
                            onClick={() => toggleRuleSetTag(option.value)}
                            className={`flex min-h-12 w-full items-center justify-between gap-3 rounded-xl border px-4 py-2 text-left transition-colors ${
                              selected
                                ? "border-primary/50 bg-primary/5"
                                : "border-border/70 active:bg-surface-secondary/60"
                            }`}
                          >
                            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                              <span className="truncate text-sm font-medium text-foreground">{option.label}</span>
                              {option.hint && <span className="truncate text-xs text-muted">{option.hint}</span>}
                            </span>
                            {selected && <CheckIcon className="size-5 shrink-0 text-primary" aria-hidden="true" />}
                          </button>
                        );
                      })}
                </div>
              ) : (
                <input
                  id="dns-rule-target"
                  aria-label="匹配目标"
                  aria-required="true"
                  value={target}
                  onChange={(event) => setTarget(event.target.value)}
                  placeholder={DNS_TARGET_PLACEHOLDER[matchType]}
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  className={`${inputClass} font-mono`}
                />
              )}
              {matchType === "rule_set" && effectiveRuleSetOptions.length === 0 ? (
                <span className="text-xs text-muted">当前没有可用规则集，可先在 配置→规则集管理 添加</span>
              ) : targetError ? (
                <span className="text-xs text-warning">{targetError}</span>
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
              <span className="text-sm font-medium text-foreground">动作</span>
              <MobileSelectSheet
                label="动作"
                value={action}
                onChange={(value) => setAction(value as DnsRuleAction)}
                options={DNS_RULE_ACTION_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
            </div>

            {/* route：目标服务器 */}
            {action === "route" && (
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-foreground">目标 DNS 服务器</span>
                <MobileSelectSheet
                  label="目标 DNS 服务器"
                  value={serverTag}
                  onChange={setServerTag}
                  placeholder={serverOptions.length === 0 ? "请先添加 DNS 服务器" : "请选择"}
                  options={effectiveServerOptions}
                />
                {serverError ? (
                  <span className="text-xs text-warning">{serverError}</span>
                ) : (
                  <span className="text-xs text-muted">命中后使用该 DNS 服务器解析</span>
                )}
              </div>
            )}

            {/* predefined：应答码 */}
            {action === "predefined" && (
              <div className="flex flex-col gap-1.5">
                <span className="text-sm font-medium text-foreground">应答码</span>
                <MobileSelectSheet label="应答码" value={rcode} onChange={setRcode} options={DNS_RCODE_OPTIONS} />
                {rcodeError ? (
                  <span className="text-xs text-warning">{rcodeError}</span>
                ) : (
                  <span className="text-xs text-muted">命中后直接返回该应答码，不再向上游查询</span>
                )}
              </div>
            )}

            {/* reject：无额外字段 */}
            {action === "reject" && <span className="text-xs text-muted">命中后直接拒绝该 DNS 查询</span>}

            {/* 启用开关 */}
            <div className="flex min-h-11 items-center justify-between gap-3">
              <span className="text-sm text-foreground">启用该规则</span>
              <Switch aria-label="启用该规则" isSelected={enabled} onChange={setEnabled} className="shrink-0">
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
                删除规则
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
