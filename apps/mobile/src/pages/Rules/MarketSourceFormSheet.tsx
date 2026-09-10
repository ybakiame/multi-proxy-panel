import { useState } from "react";
import { Button, Chip, Modal } from "@heroui/react";
import { deriveMarketSourceName, detectMarketSource } from "@pp/client-core";

interface MarketSourceFormSheetProps {
  isOpen: boolean;
  onClose: () => void;
  /**
   * 保存市场源（名称 + 输入内容 + GitHub tag）。返回是否成功——成功才收起
   * Sheet，失败保留现场（如源重复 / 拉取解析失败由后端返回错误）。
   */
  onSave: (name: string, url: string, tag?: string) => Promise<boolean>;
}

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/**
 * 市场源添加底部 Sheet。
 *
 * 单输入框支持 `owner/repo` 简写、完整 GitHub URL 或 JSON 目录 URL；输入时实时
 * 显示自动识别结果（chip），GitHub 源额外提供可选 tag（留空 = latest）。保存时
 * 后端会先拉取 + 解析验证，失败不保存并返回错误。
 */
export function MarketSourceFormSheet({ isOpen, onClose, onSave }: MarketSourceFormSheetProps) {
  const [source, setSource] = useState("");
  const [tag, setTag] = useState("");
  const [saving, setSaving] = useState(false);
  const [wasOpen, setWasOpen] = useState(false);

  // open 切换时重置表单（adjust-state-during-render）。
  if (isOpen && !wasOpen) {
    setWasOpen(true);
    setSource("");
    setTag("");
    setSaving(false);
  } else if (!isOpen && wasOpen) {
    setWasOpen(false);
  }

  const detected = detectMarketSource(source, tag);
  const isGithub = detected.kind === "github";
  const canSave = source.trim().length > 0;

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const trimmed = source.trim();
    const name = deriveMarketSourceName(trimmed, tag);
    const ok = await onSave(name, trimmed, isGithub ? tag.trim() : undefined);
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
              <label htmlFor="market-source-input" className="text-sm font-medium text-foreground">
                仓库 / 目录地址
              </label>
              <input
                id="market-source-input"
                aria-label="市场源地址"
                aria-required="true"
                value={source}
                onChange={(event) => setSource(event.target.value)}
                placeholder="owner/repo、GitHub URL 或 https://example.com/market.json"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                disabled={saving}
                className={`${inputClass} font-mono`}
              />
              {source.trim().length > 0 && (
                <div className="flex items-center gap-1.5">
                  <Chip size="sm" variant="soft" color={isGithub ? "accent" : "default"}>
                    {isGithub ? "GitHub 仓库" : "JSON 目录"}
                  </Chip>
                  {isGithub && detected.ownerRepo && (
                    <span className="min-w-0 truncate font-mono text-xs text-muted">{detected.ownerRepo}</span>
                  )}
                </div>
              )}
              <span className="text-xs text-muted">保存前会先拉取并解析该源，失败不会保存</span>
            </div>

            {isGithub && (
              <div className="flex flex-col gap-1.5">
                <label htmlFor="market-source-tag" className="text-sm font-medium text-foreground">
                  Release tag（可选）
                </label>
                <input
                  id="market-source-tag"
                  aria-label="Release tag"
                  value={tag}
                  onChange={(event) => setTag(event.target.value)}
                  placeholder="留空 = 最新 release（latest）"
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  disabled={saving}
                  className={`${inputClass} font-mono`}
                />
                <span className="text-xs text-muted">
                  也可在 GitHub URL 中使用 /releases/tag/&lt;tag&gt;，此处填写会覆盖
                </span>
              </div>
            )}
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
