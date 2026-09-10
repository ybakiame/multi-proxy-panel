import { useState } from "react";
import { AlertDialog, Button, Card, Chip } from "@heroui/react";
import { PencilIcon, PlusIcon, TrashIcon } from "@heroicons/react/24/outline";
import type { CustomTemplateInput, CustomTemplateView, LocalOverrideView } from "@pp/client-core";
import {
  buildSaveInput,
  localOverrideApplyTemplate,
  localOverrideRevertTemplate,
  localOverrideSave,
  toErrorMessage,
  toastError,
  toastSuccess,
} from "@pp/client-core";
import { TemplateFormModal } from "./TemplateFormModal";

interface TemplateSectionProps {
  /** 规则页 canonical 视图（模板/应用状态来源）。 */
  overrideData: LocalOverrideView;
  /** 核心是否运行中：运行中变更不热更新，成功 toast 追加「重启代理后生效」。 */
  coreRunning: boolean;
  /** 落盘成功后通知父层失效重读。 */
  onChanged: () => void;
}

/** 自定义模板在 apply/revert 命令中使用的 template_id（`custom:<id>`）。 */
function customTemplateId(template: CustomTemplateView): string {
  return `custom:${template.id}`;
}

/** View → Input（落盘丢弃只读的 `invalid_count`，服务端下次读取按引用重算）。 */
function toInput(template: CustomTemplateView): CustomTemplateInput {
  return {
    id: template.id,
    name: template.name,
    desc: template.desc,
    rules: template.rules,
    created_at: template.created_at,
  };
}

/**
 * 规则页场景模板区（desktop 卡片网格形态，语义对齐 mobile `TemplateSection`）。
 *
 * 只渲染用户自定义模板：应用/撤销（单飞 busy）、编辑、删除（AlertDialog 确认，
 * 已应用的模板须先撤销再删除）。新建/编辑走 `TemplateFormModal`，勾选来源为
 * 当前**已启用**规则（父层过滤），保存的是规则 ID 引用。
 */
export function TemplateSection({ overrideData, coreRunning, onChanged }: TemplateSectionProps) {
  const templates = overrideData.custom_templates;
  const appliedIds = new Set(overrideData.applied_templates.map((t) => t.template_id));
  // 新建/编辑模板只从已启用规则中勾选（引用语义：禁用规则不参与注入）。
  const ruleOptions = overrideData.singbox.rules.filter((rule) => rule.enabled);

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<CustomTemplateView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CustomTemplateView | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** `custom_templates` 段整段替换落盘（其余段经 buildSaveInput 透传当前视图）。 */
  const persistTemplates = async (next: CustomTemplateInput[]): Promise<boolean> => {
    try {
      await localOverrideSave({ ...buildSaveInput(overrideData), custom_templates: next });
      onChanged();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      onChanged();
      return false;
    }
  };

  /** 应用/撤销串行化（单飞 busy）。 */
  const run = async (templateId: string, op: (id: string) => Promise<boolean>) => {
    if (busyId !== null) return;
    setBusyId(templateId);
    // op 内部已兜底 toast 且不抛出；此处 await 后释放 busy。
    await op(templateId);
    setBusyId(null);
  };

  const handleApply = async (templateId: string): Promise<boolean> => {
    try {
      await localOverrideApplyTemplate(templateId);
      toastRuleSaved("模板已应用");
      onChanged();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  const handleRevert = async (templateId: string): Promise<boolean> => {
    try {
      await localOverrideRevertTemplate(templateId);
      toastRuleSaved("模板已撤销");
      onChanged();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  const openCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };

  const openEdit = (template: CustomTemplateView) => {
    setEditing(template);
    setFormOpen(true);
  };

  const handleCreate = async (template: CustomTemplateInput): Promise<boolean> => {
    const ok = await persistTemplates([...templates.map(toInput), template]);
    if (ok) {
      toastRuleSaved("场景模板已保存");
    }
    return ok;
  };

  const handleUpdate = async (template: CustomTemplateInput): Promise<boolean> => {
    const ok = await persistTemplates(templates.map((t) => (t.id === template.id ? template : toInput(t))));
    if (ok) {
      toastRuleSaved("场景模板已更新");
    }
    return ok;
  };

  const handleDelete = async (template: CustomTemplateView): Promise<boolean> => {
    const ok = await persistTemplates(templates.filter((t) => t.id !== template.id).map(toInput));
    if (ok) {
      toastRuleSaved(`已删除模板「${template.name.trim() || template.id}」`);
    }
    return ok;
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target) return;
    setPendingDelete(null);
    await handleDelete(target);
  };

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">场景模板</span>
          <span className="text-xs text-muted">将常用规则组合一键保存为可复用模板（规则 ID 引用）</span>
        </div>
        <Button size="sm" variant="primary" onPress={openCreate}>
          <PlusIcon className="size-4" />
          新建模板
        </Button>
      </div>

      {templates.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border/60 bg-surface p-6 text-center">
          <span className="text-sm text-muted">暂无场景模板，创建后可从已启用规则一键套用</span>
          <Button size="sm" variant="secondary" onPress={openCreate}>
            <PlusIcon className="size-4" />
            新建模板
          </Button>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {templates.map((template) => {
            const templateId = customTemplateId(template);
            const applied = appliedIds.has(templateId);
            const displayName = template.name.trim() || "未命名模板";
            const desc = template.desc.trim();
            return (
              <Card key={template.id} className="flex flex-col">
                <Card.Header>
                  <Card.Title>{displayName}</Card.Title>
                  <Card.Description>{desc || "—"}</Card.Description>
                </Card.Header>
                <Card.Content className="flex flex-1 flex-wrap items-center gap-1.5">
                  <Chip size="sm" variant="soft" color="default">
                    {template.rules.length} 条规则
                  </Chip>
                  {template.invalid_count > 0 && (
                    <Chip size="sm" variant="soft" color="warning">
                      {template.invalid_count} 条失效
                    </Chip>
                  )}
                </Card.Content>
                <Card.Footer className="flex items-center justify-between gap-2">
                  {applied ? (
                    <Button
                      size="sm"
                      variant="secondary"
                      isDisabled={busyId !== null}
                      onPress={() => void run(templateId, handleRevert)}
                    >
                      撤销
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="primary"
                      isDisabled={busyId !== null}
                      onPress={() => void run(templateId, handleApply)}
                    >
                      应用
                    </Button>
                  )}
                  <div className="flex items-center gap-1">
                    <Button
                      size="sm"
                      variant="ghost"
                      isIconOnly
                      aria-label={`编辑模板 ${displayName}`}
                      isDisabled={busyId !== null}
                      onPress={() => openEdit(template)}
                    >
                      <PencilIcon className="size-4" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      isIconOnly
                      aria-label={`删除模板 ${displayName}`}
                      isDisabled={applied || busyId !== null}
                      onPress={() => setPendingDelete(template)}
                    >
                      <TrashIcon className="size-4 text-danger" />
                    </Button>
                  </div>
                </Card.Footer>
              </Card>
            );
          })}
        </div>
      )}

      <TemplateFormModal
        isOpen={formOpen}
        rules={ruleOptions}
        editing={editing}
        onClose={() => setFormOpen(false)}
        onSave={(template) => (editing ? handleUpdate(template) : handleCreate(template))}
      />

      <AlertDialog.Backdrop
        isOpen={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
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
                确定删除自定义模板「{pendingDelete ? pendingDelete.name.trim() || pendingDelete.id : ""}
                」吗？删除后无法再从该模板一键套用规则。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={() => void handleDeleteConfirm()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </section>
  );
}
