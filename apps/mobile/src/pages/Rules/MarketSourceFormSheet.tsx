import { useState } from "react";
import { Button, Modal } from "@heroui/react";

interface MarketSourceFormSheetProps {
  isOpen: boolean;
  onClose: () => void;
  /**
   * 保存市场源（名称 + URL）。返回是否成功——成功才收起 Sheet，失败保留现场
   * （如 URL 重复 / 拉取解析失败由后端返回错误）。
   */
  onSave: (name: string, url: string) => Promise<boolean>;
}

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/**
 * 市场源添加底部 Sheet（名称 + 目录 JSON URL）。
 *
 * 保存时后端会先拉取 + 解析验证目录，失败不保存并返回错误。
 */
export function MarketSourceFormSheet({ isOpen, onClose, onSave }: MarketSourceFormSheetProps) {
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [saving, setSaving] = useState(false);
  const [wasOpen, setWasOpen] = useState(false);

  // open 切换时重置表单（adjust-state-during-render）。
  if (isOpen && !wasOpen) {
    setWasOpen(true);
    setName("");
    setUrl("");
    setSaving(false);
  } else if (!isOpen && wasOpen) {
    setWasOpen(false);
  }

  const canSave = name.trim().length > 0 && url.trim().length > 0;

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const ok = await onSave(name.trim(), url.trim());
    setSaving(false);
    if (ok) onClose();
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
            <Modal.Heading>添加市场源</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            <div className="flex flex-col gap-1.5">
              <label htmlFor="market-source-name" className="text-sm font-medium text-foreground">
                名称
              </label>
              <input
                id="market-source-name"
                aria-label="市场源名称"
                aria-required="true"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="例如：官方规则集市场"
                disabled={saving}
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="market-source-url" className="text-sm font-medium text-foreground">
                目录 URL
              </label>
              <input
                id="market-source-url"
                type="url"
                aria-label="市场目录 URL"
                aria-required="true"
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                placeholder="https://example.com/ruleset-market.json"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                inputMode="url"
                disabled={saving}
                className={`${inputClass} font-mono`}
              />
              <span className="text-xs text-muted">保存前会先拉取并解析该目录，失败不会保存</span>
            </div>
          </Modal.Body>
          <Modal.Footer>
            <Button variant="tertiary" className="min-h-12 flex-1" isDisabled={saving} onPress={onClose}>
              取消
            </Button>
            <Button
              variant="primary"
              className="min-h-12 flex-1"
              isDisabled={!canSave || saving}
              isPending={saving}
              onPress={() => void handleSave()}
            >
              添加
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
