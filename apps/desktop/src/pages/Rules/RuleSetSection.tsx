import { useState } from "react";
import { AlertDialog, Button, Chip, Table } from "@heroui/react";
import { ArrowPathIcon, PlusIcon } from "@heroicons/react/24/outline";
import type { CustomRuleSetInput, CustomRuleSetView, LocalOverrideView } from "@pp/client-core";
import {
  buildSaveInput,
  localOverrideSave,
  localOverrideUpdateRulesetsNow,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
} from "@pp/client-core";
import { RuleSetFormModal } from "./RuleSetFormModal";

interface RuleSetSectionProps {
  /** 规则页 canonical 视图（规则集来源）。 */
  overrideData: LocalOverrideView;
  /** 核心是否运行中：运行中变更不热更新，成功 toast 追加「重启代理后生效」。 */
  coreRunning: boolean;
  /** 落盘成功后通知父层失效重读。 */
  onChanged: () => void;
}

/** 来源类型 label（对齐 CustomRuleSetSource）。 */
function sourceLabel(ruleSet: CustomRuleSetView): string {
  if (ruleSet.source.kind === "manual") return "手动 JSON";
  return ruleSet.source.format === "binary" ? "远程 srs" : "远程 json";
}

function formatUpdated(lastUpdated: number): string {
  if (lastUpdated <= 0) return "从未更新";
  return new Date(lastUpdated * 1000).toLocaleString();
}

/** View → Input（落盘丢弃只读的 `cached`）。 */
function toInput(ruleSet: CustomRuleSetView): CustomRuleSetInput {
  return {
    id: ruleSet.id,
    name: ruleSet.name,
    tag: ruleSet.tag,
    source: ruleSet.source,
    last_updated: ruleSet.last_updated,
  };
}

/**
 * 规则页规则集管理区（desktop 表格形态，语义对齐 mobile `RuleSetsPage`）。
 *
 * 单一表格 + 类型列区分**社区规则集**（远程 URL，`source.kind === "remote"`）与
 * **自定义规则集**（手动 JSON，`source.kind === "manual"`）——桌面场景下表格比双卡片
 * 分区更自然。规则集是纯资源（无 enabled）：是否注入由引用它的规则决定。
 *
 * 「立即更新」同步更新**全部** Remote（`localOverrideUpdateRulesetsNow`），成功 toast
 * 汇总成功/失败数（失败数 = Remote 数 - 成功数）。增删改均整段替换 `custom_rule_sets`
 * 落盘（Rust 链路自动清理文件）。
 */
export function RuleSetSection({ overrideData, coreRunning, onChanged }: RuleSetSectionProps) {
  const ruleSets = overrideData.custom_rule_sets;
  const remoteCount = ruleSets.filter((rs) => rs.source.kind === "remote").length;

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<CustomRuleSetView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CustomRuleSetView | null>(null);
  const [updating, setUpdating] = useState(false);

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** `custom_rule_sets` 段整段替换落盘（其余段经 buildSaveInput 透传当前视图）。 */
  const persistCustom = async (next: CustomRuleSetInput[]): Promise<boolean> => {
    try {
      await localOverrideSave({ ...buildSaveInput(overrideData), custom_rule_sets: next });
      onChanged();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      onChanged();
      return false;
    }
  };

  const handleUpdateNow = async () => {
    if (updating) return;
    if (remoteCount === 0) {
      toastWarning("暂无远程规则集可更新");
      return;
    }
    setUpdating(true);
    try {
      const updated = await localOverrideUpdateRulesetsNow();
      const failed = remoteCount - updated;
      const suffix = coreRunning ? "，重启代理后生效" : "";
      const summary =
        failed > 0
          ? `已更新 ${updated} 个远程规则集，${failed} 个失败${suffix}`
          : `已更新 ${updated} 个远程规则集${suffix}`;
      if (failed > 0) {
        toastWarning(summary);
      } else {
        toastSuccess(summary);
      }
      onChanged();
    } catch (err) {
      toastError(toErrorMessage(err));
      onChanged();
    }
    setUpdating(false);
  };

  const openCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };

  const openEdit = (ruleSet: CustomRuleSetView) => {
    setEditing(ruleSet);
    setFormOpen(true);
  };

  const handleSave = async (input: CustomRuleSetInput): Promise<boolean> => {
    const exists = ruleSets.some((rs) => rs.id === input.id);
    const next = exists
      ? ruleSets.map((rs) => (rs.id === input.id ? input : toInput(rs)))
      : [...ruleSets.map(toInput), input];
    const ok = await persistCustom(next);
    if (ok) {
      const base = exists ? "规则集已更新" : "规则集已添加";
      let text = coreRunning ? `${base}，重启代理后生效` : base;
      if (!exists && input.source.kind === "remote") {
        text += "；点击「立即更新」下载规则集";
      }
      toastSuccess(text);
    }
    return ok;
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target) return;
    setPendingDelete(null);
    const next = ruleSets.filter((rs) => rs.id !== target.id).map(toInput);
    if (await persistCustom(next)) {
      toastRuleSaved(`已删除规则集「${target.name.trim() || target.tag}」`);
    }
  };

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">规则集管理</span>
          <span className="text-xs text-muted">
            社区规则集（远程 URL）与自定义规则集（手动 JSON）；是否注入由引用它的规则决定
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button size="sm" variant="primary" onPress={openCreate}>
            <PlusIcon className="size-4" />
            添加规则集
          </Button>
          <Button
            size="sm"
            variant="secondary"
            isDisabled={updating}
            isPending={updating}
            onPress={() => void handleUpdateNow()}
          >
            <ArrowPathIcon className="size-4" />
            立即更新
          </Button>
        </div>
      </div>

      {ruleSets.length === 0 ? (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          暂无规则集，点击「添加规则集」创建；远程规则集添加后需「立即更新」下载
        </div>
      ) : (
        <Table>
          <Table.ScrollContainer>
            <Table.Content aria-label="规则集列表" className="min-w-[760px]">
              <Table.Header>
                <Table.Column isRowHeader>名称</Table.Column>
                <Table.Column>tag</Table.Column>
                <Table.Column>类型</Table.Column>
                <Table.Column>缓存状态</Table.Column>
                <Table.Column>更新时间</Table.Column>
                <Table.Column>操作</Table.Column>
              </Table.Header>
              <Table.Body>
                {ruleSets.map((ruleSet) => (
                  <Table.Row key={ruleSet.id}>
                    <Table.Cell className="max-w-[220px] truncate">
                      <span title={ruleSet.name}>{ruleSet.name.trim() || ruleSet.tag}</span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[200px] truncate font-mono text-xs">
                      <span title={ruleSet.tag}>{ruleSet.tag}</span>
                    </Table.Cell>
                    <Table.Cell>
                      <Chip size="sm" variant="soft" color="accent">
                        {sourceLabel(ruleSet)}
                      </Chip>
                    </Table.Cell>
                    <Table.Cell>
                      {ruleSet.cached ? (
                        <Chip size="sm" variant="soft" color="accent">
                          已缓存
                        </Chip>
                      ) : (
                        <Chip size="sm" variant="soft" color="default">
                          未缓存
                        </Chip>
                      )}
                    </Table.Cell>
                    <Table.Cell className="text-xs">{formatUpdated(ruleSet.last_updated)}</Table.Cell>
                    <Table.Cell>
                      <div className="flex items-center gap-2">
                        <Button size="sm" variant="secondary" onPress={() => openEdit(ruleSet)}>
                          编辑
                        </Button>
                        <Button size="sm" variant="tertiary" onPress={() => setPendingDelete(ruleSet)}>
                          删除
                        </Button>
                      </div>
                    </Table.Cell>
                  </Table.Row>
                ))}
              </Table.Body>
            </Table.Content>
          </Table.ScrollContainer>
        </Table>
      )}

      <RuleSetFormModal
        isOpen={formOpen}
        editing={editing}
        onClose={() => setFormOpen(false)}
        onSave={(ruleSet) => handleSave(ruleSet)}
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
              <AlertDialog.Heading>删除规则集</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除规则集「{pendingDelete ? pendingDelete.name.trim() || pendingDelete.tag : ""}
                」吗？将同时清理其缓存文件；引用该 tag 的规则需一并调整。
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
