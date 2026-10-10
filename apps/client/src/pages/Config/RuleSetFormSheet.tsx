import { useState } from "react";
import { Radio, RadioGroup } from "@pp/ui";
import type { CustomRuleSetInput, CustomRuleSetSource, CustomRuleSetView } from "@pp/client-core";
import { BottomSheet, Button, inputClassName, textareaClassName } from "@pp/ui";

type SourceKind = "remote" | "manual";
/** 远程文件格式：binary = .srs，source = .json（对齐 CustomRuleSetSource.format）。 */
type RemoteFormat = "binary" | "source";

/**
 * URL 后缀自动识别远程规则集格式（不再手选）：
 * `.json`（大小写不敏感）结尾 → source；其余（含 `.srs` / 无后缀）→ binary。
 */
function detectRemoteFormat(url: string): RemoteFormat {
  return url.trim().toLowerCase().endsWith(".json") ? "source" : "binary";
}

interface RuleSetFormSheetProps {
  isOpen: boolean;
  /** `null` = 新建；非空 = 编辑该自定义规则集（表单预填）。 */
  editing: CustomRuleSetView | null;
  onClose: () => void;
  /** 保存（新建/编辑共用）；返回是否成功——成功才收起 Sheet，失败保留现场（如 tag 冲突由 Rust 校验返回）。 */
  onSave: (ruleSet: CustomRuleSetInput) => Promise<boolean>;
}

const inputClass = inputClassName;

/** 手动 JSON 占位骨架（sing-box rule_set source 格式）。 */
const MANUAL_PLACEHOLDER = `{\n  "version": 1,\n  "rules": [\n    { "domain_suffix": [".ads.example.com"] }\n  ]\n}`;

/** Radio 选项行（Base UI Radio + 整行 label）。 */
function RadioOption({ value, label, disabled }: { value: string; label: string; disabled: boolean }) {
  return (
    <Radio value={value} isDisabled={disabled} className="min-h-11 w-full py-1">
      <Radio.Control>
        <Radio.Indicator />
      </Radio.Control>
      <Radio.Content>{label}</Radio.Content>
    </Radio>
  );
}

/**
 * 自定义规则集添加/编辑底部 Sheet（ADR-0003 M5.4 规则集管理子页）。
 *
 * - 类型 Radio：远程 URL / 手动输入 JSON；
 * - 公共字段：名称（必填）、tag（必填，placeholder `my-ads`）；
 * - 远程：URL（必填）——**格式不再手选**，按 URL 后缀自动识别并显示只读识别
 *   结果（`.json` → source，其余 → binary）；编辑已有远程规则集时 URL 未改则
 *   保留已存 format，URL 一旦被修改即按新 URL 后缀重新识别；
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
  if (key !== prevKey) {
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
      // 规则集是纯资源（无 enabled）：是否注入由引用它的规则决定。
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
    <BottomSheet
      opened={isOpen}
      onClose={onClose}
      title={editing ? "编辑规则集" : "添加规则集"}
      footer={
        <div className="flex gap-2">
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
        </div>
      }
    >
      <div className="flex max-h-[62vh] flex-col gap-4 overflow-y-auto">
        {/* 来源类型 */}
        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">类型</span>
          <RadioGroup
            value={kind}
            onChange={(value) => setKind(value === "manual" ? "manual" : "remote")}
            isDisabled={saving}
          >
            <RadioOption value="remote" label="远程 URL" disabled={saving} />
            <RadioOption value="manual" label="手动输入 JSON" disabled={saving} />
          </RadioGroup>
        </div>

        {/* 公共字段 */}
        <div className="flex flex-col gap-1.5">
          <label htmlFor="rs-name" className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
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
          <label htmlFor="rs-tag" className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
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
          <span className="text-xs text-zinc-500 dark:text-zinc-400">
            规则卡「规则集」匹配目标按此 tag 引用；须唯一且非空
          </span>
        </div>

        {/* 远程：URL（格式按后缀自动识别） */}
        {!isManual && (
          <div className="flex flex-col gap-1.5">
            <label htmlFor="rs-url" className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
              URL
            </label>
            <input
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
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              inputMode="url"
              disabled={saving}
              className={`${inputClass} font-mono`}
            />
          </div>
        )}

        {/* 手动：JSON 内容 */}
        {isManual && (
          <div className="flex flex-col gap-1.5">
            <label htmlFor="rs-content" className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
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
              className={`${textareaClassName} resize-y font-mono leading-6`}
            />
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              sing-box rule_set source JSON（保存即写入本地文件）
            </span>
          </div>
        )}
      </div>
    </BottomSheet>
  );
}
