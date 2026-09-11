import { useState } from "react";
import { Button, Checkbox, Modal } from "@heroui/react";
import type { CustomTemplateInput, CustomTemplateView, LocalRuleView } from "@pp/client-core";
import { actionLabel, matchTypeLabel, ruleSummary } from "@pp/client-core";

interface TemplateFormSheetProps {
  isOpen: boolean;
  /** 当前 `singbox.rules`（勾选来源；父层已只传已启用规则）。 */
  rules: LocalRuleView[];
  /** 编辑目标；`null` = 新建。父层以 `key` 强制重挂载，初始值直接取自该目标。 */
  editing: CustomTemplateView | null;
  onClose: () => void;
  /** 保存（名称/描述 + 规则 ID 引用列表）；返回是否成功——成功才收起 Sheet，失败保留现场。 */
  onSave: (template: CustomTemplateInput) => Promise<boolean>;
}

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/** 当前 Unix 秒。抽到模块级：避免 oxlint react(purity) 将事件处理器内的 `Date.now` 误判为 render 期调用。 */
function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

/** 动作 badge 配色（与 RuleCard 一致：proxy ≈ 品牌主色 / direct = 成功 / reject = 危险）。 */
const ACTION_BADGE_CLASS: Record<string, string> = {
  proxy: "bg-accent/10 text-accent",
  direct: "bg-success/10 text-success",
  reject: "bg-danger/10 text-danger",
};

/**
 * 场景模板 新建/编辑 底部 Sheet（ADR-0003 M5.4 J4）。
 *
 * - 名称（必填）、描述（可选）；
 * - 从现有规则勾选：复选列表逐条展示 `ruleSummary` + 动作 badge；
 *   **只列已启用的规则**（父层已过滤，这里双保险——禁用规则不能进模板：引用
 *   语义下它本就不会注入启动配置）；
 * - 保存时把勾选规则的 **ID** 写入 `CustomTemplateInput.rules`（引用语义，不复制
 *   规则定义；应用模板 = 激活场景，不复制规则）。
 *
 * 编辑模式（`editing` 非空）：预填名称/描述，并按模板 `rules` ID 集合回显勾选。
 * **失效引用保留**：被引用但已禁用/删除的规则不在勾选列表（`rules` 只含已启用
 * 规则），其 ID 仍留在 `editing.rules` 中；保存时合并「不在勾选列表中的既有引用」
 * + 「本次勾选集合」，因此编辑保存不会丢失未列出的失效引用（失效标记语义）。
 *
 * 由父层以 `key` 强制重挂载，每次打开即按 `editing` 初始化表单。
 */
export function TemplateFormSheet({ isOpen, rules, editing, onClose, onSave }: TemplateFormSheetProps) {
  const [name, setName] = useState(() => editing?.name ?? "");
  const [desc, setDesc] = useState(() => editing?.desc ?? "");
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(() => new Set(editing?.rules ?? []));
  const [saving, setSaving] = useState(false);

  // 双保险：勾选列表只列已启用规则（禁用规则不能作为模板引用——引用语义下不注入）。
  const selectableRules = rules.filter((rule) => rule.enabled);
  const selectableIdSet = new Set(selectableRules.map((rule) => rule.id));

  // 编辑：不在勾选列表中的既有引用（规则被禁用/删除）原样保留，保存不丢失。
  const preservedRefs = editing ? editing.rules.filter((id) => !selectableIdSet.has(id)) : [];
  // 本次勾选、且不在既有引用中的新规则（追加到引用列表末尾）。
  const addedRefs = selectableRules
    .filter((rule) => selectedIds.has(rule.id) && !(editing?.rules.includes(rule.id) ?? false))
    .map((rule) => rule.id);
  // 合并结果 = 既有引用（失效引用全留 + 仍选中的可见规则）+ 新增引用。
  const mergedRules = editing
    ? [...editing.rules.filter((id) => !selectableIdSet.has(id) || selectedIds.has(id)), ...addedRefs]
    : selectableRules.filter((rule) => selectedIds.has(rule.id)).map((rule) => rule.id);

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

  const canSave = name.trim().length > 0 && mergedRules.length > 0 && !saving;

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const template: CustomTemplateInput = {
      id: editing?.id ?? crypto.randomUUID(),
      name: name.trim(),
      desc: desc.trim(),
      // 规则 ID 引用列表（不复制规则定义；应用时以引用计算激活集合）。
      rules: mergedRules,
      created_at: editing?.created_at ?? nowSeconds(),
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
            <Modal.Heading>{editing ? "编辑场景模板" : "新建场景模板"}</Modal.Heading>
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
                <span className="text-xs text-muted">已选 {mergedRules.length} 条</span>
              </div>
              <span className="text-xs text-muted">
                勾选已启用的规则加入模板；只有被已应用模板引用的启用规则会注入启动配置
              </span>
              {preservedRefs.length > 0 && (
                <span className="text-xs text-warning">
                  另有 {preservedRefs.length} 条失效引用（规则已禁用或删除）不在列表中，保存时将保留
                </span>
              )}
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
