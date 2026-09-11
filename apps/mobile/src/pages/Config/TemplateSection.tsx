import { useState } from "react";
import { PencilIcon, PlusIcon, TrashIcon } from "@heroicons/react/24/outline";
import { AlertDialog, Button, Card, Chip, Switch } from "@heroui/react";
import type { CustomTemplateInput, CustomTemplateView, LocalRuleView } from "@pp/client-core";
import type { ReactNode } from "react";
import { TemplateFormSheet } from "./TemplateFormSheet";

interface TemplateSectionProps {
  /** 已应用模板 id 集合（`applied_templates`；自定义模板为 `custom:<id>`）。 */
  appliedIds: ReadonlySet<string>;
  /** 自定义场景模板列表。 */
  customTemplates: CustomTemplateView[];
  /** 当前规则列表（新建/编辑模板时的勾选来源；父层已只传已启用规则）。 */
  ruleOptions: LocalRuleView[];
  onApply: (templateId: string) => Promise<boolean>;
  onRevert: (templateId: string) => Promise<boolean>;
  /** 新建模板（custom_templates 段追加落盘）；返回是否成功——成功才收起 Sheet。 */
  onCreate: (template: CustomTemplateInput) => Promise<boolean>;
  /** 编辑保存（custom_templates 段整段替换）；返回是否成功——成功才收起 Sheet。 */
  onUpdate: (template: CustomTemplateInput) => Promise<boolean>;
  /** 删除自定义模板；返回是否成功。 */
  onDelete: (template: CustomTemplateView) => Promise<boolean>;
}

/** 自定义模板在 apply/revert 命令中使用的 template_id（`custom:<id>`）。 */
function customTemplateId(template: CustomTemplateView): string {
  return `custom:${template.id}`;
}

/** 操作图标按钮（触达区 44×44，与 SubscriptionRow 一致）。 */
function IconButton({
  label,
  danger,
  disabled,
  onPress,
  children,
}: {
  label: string;
  danger?: boolean;
  disabled: boolean;
  onPress: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={disabled ? undefined : onPress}
      className={`flex size-11 shrink-0 items-center justify-center rounded-lg transition-opacity active:opacity-70 disabled:cursor-not-allowed disabled:opacity-35 ${
        danger ? "text-danger" : "text-muted"
      }`}
    >
      {children}
    </button>
  );
}

/**
 * 规则页场景模板区（ADR-0003 M5.4 / J4 扩展）。
 *
 * 自「废弃内置场景模板」起只渲染**用户自定义模板卡**；区头「新建模板」按钮 → 底部
 * Sheet（`TemplateFormSheet`，勾选已启用规则写入 ID 引用）；应用/撤销串行化（单飞
 * busy），成功/失败 toast 由页面处理器负责；删除需 AlertDialog 确认；已应用的模板
 * 必须先撤销再删除。
 *
 * 卡片布局（自「模板卡重排」起，移动单列左对齐）：
 * - 左侧信息列：名称（标题）/ 描述（副标题，空则不渲染）/ 规则数 chip / 失效数 chip
 *   （`invalid_count > 0` 时 warning，引用规则被删除/禁用的失效引用会保留在模板上，
 *   应用时跳过，提示用户模板内存在不生效的引用）；
 * - 右侧操作列：应用 Switch（开 = 已应用）+ 编辑（PencilIcon）/ 删除（TrashIcon）图标
 *   按钮，触达区 ≥44px。
 *
 * 应用状态由 Switch 承载（不再用文字按钮 + 「已应用」chip）：开 → `onApply`，
 * 关 → `onRevert`，均沿用页面层单飞 busy 逻辑（busy 期间 Switch 禁用）。
 * 编辑入口打开同一个 `TemplateFormSheet`（`editing` 非空即编辑模式）。
 */
export function TemplateSection({
  appliedIds,
  customTemplates,
  ruleOptions,
  onApply,
  onRevert,
  onCreate,
  onUpdate,
  onDelete,
}: TemplateSectionProps) {
  const [busyId, setBusyId] = useState<string | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [formSeq, setFormSeq] = useState(0);
  const [editingTemplate, setEditingTemplate] = useState<CustomTemplateView | null>(null);
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
    setEditingTemplate(null);
    setFormSeq((seq) => seq + 1);
    setFormOpen(true);
  };

  const openEdit = (template: CustomTemplateView) => {
    setEditingTemplate(template);
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
        {/* 自定义模板卡（可编辑/删除） */}
        {customTemplates.map((template) => {
          const templateId = customTemplateId(template);
          const applied = appliedIds.has(templateId);
          const displayName = template.name.trim() || "未命名模板";
          const desc = template.desc.trim();
          return (
            <Card key={template.id}>
              <Card.Content className="flex flex-row items-start gap-3">
                {/* 左侧信息列（左对齐） */}
                <div className="flex min-w-0 flex-1 flex-col gap-1 py-0.5">
                  <span className="min-w-0 truncate text-sm font-medium text-foreground">{displayName}</span>
                  {desc !== "" && <span className="min-w-0 truncate text-xs text-muted">{desc}</span>}
                  <span className="flex min-w-0 flex-wrap items-center gap-1.5">
                    <Chip size="sm" variant="soft" color="default" className="shrink-0">
                      {template.rules.length} 条规则
                    </Chip>
                    {template.invalid_count > 0 && (
                      <Chip size="sm" variant="soft" color="warning" className="shrink-0">
                        {template.invalid_count} 条失效
                      </Chip>
                    )}
                  </span>
                </div>
                {/* 右侧操作列：应用 Switch + 编辑/删除 */}
                <div className="flex shrink-0 flex-col items-end gap-0.5">
                  <Switch
                    aria-label="应用模板"
                    isSelected={applied}
                    isDisabled={busyId !== null}
                    onChange={(next) => void run(templateId, next ? onApply : onRevert)}
                    className="shrink-0 px-1.5"
                  >
                    <Switch.Content>
                      <Switch.Control>
                        <Switch.Thumb />
                      </Switch.Control>
                    </Switch.Content>
                  </Switch>
                  <div className="flex items-center">
                    <IconButton
                      label={`编辑模板 ${displayName}`}
                      disabled={busyId !== null}
                      onPress={() => openEdit(template)}
                    >
                      <PencilIcon className="size-5" aria-hidden="true" />
                    </IconButton>
                    <IconButton
                      label={`删除模板 ${displayName}`}
                      danger
                      disabled={applied || busyId !== null}
                      onPress={() => setPendingDelete(template)}
                    >
                      <TrashIcon className="size-5" aria-hidden="true" />
                    </IconButton>
                  </div>
                </div>
              </Card.Content>
            </Card>
          );
        })}
      </div>

      <TemplateFormSheet
        key={formSeq}
        isOpen={formOpen}
        rules={ruleOptions}
        editing={editingTemplate}
        onClose={() => setFormOpen(false)}
        onSave={(template) => (editingTemplate ? onUpdate(template) : onCreate(template))}
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
