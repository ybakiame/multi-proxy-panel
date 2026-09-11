import { AlertDialog, Button } from "@heroui/react";
import type { LocalRuleView } from "@pp/client-core";
import { ruleSummary } from "@pp/client-core";

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
    <AlertDialog.Backdrop
      isOpen={rule !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <AlertDialog.Container size="sm">
        <AlertDialog.Dialog>
          <AlertDialog.CloseTrigger />
          <AlertDialog.Header>
            <AlertDialog.Icon status="danger" />
            <AlertDialog.Heading>删除规则</AlertDialog.Heading>
          </AlertDialog.Header>
          <AlertDialog.Body>
            <p className="break-words">确定删除规则「{rule ? ruleSummary(rule) : ""}」吗？该操作不可撤销。</p>
          </AlertDialog.Body>
          <AlertDialog.Footer>
            <Button slot="close" variant="tertiary" onPress={onClose}>
              取消
            </Button>
            <Button slot="close" variant="danger" onPress={onConfirm}>
              删除
            </Button>
          </AlertDialog.Footer>
        </AlertDialog.Dialog>
      </AlertDialog.Container>
    </AlertDialog.Backdrop>
  );
}
