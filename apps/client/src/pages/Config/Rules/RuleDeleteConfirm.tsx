import type { LocalRuleView } from "@pp/client-core";
import { ruleSummary } from "@pp/client-core";
import { ConfirmDialog } from "@pp/ui";

interface RuleDeleteConfirmProps {
  /** 待删除规则；`null` = 关闭。 */
  rule: LocalRuleView | null;
  onClose: () => void;
  onConfirm: () => void;
}

/**
 * 删除规则确认框（ADR-0003 M5.4）。
 *
 * 由编辑 Sheet 的「删除规则」入口触发（先收起 Sheet 再弹确认，避免双层遮罩叠放）。
 */
export function RuleDeleteConfirm({ rule, onClose, onConfirm }: RuleDeleteConfirmProps) {
  return (
    <ConfirmDialog
      opened={rule !== null}
      title="删除规则"
      danger
      confirmText="删除"
      onConfirm={onConfirm}
      onClose={onClose}
    >
      <p className="break-words">确定删除规则「{rule ? ruleSummary(rule) : ""}」吗？该操作不可撤销。</p>
    </ConfirmDialog>
  );
}
