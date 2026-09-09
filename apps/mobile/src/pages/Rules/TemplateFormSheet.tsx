import { useState } from "react";
import { Button, Checkbox, Modal } from "@heroui/react";
import type { CustomTemplateInput, LocalRuleView } from "@pp/client-core";
import { actionLabel, matchTypeLabel, ruleSummary } from "@pp/client-core";

interface TemplateFormSheetProps {
  isOpen: boolean;
  /** 当前 `singbox.rules`（新建模板时的勾选来源；父层已只传已启用规则）。 */
  rules: LocalRuleView[];
  onClose: () => void;
  /** 保存（名称/描述 + 所选规则 ID 引用列表）；返回是否成功——成功才收起 Sheet，失败保留现场。 */
  onSave: (template: CustomTemplateInput) => Promise<boolean>;
}

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/** 动作 badge 配色（与 RuleCard 一致：proxy ≈ 品牌主色 / direct = 成功 / reject = 危险）。 */
const ACTION_BADGE_CLASS: Record<string, string> = {
  proxy: "bg-accent/10 text-accent",
  direct: "bg-success/10 text-success",
  reject: "bg-danger/10 text-danger",
};

/**
 * 新建自定义场景模板底部 Sheet（ADR-0003 M5.4 J4）。
 *
 * - 名称（必填）、描述（可选）；
 * - 从现有规则勾选：复选列表逐条展示 `ruleSummary` + 动作 badge，至少勾选 1 条；
 *   **只列已启用的规则**（父层已过滤，这里双保险——禁用规则不能进模板：引用
 *   语义下它本就不会注入启动配置）；
 * - 保存时把勾选规则的 **ID** 写入 `CustomTemplateInput.rules`（引用语义，不复制
 *   规则定义；应用模板 = 激活场景，不复制规则）。
 *
 * 由父层以 `key` 强制重挂载，每次打开即空白表单（无编辑态）。
 */
export function TemplateFormSheet({ isOpen, rules, onClose, onSave }: TemplateFormSheetProps) {
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(() => new Set());
  const [saving, setSaving] = useState(false);

  // 双保险：勾选列表只列已启用规则（禁用规则不能作为模板引用——引用语义下不注入）。
  const selectableRules = rules.filter((rule) => rule.enabled);

  const toggle = (ruleId: string, selected: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (selected) {
        next.add(ruleId);
      } else {
        next.delete(ruleId);
      }
      return next;
    });
  };

  const canSave = name.trim().length > 0 && selectableRules.length > 0 && selectedIds.size > 0 && !saving;

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    // 引用语义：保存勾选规则的 ID；仅已启用规则可进模板（父层已过滤，这里双保险）。
    const selected = rules.filter((rule) => rule.enabled && selectedIds.has(rule.id));
    const template: CustomTemplateInput = {
      id: crypto.randomUUID(),
      name: name.trim(),
      desc: desc.trim(),
      // 规则 ID 引用列表（不复制规则定义；应用时以引用计算激活集合）。
      rules: selected.map((rule) => rule.id),
      created_at: Math.floor(Date.now() / 1000),
    };
    const ok = await onSave(template);
    setSaving(false);
    if (ok) {
      onClose();
    }
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
            <Modal.Heading>新建场景模板</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            <div className="flex flex-col gap-1.5">
              <label htmlFor="ctpl-name" className="text-sm font-medium text-foreground">
                名称
              </label>
              <input
                id="ctpl-name"
                aria-label="模板名称"
                aria-required="true"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="例如：科学上网常用"
                disabled={saving}
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="ctpl-desc" className="text-sm font-medium text-foreground">
                描述（可选）
              </label>
              <input
                id="ctpl-desc"
                aria-label="模板描述"
                value={desc}
                onChange={(event) => setDesc(event.target.value)}
                placeholder="一句话说明适用场景"
                disabled={saving}
                className={inputClass}
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-foreground">从现有规则勾选</span>
                <span className="text-xs text-muted">已选 {selectedIds.size} 条</span>
              </div>
              <span className="text-xs text-muted">
                勾选已启用的规则加入模板；只有被已应用模板引用的启用规则会注入启动配置
              </span>
              {selectableRules.length === 0 ? (
                <div className="rounded-lg border border-border/70 px-3 py-6 text-center text-sm text-muted">
                  暂无已启用的规则可选，请先在「自定义规则」中添加并启用
                </div>
              ) : (
                <div className="flex flex-col gap-1">
                  {selectableRules.map((rule) => {
                    const badgeClass = ACTION_BADGE_CLASS[rule.action] ?? "bg-default-soft text-muted";
                    return (
                      <Checkbox
                        key={rule.id}
                        isSelected={selectedIds.has(rule.id)}
                        isDisabled={saving}
                        onChange={(selected) => toggle(rule.id, selected)}
                        className="w-full"
                      >
                        <Checkbox.Content className="min-h-11 w-full">
                          <Checkbox.Control>
                            <Checkbox.Indicator />
                          </Checkbox.Control>
                          <span className="flex min-w-0 flex-1 flex-col gap-0.5 py-1">
                            <span className="flex min-w-0 items-center gap-2">
                              <span className="min-w-0 truncate text-sm font-medium text-foreground">
                                {ruleSummary(rule)}
                              </span>
                              <span
                                className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-medium leading-5 ${badgeClass}`}
                              >
                                {actionLabel(rule.action)}
                              </span>
                            </span>
                            <span className="truncate text-xs text-muted">
                              {matchTypeLabel(rule.match_type)}
                              {rule.target ? `: ${rule.target}` : ""}
                            </span>
                          </span>
                        </Checkbox.Content>
                      </Checkbox>
                    );
                  })}
                </div>
              )}
            </div>
          </Modal.Body>
          <Modal.Footer>
            <Button variant="tertiary" className="min-h-12 flex-1" isDisabled={saving} onPress={onClose}>
              取消
            </Button>
            <Button
              variant="primary"
              className="min-h-12 flex-1"
              isDisabled={!canSave}
              isPending={saving}
              onPress={() => void handleSave()}
            >
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
