import { AlertDialog, Button, Input, Label, Modal } from "@heroui/react";
import type { ProfileView } from "../api";

interface CreateProfileModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  name: string;
  onNameChange: (name: string) => void;
  busy: boolean;
  onSubmit: () => void;
}

/** 新建覆写模板对话框：表单状态由父级持有（创建成功后父级重置）。 */
export function CreateProfileModal({
  open,
  onOpenChange,
  name,
  onNameChange,
  busy,
  onSubmit,
}: CreateProfileModalProps) {
  return (
    <Modal.Backdrop isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[480px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>新建覆写模板</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex flex-col gap-4">
            <div className="flex flex-col gap-1">
              <Label htmlFor="profile-name">名称</Label>
              <Input
                id="profile-name"
                aria-label="模板名称"
                value={name}
                onChange={(event) => onNameChange(event.target.value)}
                placeholder="例如：香港-去广告"
                fullWidth
              />
            </div>
          </Modal.Body>
          <Modal.Footer>
            <Button slot="close" variant="secondary" onPress={() => onOpenChange(false)}>
              取消
            </Button>
            <Button variant="primary" isPending={busy} isDisabled={name.trim().length === 0} onPress={onSubmit}>
              创建
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}

interface DeleteProfileDialogProps {
  /** 待删除模板（null 表示对话框关闭）。 */
  target: ProfileView | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

/** 删除模板确认对话框。 */
export function DeleteProfileDialog({ target, busy, onCancel, onConfirm }: DeleteProfileDialogProps) {
  return (
    <AlertDialog.Backdrop
      isOpen={target !== null}
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <AlertDialog.Container size="sm">
        <AlertDialog.Dialog>
          <AlertDialog.CloseTrigger />
          <AlertDialog.Header>
            <AlertDialog.Icon status="danger" />
            <AlertDialog.Heading>删除模板</AlertDialog.Heading>
          </AlertDialog.Header>
          <AlertDialog.Body>
            <p className="break-words">确定删除模板「{target?.name}」吗？该操作不可撤销。</p>
          </AlertDialog.Body>
          <AlertDialog.Footer>
            <Button slot="close" variant="tertiary" onPress={onCancel}>
              取消
            </Button>
            <Button slot="close" variant="danger" isPending={busy} onPress={onConfirm}>
              删除
            </Button>
          </AlertDialog.Footer>
        </AlertDialog.Dialog>
      </AlertDialog.Container>
    </AlertDialog.Backdrop>
  );
}
