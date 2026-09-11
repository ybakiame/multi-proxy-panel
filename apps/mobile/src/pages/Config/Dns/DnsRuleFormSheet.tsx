import { useMemo, useState } from "react";
import { TrashIcon } from "@heroicons/react/24/outline";
import { Button, Modal, Switch } from "@heroui/react";
import type { DnsMatchType, DnsRule } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { DNS_MATCH_TYPE_OPTIONS, DNS_TARGET_PLACEHOLDER } from "./dnsUtils";

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
  onClose: () => void;
  /** 保存（仅更新内存草稿，落盘由页面统一执行）。 */
  onSave: (rule: DnsRule) => void;
  /** 编辑模式点「删除规则」：父层收起 Sheet 并弹 AlertDialog 确认。 */
  onDeleteRequest: (rule: DnsRule) => void;
}

/**
 * DNS 分流规则编辑底部 Sheet（ADR-0005 P0-4b）。
 *
 * 字段：match_type / target / server_tag / enabled。校验：target 非空、
 * server_tag 必须指向已定义 server，非法时禁用保存并给出行内错误。
 */
export function DnsRuleFormSheet({
  isOpen,
  editing,
  serverOptions,
  onClose,
  onSave,
  onDeleteRequest,
}: DnsRuleFormSheetProps) {
  const [matchType, setMatchType] = useState<DnsMatchType>("domain");
  const [target, setTarget] = useState("");
  const [serverTag, setServerTag] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建空表单 / 编辑预填）时同步表单初始值（adjust-state-during-render）。
  const key = isOpen ? (editing?.id ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setMatchType(editing?.match_type ?? "domain");
    setTarget(editing?.target ?? "");
    setServerTag(editing?.server_tag ?? "");
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

  const targetError = target.trim() === "" ? "请输入匹配目标" : null;
  const serverError =
    serverTag.trim() === ""
      ? "请选择目标 DNS 服务器"
      : serverOptions.some((option) => option.value === serverTag)
        ? null
        : "目标 DNS 服务器不存在";
  const canSave = targetError === null && serverError === null;

  const handleSave = () => {
    if (!canSave) return;
    onSave({
      id: editing?.id ?? crypto.randomUUID(),
      enabled,
      match_type: matchType,
      target: target.trim(),
      server_tag: serverTag.trim(),
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

            {/* 匹配目标 */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="dns-rule-target" className="text-sm font-medium text-foreground">
                匹配目标
              </label>
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
              {targetError ? (
                <span className="text-xs text-warning">{targetError}</span>
              ) : (
                <span className="text-xs text-muted">按所选匹配类型填写目标值</span>
              )}
            </div>

            {/* 目标服务器 */}
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
