/**
 * Reusable modal for adding or editing a remote resource.
 * Manages its own form state and delegates save / close to the parent.
 */

import { useCallback, useEffect, useState } from "react";
import { Button, Input, Label, ListBox, Modal, Select } from "@heroui/react";
import type { RemoteResource } from "../../../api";
import { REMOTE_DIALECT_OPTIONS, groupArgsByTag } from "../utils";
import { useRemoteSniff } from "./useRemoteSniff";
import type { ArgEdit, RemoteFormState } from "./types";

interface RemoteFormModalProps {
  /** 'add' for new resource, 'edit' for existing. */
  mode: "add" | "edit";
  /** Whether the modal is open. */
  open: boolean;
  /** Existing resource data (required when mode === 'edit'). */
  initialData?: RemoteResource | null;
  /** Called when the modal should close. */
  onClose: () => void;
  /** Called with the built resource on save. */
  onSave: (resource: RemoteResource) => Promise<void>;
  /** Global busy flag (disables save button). */
  busy: boolean;
  /** Report errors upward. */
  setError: (msg: string | null) => void;
}

const DEFAULT_FORM: RemoteFormState = {
  name: "",
  description: "",
  url: "",
  kind: "Script",
  dialect: "Surge",
  interval: "86400",
};

function buildArgEditsFromDetected(
  detected: {
    arguments: {
      key: string;
      default_value: string;
      description: string | null;
      kind: "Input" | "Select";
      options: string[];
      tag: string | null;
    }[];
  },
  prev: ArgEdit[],
): ArgEdit[] {
  return detected.arguments.map((arg) => {
    const found = prev.find((item) => item.key === arg.key);
    return {
      key: arg.key,
      default_value: arg.default_value,
      description: arg.description,
      kind: arg.kind,
      options: arg.options,
      tag: arg.tag,
      value: found?.value ?? "",
    };
  });
}

export default function RemoteFormModal({
  mode,
  open,
  initialData,
  onClose,
  onSave,
  busy,
  setError,
}: RemoteFormModalProps) {
  const [form, setForm] = useState<RemoteFormState>(DEFAULT_FORM);
  const [args, setArgs] = useState<ArgEdit[]>([]);
  const [icon, setIcon] = useState<string | null>(null);
  const [iconFailed, setIconFailed] = useState(false);
  const { detecting, detectInfo, sniff, reset: resetSniff } = useRemoteSniff();

  // Initialize form when opening in edit mode
  useEffect(() => {
    if (!open) return;
    if (mode === "edit" && initialData) {
      setForm({
        name: initialData.name,
        description: initialData.description ?? "",
        url: initialData.url,
        kind: initialData.kind,
        dialect: initialData.dialect,
        interval: String(initialData.update_interval_secs),
      });
      const specs = initialData.arguments ?? [];
      setArgs(
        specs.map((arg) => {
          const found = (initialData.argument_values ?? []).find(([key]) => key === arg.key);
          return { ...arg, value: found?.[1] ?? "" };
        }),
      );
      setIcon(initialData.icon ?? null);
      setIconFailed(false);
    } else {
      setForm(DEFAULT_FORM);
      setArgs([]);
      setIcon(null);
      setIconFailed(false);
    }
    resetSniff();
  }, [open, mode, initialData, resetSniff]);

  const handleDetect = useCallback(async () => {
    const result = await sniff(form.url, setError);
    if (!result) return;

    const kind = result.kind;
    const dialect = result.dialect;

    setForm((prev) => ({
      ...prev,
      kind: kind ?? prev.kind,
      dialect: dialect ?? prev.dialect,
      name: prev.name.trim() !== "" ? prev.name : (result.name ?? prev.name),
      description: prev.description.trim() !== "" ? prev.description : (result.description ?? prev.description),
    }));

    if (result.icon) {
      setIcon(result.icon);
      setIconFailed(false);
    }

    if (result.arguments.length > 0) {
      setArgs((prev) => buildArgEditsFromDetected({ arguments: result.arguments }, prev));
    }
  }, [form.url, sniff, setError]);

  const handleArgChange = useCallback((key: string, value: string) => {
    setArgs((prev) => prev.map((arg) => (arg.key === key ? { ...arg, value } : arg)));
  }, []);

  const handleSave = async () => {
    const interval = Number(form.interval);
    const resource: RemoteResource = {
      name: form.name.trim(),
      url: form.url.trim(),
      kind: form.kind as RemoteResource["kind"],
      dialect: form.dialect,
      description: form.description.trim() || null,
      update_interval_secs: Number.isFinite(interval) && interval > 0 ? interval : 86400,
      enabled: mode === "edit" && initialData ? initialData.enabled : true,
      icon,
      argument_values: args
        .filter((arg) => arg.value.trim() !== "")
        .map((arg) => [arg.key, arg.value.trim()] as [string, string]),
      arguments: args.map((arg) => ({
        key: arg.key,
        default_value: arg.default_value,
        description: arg.description,
        kind: arg.kind,
        options: arg.options,
        tag: arg.tag,
      })),
    };
    await onSave(resource);
  };

  const title = mode === "add" ? "添加远程资源" : "编辑远程资源";
  const saveLabel = mode === "add" ? "添加" : "保存";
  const nameId = mode === "add" ? "remote-name" : "remote-edit-name";
  const descId = mode === "add" ? "remote-desc" : "remote-edit-desc";
  const urlId = mode === "add" ? "remote-url" : "remote-edit-url";
  const intervalId = mode === "add" ? "remote-interval" : "remote-edit-interval";

  return (
    <Modal.Backdrop
      isOpen={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[480px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{title}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex flex-col gap-4">
            <div className="flex flex-col gap-1">
              <Label htmlFor={nameId}>名称</Label>
              <Input
                id={nameId}
                aria-label="资源名"
                value={form.name}
                onChange={(event) => setForm((prev) => ({ ...prev, name: event.target.value }))}
                placeholder={mode === "add" ? "my-rules" : undefined}
                fullWidth
              />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor={descId}>描述</Label>
              <Input
                id={descId}
                aria-label="资源描述"
                value={form.description}
                onChange={(event) => setForm((prev) => ({ ...prev, description: event.target.value }))}
                placeholder="资源描述（可选）"
                fullWidth
              />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor={urlId}>URL</Label>
              <div className="flex gap-2">
                <Input
                  id={urlId}
                  aria-label="资源 URL"
                  value={form.url}
                  onChange={(event) => setForm((prev) => ({ ...prev, url: event.target.value }))}
                  onBlur={() => {
                    if (mode === "add") void handleDetect();
                  }}
                  placeholder="https://example.com/rules.conf"
                  fullWidth
                />
                <Button
                  variant="secondary"
                  isPending={detecting}
                  isDisabled={form.url.trim().length === 0}
                  onPress={() => void handleDetect()}
                >
                  嗅探
                </Button>
              </div>
              {detectInfo && <span className="break-words text-xs text-muted">{detectInfo}</span>}
              {icon && !iconFailed && (
                <div className="mt-1 flex items-center gap-2">
                  <img
                    src={icon}
                    alt="资源图标"
                    className="h-8 w-8 rounded object-contain"
                    onError={() => setIconFailed(true)}
                  />
                  <span className="text-xs text-muted">已检测到图标</span>
                </div>
              )}
            </div>
            <Select
              className="w-full"
              placeholder="选择类型"
              value={form.kind}
              onChange={(value) => setForm((prev) => ({ ...prev, kind: String(value ?? "Script") }))}
            >
              <Label>类型</Label>
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  <ListBox.Item id="Script" textValue="脚本">
                    脚本（纯 JS）
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                  <ListBox.Item id="Snippet" textValue="片段">
                    片段（Surge / Loon 配置）
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                </ListBox>
              </Select.Popover>
            </Select>
            <Select
              className="w-full"
              placeholder="选择方言"
              value={form.dialect}
              onChange={(value) => setForm((prev) => ({ ...prev, dialect: String(value ?? "Surge") }))}
            >
              <Label>方言</Label>
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {REMOTE_DIALECT_OPTIONS.map((option) => (
                    <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                      {option.label}
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  ))}
                </ListBox>
              </Select.Popover>
            </Select>
            <div className="flex flex-col gap-1">
              <Label htmlFor={intervalId}>更新间隔（秒）</Label>
              <Input
                id={intervalId}
                aria-label="更新间隔（秒）"
                type="number"
                min="60"
                value={form.interval}
                onChange={(event) => setForm((prev) => ({ ...prev, interval: event.target.value }))}
                fullWidth
              />
            </div>
            {args.length > 0 && (
              <div className="flex flex-col gap-3">
                <Label>模块参数</Label>
                {groupArgsByTag(args).map((group) => (
                  <div key={group.tag ?? "__untagged"} className="flex flex-col gap-2">
                    {group.tag && (
                      <span className="border-b border-border/40 pb-1 text-xs font-medium text-muted">{group.tag}</span>
                    )}
                    {group.args.map((arg) => (
                      <div key={arg.key} className="flex flex-col gap-1">
                        <div className="flex items-baseline justify-between gap-2">
                          <span className="font-mono text-xs font-medium">{arg.key}</span>
                          {arg.description && (
                            <span className="min-w-0 truncate text-xs text-muted" title={arg.description}>
                              {arg.description}
                            </span>
                          )}
                        </div>
                        {arg.kind === "Select" ? (
                          <Select
                            aria-label={`参数 ${arg.key}`}
                            placeholder={arg.default_value ? `默认：${arg.default_value}` : "选择参数值（可选）"}
                            value={arg.value}
                            onChange={(value) => handleArgChange(arg.key, String(value ?? ""))}
                            fullWidth
                          >
                            <Select.Trigger>
                              <Select.Value />
                              <Select.Indicator />
                            </Select.Trigger>
                            <Select.Popover>
                              <ListBox>
                                {arg.options.length > 0 ? (
                                  arg.options.map((option) => (
                                    <ListBox.Item key={option} id={option} textValue={option}>
                                      {option}
                                      <ListBox.ItemIndicator />
                                    </ListBox.Item>
                                  ))
                                ) : (
                                  <ListBox.Item id="__empty" textValue="无可选选项">
                                    无可选选项
                                  </ListBox.Item>
                                )}
                              </ListBox>
                            </Select.Popover>
                          </Select>
                        ) : (
                          <Input
                            aria-label={`参数 ${arg.key}`}
                            value={arg.value}
                            onChange={(event) => handleArgChange(arg.key, event.target.value)}
                            placeholder={arg.default_value ? `默认：${arg.default_value}` : "填写参数值（可选）"}
                            fullWidth
                          />
                        )}
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            )}
          </Modal.Body>
          <Modal.Footer>
            <Button slot="close" variant="secondary" onPress={onClose}>
              取消
            </Button>
            <Button
              variant="primary"
              isPending={busy}
              isDisabled={form.name.trim().length === 0 || form.url.trim().length === 0}
              onPress={() => void handleSave()}
            >
              {saveLabel}
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
