import { useState } from "react";
import { Button, Card, Chip } from "@heroui/react";
import { TEMPLATE_DEFS } from "@pp/client-core";

interface TemplateSectionProps {
  /** 已应用模板 id 集合（`applied_templates`）。 */
  appliedIds: ReadonlySet<string>;
  onApply: (templateId: string) => Promise<boolean>;
  onRevert: (templateId: string) => Promise<boolean>;
}

/**
 * 规则页 2 区：场景模板卡（ADR-0003 M5.4）。
 *
 * - 3 张模板卡来自共享 `TEMPLATE_DEFS`（回国/海外/广告过滤），卡片展示名称 + 描述；
 * - 未应用 → 「应用」按钮；已应用 → 「已应用」chip + 「撤销」按钮；
 * - 应用/撤销串行化：单飞 busy（同一时刻仅一张卡可操作），成功/失败 toast 由
 *   页面处理器负责，本组件只控制等待态与按钮禁用。
 */
export function TemplateSection({ appliedIds, onApply, onRevert }: TemplateSectionProps) {
  const [busyId, setBusyId] = useState<string | null>(null);

  const run = async (templateId: string, op: (id: string) => Promise<boolean>) => {
    if (busyId !== null) return;
    setBusyId(templateId);
    try {
      await op(templateId);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <span className="text-sm font-medium text-foreground">场景模板</span>
      <div className="flex flex-col gap-2">
        {TEMPLATE_DEFS.map((template) => {
          const applied = appliedIds.has(template.id);
          return (
            <Card key={template.id}>
              <Card.Content className="flex items-center gap-3">
                <div className="flex min-w-0 flex-1 flex-col gap-0.5 py-0.5">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="min-w-0 truncate text-sm font-medium text-foreground">{template.name}</span>
                    {applied && (
                      <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                        已应用
                      </Chip>
                    )}
                  </span>
                  <span className="text-xs text-muted">{template.desc}</span>
                </div>
                {applied ? (
                  <Button
                    variant="secondary"
                    className="min-h-11 shrink-0 px-4"
                    isDisabled={busyId !== null}
                    isPending={busyId === template.id}
                    onPress={() => void run(template.id, onRevert)}
                  >
                    撤销
                  </Button>
                ) : (
                  <Button
                    variant="primary"
                    className="min-h-11 shrink-0 px-4"
                    isDisabled={busyId !== null}
                    isPending={busyId === template.id}
                    onPress={() => void run(template.id, onApply)}
                  >
                    应用
                  </Button>
                )}
              </Card.Content>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
