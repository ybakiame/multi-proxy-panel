import { useState } from "react";
import type { SubscriptionView } from "@pp/client-core";
import { BottomSheet, Button, inputClassName } from "./ui";

export interface SubscriptionDraft {
  name: string;
  url: string;
  /** 拉取请求的 User-Agent（空 = 默认 `clash.meta`）。 */
  userAgent: string;
}

interface SubscriptionFormSheetProps {
  isOpen: boolean;
  /** `null` = 添加模式；非空 = 编辑该订阅（表单预填）。 */
  editing: SubscriptionView | null;
  /** 保存进行中（提交按钮 loading 且禁用表单）。 */
  busy: boolean;
  onClose: () => void;
  onSave: (draft: SubscriptionDraft) => void;
}

/** 表单字段标签行样式。 */
const labelClassName = "text-sm font-medium text-zinc-900 dark:text-zinc-100";

/**
 * 订阅添加/编辑底部 Sheet（ADR-0003 M5.6）。
 *
 * - 字段对齐 desktop AddSubscriptionModal：名称 + URL 必填（trim 后非空才可提交），
 *   User-Agent 可选（空 = 默认 `clash.meta`；部分订阅源按 UA 返回不同格式）；
 *   Profile 关联/覆写模板选择为 desktop 后续批次能力，本表单不提供（边界见任务说明）；
 * - Sheet 常驻挂载（isOpen 控制显隐）：open/编辑对象变化时在渲染期同步初始值
 *   （adjust-state-during-render，替代 effect 同步 setState，同 desktop EditSubscriptionModal）。
 */
export function SubscriptionFormSheet({ isOpen, editing, busy, onClose, onSave }: SubscriptionFormSheetProps) {
  const [draft, setDraft] = useState<SubscriptionDraft>({ name: "", url: "", userAgent: "" });
  const [prevKey, setPrevKey] = useState<string | null>(null);
  // open 切换（添加空表单 / 编辑预填）时同步表单初始值。
  const key = isOpen ? (editing?.id ?? "__add__") : null;
  if (key !== null && key !== prevKey) {
    setPrevKey(key);
    setDraft(
      editing
        ? { name: editing.name, url: editing.url, userAgent: editing.user_agent ?? "" }
        : { name: "", url: "", userAgent: "" },
    );
  }

  const formValid = draft.name.trim().length > 0 && draft.url.trim().length > 0;

  const handleSave = () => {
    if (!formValid || busy) return;
    onSave({ name: draft.name.trim(), url: draft.url.trim(), userAgent: draft.userAgent.trim() });
  };

  return (
    <BottomSheet
      opened={isOpen}
      onClose={onClose}
      title={editing ? "编辑订阅" : "添加订阅"}
      footer={
        <Button
          variant="primary"
          className="min-h-12 w-full"
          isDisabled={!formValid}
          isPending={busy}
          onPress={handleSave}
        >
          {editing ? "保存" : "添加"}
        </Button>
      }
    >
      <div className="flex flex-col gap-4">
        <label className="flex flex-col gap-1.5">
          <span className={labelClassName}>名称</span>
          <input
            value={draft.name}
            onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
            placeholder="我的机场"
            aria-required="true"
            disabled={busy}
            className={inputClassName}
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className={labelClassName}>URL</span>
          <input
            type="url"
            value={draft.url}
            onChange={(event) => setDraft((prev) => ({ ...prev, url: event.target.value }))}
            placeholder="https://example.com/sub"
            aria-required="true"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            inputMode="url"
            disabled={busy}
            className={`${inputClassName} font-mono`}
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className={labelClassName}>User-Agent（可选）</span>
          <input
            value={draft.userAgent}
            onChange={(event) => setDraft((prev) => ({ ...prev, userAgent: event.target.value }))}
            placeholder="留空使用默认 clash.meta"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            disabled={busy}
            className={`${inputClassName} font-mono`}
          />
          <span className="text-xs text-zinc-500 dark:text-zinc-400">
            部分订阅源按 UA 返回不同格式（如 clash / sing-box）
          </span>
        </label>
      </div>
    </BottomSheet>
  );
}
