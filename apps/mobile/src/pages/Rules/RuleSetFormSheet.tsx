import { useState } from "react";
import { Button, Modal, Radio, RadioGroup } from "@heroui/react";
import type { CustomRuleSetInput, CustomRuleSetSource, CustomRuleSetView } from "@pp/client-core";

type SourceKind = "remote" | "manual";
/** 远程文件格式：binary = .srs，source = .json（对齐 CustomRuleSetSource.format）。 */
type RemoteFormat = "binary" | "source";

interface RuleSetFormSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该自定义规则集（表单预填）。 */
  editing: CustomRuleSetView | null;
  onClose: () => void;
  /** 保存（新建/编辑共用）；返回是否成功——成功才收起 Sheet，失败保留现场（如 tag 冲突由 Rust 校验返回）。 */
  onSave: (ruleSet: CustomRuleSetInput) => Promise<boolean>;
}

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/** 手动 JSON 占位骨架（sing-box rule_set source 格式）。 */
const MANUAL_PLACEHOLDER = `{\n  "version": 1,\n  "rules": [\n    { "domain_suffix": [".ads.example.com"] }\n  ]\n}`;

/** Radio 选项行（触达 ≥48px：Control 组行高 min-h-11）。 */
function RadioOption({ value, label, disabled }: { value: string; label: string; disabled: boolean }) {
  return (
    <Radio value={value} isDisabled={disabled} className="w-full">
      <Radio.Content className="min-h-11 w-full py-1">
        <Radio.Control>
          <Radio.Indicator />
        </Radio.Control>
        <span className="text-sm font-medium text-foreground">{label}</span>
      </Radio.Content>
    </Radio>
  );
}

/**
 * 自定义规则集添加/编辑底部 Sheet（ADR-0003 M5.4 规则集管理子页）。
 *
 * - 类型 Radio：远程 URL / 手动输入 JSON；
 * - 公共字段：名称（必填）、tag（必填，placeholder `my-ads`）；
 * - 远程：格式 Radio（srs 二进制 / json）+ URL（必填）；
 * - 手动：JSON 内容多行等宽编辑器（必填，placeholder 为 sing-box source 骨架）；
 * - 保存构建 `CustomRuleSetInput`（custom 段整段替换落盘由父层执行）。
 */
export function RuleSetFormSheet({ isOpen, editing, onClose, onSave }: RuleSetFormSheetProps) {
  const [kind, setKind] = useState<SourceKind>("remote");
  const [remoteFormat, setRemoteFormat] = useState<RemoteFormat>("binary");
  const [name, setName] = useState("");
  const [tag, setTag] = useState("");
  const [url, setUrl] = useState("");
  const [content, setContent] = useState("");
  const [saving, setSaving] = useState(false);
  const [prevKey, setPrevKey] = useState<string | null>(null);

  // open 切换（新建空表单 / 编辑预填）时同步表单初始值（adjust-state-during-render）。
  const key = isOpen ? (editing?.id ?? "__new__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    const source = editing?.source;
    setKind(source?.kind === "manual" ? "manual" : "remote");
    setRemoteFormat(source?.kind === "remote" ? source.format : "binary");
    setName(editing?.name ?? "");
    setTag(editing?.tag ?? "");
    setUrl(source?.kind === "remote" ? source.url : "");
    setContent(source?.kind === "manual" ? source.content : "");
    setSaving(false);
  }

  const isManual = kind === "manual";
  const canSave =
    name.trim().length > 0 && tag.trim().length > 0 && (isManual ? content.trim().length > 0 : url.trim().length > 0);

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const source: CustomRuleSetSource = isManual
      ? { kind: "manual", content: content.trim() }
      : { kind: "remote", url: url.trim(), format: remoteFormat };
    const now = Math.floor(Date.now() / 1000);
    const input: CustomRuleSetInput = {
      id: editing?.id ?? crypto.randomUUID(),
      name: name.trim(),
      tag: tag.trim(),
      source,
      enabled: editing?.enabled ?? true,
      // 手动内容保存即写入文件；远程沿用最近一次成功下载时间（新条目为 0，待「立即更新」）。
      last_updated: isManual ? now : (editing?.last_updated ?? 0),
    };
    const ok = await onSave(input);
    setSaving(false);
    if (ok) {
      onClose();
    }
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
            <Modal.Heading>{editing ? "编辑规则集" : "添加规则集"}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
            {/* 来源类型 */}
            <div className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">类型</span>
              <RadioGroup value={kind} onChange={(value) => setKind(String(value) === "manual" ? "manual" : "remote")}>
                <RadioOption value="remote" label="远程 URL" disabled={saving} />
                <RadioOption value="manual" label="手动输入 JSON" disabled={saving} />
              </RadioGroup>
            </div>

            {/* 公共字段 */}
            <div className="flex flex-col gap-1.5">
              <label htmlFor="rs-name" className="text-sm font-medium text-foreground">
                名称
              </label>
              <input
                id="rs-name"
                aria-label="规则集名称"
                aria-required="true"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="例如：我的广告过滤"
                disabled={saving}
                className={inputClass}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="rs-tag" className="text-sm font-medium text-foreground">
                引用 tag
              </label>
              <input
                id="rs-tag"
                aria-label="规则集 tag"
                aria-required="true"
                value={tag}
                onChange={(event) => setTag(event.target.value)}
                placeholder="my-ads"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                disabled={saving}
                className={`${inputClass} font-mono`}
              />
              <span className="text-xs text-muted">规则卡「规则集」匹配目标按此 tag 引用；须唯一且非空</span>
            </div>

            {/* 远程：格式 + URL */}
            {!isManual && (
              <>
                <div className="flex flex-col gap-1.5">
                  <span className="text-sm font-medium text-foreground">格式</span>
                  <RadioGroup
                    value={remoteFormat}
                    onChange={(value) => setRemoteFormat(String(value) === "source" ? "source" : "binary")}
                  >
                    <RadioOption value="binary" label="srs 二进制（推荐）" disabled={saving} />
                    <RadioOption value="source" label="json（source 文本）" disabled={saving} />
                  </RadioGroup>
                </div>
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="rs-url" className="text-sm font-medium text-foreground">
                    URL
                  </label>
                  <input
                    id="rs-url"
                    type="url"
                    aria-label="规则集 URL"
                    aria-required="true"
                    value={url}
                    onChange={(event) => setUrl(event.target.value)}
                    placeholder="https://example.com/ads.srs"
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    inputMode="url"
                    disabled={saving}
                    className={`${inputClass} font-mono`}
                  />
                  <span className="text-xs text-muted">保存后点击「立即更新」下载；下载成功前不会被注入</span>
                </div>
              </>
            )}

            {/* 手动：JSON 内容 */}
            {isManual && (
              <div className="flex flex-col gap-1.5">
                <label htmlFor="rs-content" className="text-sm font-medium text-foreground">
                  JSON 内容
                </label>
                <textarea
                  id="rs-content"
                  aria-label="规则集 JSON 内容"
                  aria-required="true"
                  value={content}
                  onChange={(event) => setContent(event.target.value)}
                  placeholder={MANUAL_PLACEHOLDER}
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  disabled={saving}
                  rows={8}
                  className="w-full resize-y rounded-lg border border-border/70 bg-surface px-3 py-2 font-mono text-sm leading-6 text-foreground outline-none placeholder:text-muted focus:border-accent/60 disabled:opacity-60"
                />
                <span className="text-xs text-muted">sing-box rule_set source JSON（保存即写入本地文件）</span>
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
              保存
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
