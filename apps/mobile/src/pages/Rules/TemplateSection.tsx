import { useState } from "react";
import { PlusIcon, TrashIcon } from "@heroicons/react/24/outline";
import { AlertDialog, Button, Card, Chip } from "@heroui/react";
import type { CustomTemplateInput, CustomTemplateView, LocalRuleView } from "@pp/client-core";
import { TemplateFormSheet } from "./TemplateFormSheet";

interface TemplateSectionProps {
  /** 已应用模板 id 集合（`applied_templates`；自定义模板为 `custom:<id>`）。 */
  appliedIds: ReadonlySet<string>;
  /** 自定义场景模板列表。 */
  customTemplates: CustomTemplateView[];
  /** 当前规则列表（新建模板时的勾选来源）。 */
  ruleOptions: LocalRuleView[];
  onApply: (templateId: string) => Promise<boolean>;
  onRevert: (templateId: string) => Promise<boolean>;
  /** 新建模板（custom_templates 段追加落盘）；返回是否成功——成功才收起 Sheet。 */
  onCreate: (template: CustomTemplateInput) => Promise<boolean>;
  /** 删除自定义模板；返回是否成功。 */
  onDelete: (template: CustomTemplateView) => Promise<boolean>;
}

/** 自定义模板在 apply/revert 命令中使用的 template_id（`custom:<id>`）。 */
function customTemplateId(template: CustomTemplateView): string {
  return `custom:${template.id}`;
}

/**
 * 规则页场景模板区（ADR-0003 M5.4 / J4 扩展）。
 *
 * 自「废弃内置场景模板」起只渲染**用户自定义模板卡**（名称/描述/规则数 + 应用或
 * 撤销按钮 + 删除入口）；区头「新建模板」按钮 → 底部 Sheet（`TemplateFormSheet`，
 * 规则勾选快照）；应用/撤销串行化（单飞 busy），成功/失败 toast 由页面处理器负责；
 * 删除需 AlertDialog 确认；已应用的模板必须先撤销再删除（避免丢失撤销入口）。
 */
export function TemplateSection({
  appliedIds,
  customTemplates,
  ruleOptions,
  onApply,
  onRevert,
  onCreate,
  onDelete,
}: TemplateSectionProps) {
  const [busyId, setBusyId] = useState<string | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [formSeq, setFormSeq] = useState(0);
  const [pendingDelete, setPendingDelete] = useState<CustomTemplateView | null>(null);
  const [deleting, setDeleting] = useState(false);

  const run = async (templateId: string, op: (id: string) => Promise<boolean>) => {
    if (busyId !== null) return;
    setBusyId(templateId);
    try {
      await op(templateId);
    } finally {
      setBusyId(null);
    }
  };

  const openCreate = () => {
    setFormSeq((seq) => seq + 1);
    setFormOpen(true);
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target || deleting) return;
    setDeleting(true);
    try {
      setPendingDelete(null);
      await onDelete(target);
    } finally {
      setDeleting(false);
    }
  };

  return (
    <section className="flex flex-col gap-3">
      {/* 区头：标题 + 新建入口 */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">场景模板</span>
          <span className="text-xs text-muted">将常用规则组合一键保存为可复用模板</span>
        </div>
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={openCreate}>
          <PlusIcon className="size-4" aria-hidden="true" />
          新建模板
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        {/* 自定义模板卡（可删除） */}
        {customTemplates.map((template) => {
          const templateId = customTemplateId(template);
          const applied = appliedIds.has(templateId);
          const displayName = template.name.trim() || "未命名模板";
          return (
            <Card key={template.id}>
              <Card.Content className="flex items-center gap-3">
                <div className="flex min-w-0 flex-1 flex-col gap-0.5 py-0.5">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="min-w-0 truncate text-sm font-medium text-foreground">{displayName}</span>
                    {applied && (
                      <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                        已应用
                      </Chip>
                    )}
                    <Chip size="sm" variant="soft" color="default" className="shrink-0">
                      {template.rules.length} 条规则
                    </Chip>
                  </span>
                  <span className="truncate text-xs text-muted">{template.desc.trim() || "自定义规则组合快照"}</span>
                </div>
                {applied ? (
                  <Button
                    variant="secondary"
                    className="min-h-11 shrink-0 px-4"
                    isDisabled={busyId !== null}
                    isPending={busyId === templateId}
                    onPress={() => void run(templateId, onRevert)}
                  >
                    撤销
                  </Button>
                ) : (
                  <Button
                    variant="primary"
                    className="min-h-11 shrink-0 px-4"
                    isDisabled={busyId !== null}
                    isPending={busyId === templateId}
                    onPress={() => void run(templateId, onApply)}
                  >
                    应用
                  </Button>
                )}
                <button
                  type="button"
                  aria-label={`删除模板 ${displayName}`}
                  aria-disabled={applied || busyId !== null}
                  disabled={applied || busyId !== null}
                  onClick={() => setPendingDelete(template)}
                  className="flex size-11 shrink-0 items-center justify-center rounded-lg text-muted transition-opacity active:opacity-70 disabled:cursor-not-allowed disabled:opacity-35"
                >
                  <TrashIcon className="size-5" aria-hidden="true" />
                </button>
              </Card.Content>
            </Card>
          );
        })}
      </div>

      <TemplateFormSheet
        key={formSeq}
        isOpen={formOpen}
        rules={ruleOptions}
        onClose={() => setFormOpen(false)}
        onSave={onCreate}
      />

      {/* 删除确认 */}
      <AlertDialog.Backdrop
        isOpen={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open && !deleting) setPendingDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除场景模板</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除自定义模板「{pendingDelete ? pendingDelete.name.trim() || pendingDelete.id : ""}」吗？
                删除后无法再从该模板一键套用规则。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" isDisabled={deleting} onPress={() => setPendingDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" isPending={deleting} onPress={() => void handleDeleteConfirm()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </section>
  );
}
