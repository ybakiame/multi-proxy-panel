import { useState } from "react";
import { Button, Input, Label, Modal, Radio, RadioGroup } from "@heroui/react";
import type { CustomRuleSetInput, CustomRuleSetSource, CustomRuleSetView } from "@pp/client-core";

type SourceKind = "remote" | "manual";
/** 远程文件格式：binary = .srs，source = .json（对齐 CustomRuleSetSource.format）。 */
type RemoteFormat = "binary" | "source";

export interface RuleSetFormModalProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该规则集（表单预填）。 */
  editing: CustomRuleSetView | null;
  onClose: () => void;
  /** 保存（新建/编辑共用）；返回是否成功——成功才收起，失败保留现场（如 tag 冲突由 Rust 校验返回）。 */
  onSave: (ruleSet: CustomRuleSetInput) => Promise<boolean>;
}

/**
 * URL 后缀自动识别远程规则集格式（不再手选）：
 * `.json`（大小写不敏感）结尾 → source；其余（含 `.srs` / 无后缀）→ binary。
 */
function detectRemoteFormat(url: string): RemoteFormat {
  return url.trim().toLowerCase().endsWith(".json") ? "source" : "binary";
}

/** 当前 Unix 秒；抽到模块级避免 oxlint react(purity) 误判事件处理器内的 `Date.now`。 */
function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

/** 手动 JSON 占位骨架（sing-box rule_set source 格式）。 */
const MANUAL_PLACEHOLDER = `{\n  "version": 1,\n  "rules": [\n    { "domain_suffix": [".ads.example.com"] }\n  ]\n}`;

/** Radio 选项行。 */
function RadioOption({ value, label, disabled }: { value: string; label: string; disabled: boolean }) {
  return (
    <Radio value={value} isDisabled={disabled} className="w-full">
      <Radio.Content className="min-h-9 w-full py-1">
        <Radio.Control>
          <Radio.Indicator />
        </Radio.Control>
        <span className="text-sm font-medium text-foreground">{label}</span>
      </Radio.Content>
    </Radio>
  );
}

/**
 * 规则集添加/编辑 Modal（desktop 形态，语义对齐 mobile `RuleSetFormSheet`）。
 *
 * - 类型 Radio：远程 URL / 手动输入 JSON；
 * - 公共字段：名称（必填）、tag（必填）；
 * - 远程：URL（必填）——格式不再手选，按 URL 后缀自动识别并只读展示（`.json` → source，
 *   其余 → binary）；编辑已有远程规则集时 URL 未改则保留已存 format，URL 一旦被修改即按新
 *   URL 后缀重新识别；
 * - 手动：JSON 内容多行等宽编辑器（必填）；
 * - 保存构建 `CustomRuleSetInput`（custom 段整段替换落盘由父层执行）。
 */
function RuleSetForm({ editing, onClose, onSave }: Omit<RuleSetFormModalProps, "isOpen">) {
  const source = editing?.source;
  const [kind, setKind] = useState<SourceKind>(source?.kind === "manual" ? "manual" : "remote");
  const [remoteFormat, setRemoteFormat] = useState<RemoteFormat>(source?.kind === "remote" ? source.format : "binary");
  const [name, setName] = useState(editing?.name ?? "");
  const [tag, setTag] = useState(editing?.tag ?? "");
  const [url, setUrl] = useState(source?.kind === "remote" ? source.url : "");
  const [content, setContent] = useState(source?.kind === "manual" ? source.content : "");
  const [saving, setSaving] = useState(false);

  const isManual = kind === "manual";
  const canSave =
    name.trim().length > 0 && tag.trim().length > 0 && (isManual ? content.trim().length > 0 : url.trim().length > 0);

  const handleSave = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    const nextSource: CustomRuleSetSource = isManual
      ? { kind: "manual", content: content.trim() }
      : { kind: "remote", url: url.trim(), format: remoteFormat };
    const input: CustomRuleSetInput = {
      id: editing?.id ?? crypto.randomUUID(),
      name: name.trim(),
      tag: tag.trim(),
      source: nextSource,
      // 规则集是纯资源（无 enabled）：是否注入由引用它的规则决定。
      // 手动内容保存即写入文件；远程沿用最近一次成功下载时间（新条目为 0，待「立即更新」）。
      last_updated: isManual ? nowSeconds() : (editing?.last_updated ?? 0),
    };
    const ok = await onSave(input);
    setSaving(false);
    if (ok) {
      onClose();
    }
  };

  return (
    <>
      <Modal.Body className="flex max-h-[70vh] flex-col gap-4 overflow-y-auto">
        <div className="flex flex-col gap-1.5">
          <Label>类型</Label>
          <RadioGroup
            value={kind}
            onChange={(value) => setKind(String(value) === "manual" ? "manual" : "remote")}
            isDisabled={saving}
          >
            <RadioOption value="remote" label="远程 URL" disabled={saving} />
            <RadioOption value="manual" label="手动输入 JSON" disabled={saving} />
          </RadioGroup>
        </div>

        <div className="flex flex-col gap-1">
          <Label htmlFor="rs-name">名称</Label>
          <Input
            id="rs-name"
            aria-label="规则集名称"
            aria-required="true"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="例如：我的广告过滤"
            disabled={saving}
            fullWidth
          />
        </div>

        <div className="flex flex-col gap-1">
          <Label htmlFor="rs-tag">引用 tag</Label>
          <Input
            id="rs-tag"
            aria-label="规则集 tag"
            aria-required="true"
            value={tag}
            onChange={(event) => setTag(event.target.value)}
            placeholder="my-ads"
            disabled={saving}
            fullWidth
          />
          <span className="text-xs text-muted">规则卡「规则集」匹配目标按此 tag 引用；须唯一且非空</span>
        </div>

        {!isManual && (
          <div className="flex flex-col gap-1">
            <Label htmlFor="rs-url">URL</Label>
            <Input
              id="rs-url"
              type="url"
              aria-label="规则集 URL"
              aria-required="true"
              value={url}
              onChange={(event) => {
                const next = event.target.value;
                setUrl(next);
                // 格式不再手选：URL 每次变化都按后缀自动识别（`.json` → source，其余 → binary）。
                setRemoteFormat(detectRemoteFormat(next));
              }}
              placeholder="https://example.com/ads.srs"
              disabled={saving}
              fullWidth
            />
            {url.trim() !== "" && (
              <span className="inline-flex self-start items-center gap-1.5 rounded-full border border-border/60 bg-surface-secondary/60 px-3 py-1 text-xs text-muted">
                {detectRemoteFormat(url) === "source"
                  ? "URL 以 .json 结尾：将识别为 json 源码（source）"
                  : "URL 非 .json 结尾：将识别为 srs 二进制（binary）"}
              </span>
            )}
            <span className="text-xs text-muted">保存后点击「立即更新」下载；下载成功前不会被注入</span>
          </div>
        )}

        {isManual && (
          <div className="flex flex-col gap-1">
            <Label htmlFor="rs-content">JSON 内容</Label>
            <textarea
              id="rs-content"
              aria-label="规则集 JSON 内容"
              aria-required="true"
              value={content}
              onChange={(event) => setContent(event.target.value)}
              placeholder={MANUAL_PLACEHOLDER}
              spellCheck={false}
              disabled={saving}
              rows={10}
              className="w-full resize-y rounded-lg border border-border/70 bg-surface px-3 py-2 font-mono text-sm leading-6 text-foreground outline-none placeholder:text-muted focus:border-accent/60 disabled:opacity-60"
            />
            <span className="text-xs text-muted">sing-box rule_set source JSON（保存即写入本地文件）</span>
          </div>
        )}
      </Modal.Body>
      <Modal.Footer>
        <Button slot="close" variant="tertiary" isDisabled={saving} onPress={onClose}>
          取消
        </Button>
        <Button variant="primary" isDisabled={!canSave} isPending={saving} onPress={() => void handleSave()}>
          保存
        </Button>
      </Modal.Footer>
    </>
  );
}

export function RuleSetFormModal({ isOpen, editing, onClose, onSave }: RuleSetFormModalProps) {
  // key 确保切换编辑目标 / 新建时表单重新挂载、状态重置。
  const formKey = editing?.id ?? "__new__";

  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[520px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{editing ? "编辑规则集" : "添加规则集"}</Modal.Heading>
          </Modal.Header>
          <RuleSetForm key={formKey} editing={editing} onClose={onClose} onSave={onSave} />
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
