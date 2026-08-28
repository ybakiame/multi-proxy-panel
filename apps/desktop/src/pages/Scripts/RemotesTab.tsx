import { useCallback, useRef, useState } from "react";
import { Alert, Avatar, Button, Card, Input, Label, ListBox, Modal, Select, Switch, Table } from "@heroui/react";
import {
  addRemote,
  detectRemote,
  fetchRemotes,
  getRemoteIcon,
  listRemotes,
  removeRemote,
  updateRemote,
  toErrorMessage,
} from "../../api";
import type { DetectRemoteView, FetchReport, RemoteResource } from "../../api";
import {
  ArgEdit,
  ArgValueEdit,
  REMOTE_DIALECT_OPTIONS,
  deriveNameFromUrl,
  groupArgsByTag,
  normalizeDialect,
  normalizeKind,
} from "./utils";

interface RemotesTabProps {
  remotes: RemoteResource[];
  setRemotes: React.Dispatch<React.SetStateAction<RemoteResource[]>>;
  iconCache: Record<string, string>;
  setIconCache: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  busy: boolean;
  setBusy: React.Dispatch<React.SetStateAction<boolean>>;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  fetchResult: FetchReport | null;
  setFetchResult: React.Dispatch<React.SetStateAction<FetchReport | null>>;
}

export default function RemotesTab({
  remotes,
  setRemotes,
  iconCache,
  setIconCache,
  busy,
  setBusy,
  setError,
  fetchResult,
  setFetchResult,
}: RemotesTabProps) {
  const [addOpen, setAddOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newUrl, setNewUrl] = useState("");
  const [newDescription, setNewDescription] = useState("");
  const [newKind, setNewKind] = useState<string>("Script");
  const [newDialect, setNewDialect] = useState<string>("Surge");
  const [newInterval, setNewInterval] = useState<string>("86400");
  const [detecting, setDetecting] = useState(false);
  const [detectInfo, setDetectInfo] = useState<string | null>(null);
  const [newIcon, setNewIcon] = useState<string | null>(null);
  const [newIconFailed, setNewIconFailed] = useState(false);
  const [argEdits, setArgEdits] = useState<ArgEdit[]>([]);
  const lastDetectedUrlRef = useRef("");

  const [editOpen, setEditOpen] = useState(false);
  const [editRemote, setEditRemote] = useState<RemoteResource | null>(null);
  const [editForm, setEditForm] = useState({
    name: "",
    description: "",
    url: "",
    kind: "Script",
    dialect: "Surge",
    interval: "86400",
  });
  const [editArgs, setEditArgs] = useState<ArgValueEdit[]>([]);
  const [editIcon, setEditIcon] = useState<string | null>(null);
  const [editIconFailed, setEditIconFailed] = useState(false);
  const [editDetecting, setEditDetecting] = useState(false);
  const [editDetectInfo, setEditDetectInfo] = useState<string | null>(null);

  const refreshRemotes = useCallback(async () => {
    try {
      const list = await listRemotes();
      setRemotes(list);
      const icons: Record<string, string> = {};
      await Promise.allSettled(
        list
          .filter((r) => r.icon)
          .map(async (r) => {
            const dataUrl = await getRemoteIcon(r.name);
            if (dataUrl) icons[r.name] = dataUrl;
          }),
      );
      setIconCache(icons);
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, [setRemotes, setIconCache, setError]);

  const handleDetect = useCallback(async () => {
    const url = newUrl.trim();
    if (!url || url === lastDetectedUrlRef.current) return;
    lastDetectedUrlRef.current = url;
    setDetecting(true);
    setError(null);
    setDetectInfo(null);
    try {
      const result: DetectRemoteView = await detectRemote(url);
      const kind = normalizeKind(result.kind);
      const dialect = normalizeDialect(result.dialect);
      if (kind) setNewKind(kind);
      if (dialect) setNewDialect(dialect);
      setNewName((prev) => (prev.trim() !== "" ? prev : result.meta?.name?.trim() || deriveNameFromUrl(url)));
      setNewDescription((prev) => (prev.trim() !== "" ? prev : (result.meta?.desc?.trim() ?? "")));
      if (result.meta) {
        setNewIcon(result.meta.icon ?? null);
        setNewIconFailed(false);
        setArgEdits(
          (result.meta.arguments ?? []).map((arg) => ({
            key: arg.key,
            default_value: arg.default_value,
            description: arg.description,
            kind: (arg.kind ?? "Input") as "Input" | "Select",
            options: arg.options ?? [],
            tag: arg.tag ?? null,
            value: "",
          })),
        );
      }
      const parts: string[] = [];
      if (kind) parts.push(kind === "Script" ? "脚本" : "片段");
      if (dialect) parts.push(dialect);
      if (result.meta?.arguments?.length) parts.push(`${result.meta.arguments.length} 个模块参数`);
      if (result.meta?.name) {
        setDetectInfo(`已识别：${result.meta.name}${parts.length ? `（${parts.join(" / ")}）` : ""}`);
      } else if (parts.length > 0) {
        setDetectInfo(`已识别类型：${parts.join(" / ")}`);
      } else {
        setDetectInfo("未识别出类型与元数据，可手动填写");
      }
    } catch (err) {
      lastDetectedUrlRef.current = "";
      setDetectInfo(null);
      setError(toErrorMessage(err));
    } finally {
      setDetecting(false);
    }
  }, [newUrl, setError]);

  const resetAddForm = useCallback(() => {
    setNewName("");
    setNewUrl("");
    setNewDescription("");
    setNewKind("Script");
    setNewDialect("Surge");
    setNewInterval("86400");
    setNewIcon(null);
    setNewIconFailed(false);
    setDetectInfo(null);
    setArgEdits([]);
    lastDetectedUrlRef.current = "";
  }, []);

  const handleArgChange = useCallback((key: string, value: string) => {
    setArgEdits((prev) => prev.map((arg) => (arg.key === key ? { ...arg, value } : arg)));
  }, []);

  const handleAdd = async () => {
    const interval = Number(newInterval);
    setBusy(true);
    setError(null);
    try {
      await addRemote({
        name: newName.trim(),
        url: newUrl.trim(),
        kind: newKind as RemoteResource["kind"],
        dialect: newDialect,
        description: newDescription.trim() || null,
        update_interval_secs: Number.isFinite(interval) && interval > 0 ? interval : 86400,
        enabled: true,
        icon: newIcon,
        argument_values: argEdits
          .filter((arg) => arg.value.trim() !== "")
          .map((arg) => [arg.key, arg.value.trim()] as [string, string]),
        arguments: argEdits.map((arg) => ({
          key: arg.key,
          default_value: arg.default_value,
          description: arg.description,
          kind: arg.kind,
          options: arg.options,
          tag: arg.tag,
        })),
      });
      setAddOpen(false);
      resetAddForm();
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (name: string) => {
    setBusy(true);
    setError(null);
    try {
      await removeRemote(name);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (remote: RemoteResource) => {
    const next = { ...remote, enabled: !remote.enabled };
    setBusy(true);
    setError(null);
    try {
      await removeRemote(remote.name);
      await addRemote(next);
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
      await refreshRemotes();
    } finally {
      setBusy(false);
    }
  };

  const handleOpenEdit = (remote: RemoteResource) => {
    setEditRemote(remote);
    setEditForm({
      name: remote.name,
      description: remote.description ?? "",
      url: remote.url,
      kind: remote.kind,
      dialect: remote.dialect,
      interval: String(remote.update_interval_secs),
    });
    const specs = remote.arguments ?? [];
    setEditArgs(
      specs.map((arg) => {
        const found = (remote.argument_values ?? []).find(([key]) => key === arg.key);
        return { ...arg, value: found?.[1] ?? "" };
      }),
    );
    setEditIcon(remote.icon ?? null);
    setEditIconFailed(false);
    setEditDetecting(false);
    setEditDetectInfo(null);
    setError(null);
    setEditOpen(true);
  };

  const handleEditDetect = useCallback(async () => {
    const url = editForm.url.trim();
    if (!url) return;
    setEditDetecting(true);
    setError(null);
    setEditDetectInfo(null);
    try {
      const result: DetectRemoteView = await detectRemote(url);
      const kind = normalizeKind(result.kind);
      const dialect = normalizeDialect(result.dialect);
      if (kind) setEditForm((prev) => ({ ...prev, kind }));
      if (dialect) setEditForm((prev) => ({ ...prev, dialect }));
      const metaDesc = result.meta?.desc?.trim();
      if (metaDesc) setEditForm((prev) => ({ ...prev, description: metaDesc }));
      if (result.meta?.icon) {
        setEditIcon(result.meta.icon);
        setEditIconFailed(false);
      }
      const newSpecs = result.meta?.arguments ?? [];
      if (newSpecs.length > 0) {
        setEditArgs((prev) =>
          newSpecs.map((arg) => {
            const found = prev.find((item) => item.key === arg.key);
            return { ...arg, value: found?.value ?? "" };
          }),
        );
      }
      const parts: string[] = [];
      if (kind) parts.push(kind);
      if (dialect) parts.push(dialect);
      if (newSpecs.length > 0) parts.push(`${newSpecs.length} 个模块参数`);
      if (result.meta?.name) {
        setEditDetectInfo(`已识别：${result.meta.name}${parts.length ? `（${parts.join(" / ")}）` : ""}`);
      } else if (parts.length > 0) {
        setEditDetectInfo(`已识别：${parts.join(" / ")}`);
      } else {
        setEditDetectInfo("未识别出类型与元数据");
      }
    } catch (err) {
      setEditDetectInfo(null);
      setError(toErrorMessage(err));
    } finally {
      setEditDetecting(false);
    }
  }, [editForm.url, setError]);

  const resetEditDetectState = useCallback(() => {
    setEditIcon(null);
    setEditIconFailed(false);
    setEditDetecting(false);
    setEditDetectInfo(null);
  }, []);

  const handleEditSave = async () => {
    if (!editRemote) return;
    const interval = Number(editForm.interval);
    setBusy(true);
    setError(null);
    try {
      const next: RemoteResource = {
        ...editRemote,
        name: editForm.name.trim(),
        description: editForm.description.trim() || null,
        url: editForm.url.trim(),
        kind: editForm.kind as RemoteResource["kind"],
        dialect: editForm.dialect,
        update_interval_secs: Number.isFinite(interval) && interval > 0 ? interval : 86400,
        icon: editIcon,
        argument_values: editArgs
          .filter((arg) => arg.value.trim() !== "")
          .map((arg) => [arg.key, arg.value.trim()] as [string, string]),
      };
      await updateRemote(next);
      setEditOpen(false);
      setEditRemote(null);
      resetEditDetectState();
      await refreshRemotes();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleFetch = async () => {
    setBusy(true);
    setError(null);
    setFetchResult(null);
    try {
      setFetchResult(await fetchRemotes());
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <Card.Header>
          <Card.Title>远程资源</Card.Title>
          <Card.Description>脚本 / 配置片段订阅，按间隔拉取并落盘缓存</Card.Description>
        </Card.Header>
        <Card.Content>
          {remotes.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-muted">暂无远程资源</span>
              <span className="text-xs text-muted/80">点击「添加资源」创建第一条订阅</span>
            </div>
          ) : (
            <Table>
              <Table.ScrollContainer>
                <Table.Content aria-label="远程资源" className="min-w-[720px]">
                  <Table.Header>
                    <Table.Column>图标</Table.Column>
                    <Table.Column isRowHeader>名称</Table.Column>
                    <Table.Column>描述</Table.Column>
                    <Table.Column>类型</Table.Column>
                    <Table.Column>更新间隔</Table.Column>
                    <Table.Column>启用</Table.Column>
                    <Table.Column>操作</Table.Column>
                  </Table.Header>
                  <Table.Body>
                    {remotes.map((remote) => (
                      <Table.Row key={remote.name}>
                        <Table.Cell>
                          <Avatar size="sm" className="h-6 w-6">
                            {remote.icon ? (
                              <Avatar.Image src={iconCache[remote.name] ?? remote.icon} alt={`${remote.name} 图标`} />
                            ) : null}
                            <Avatar.Fallback color="accent">
                              {(remote.name.charAt(0) || "?").toUpperCase()}
                            </Avatar.Fallback>
                          </Avatar>
                        </Table.Cell>
                        <Table.Cell className="max-w-[180px] truncate">
                          <span title={remote.name}>{remote.name}</span>
                        </Table.Cell>
                        <Table.Cell className="max-w-[200px] truncate">
                          <span title={remote.description ?? "-"}>{remote.description ?? "-"}</span>
                        </Table.Cell>
                        <Table.Cell>
                          {remote.kind === "Script"
                            ? "脚本"
                            : `片段 / ${normalizeDialect(remote.dialect) ?? remote.dialect}`}
                        </Table.Cell>
                        <Table.Cell>{formatInterval(remote.update_interval_secs)}</Table.Cell>
                        <Table.Cell>
                          <Switch
                            aria-label={`启用 ${remote.name}`}
                            isSelected={remote.enabled}
                            onChange={() => void handleToggle(remote)}
                          >
                            <Switch.Content>
                              <Switch.Control>
                                <Switch.Thumb />
                              </Switch.Control>
                              <span className="sr-only">{remote.enabled ? "启用" : "停用"}</span>
                            </Switch.Content>
                          </Switch>
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex items-center gap-2">
                            <Button
                              size="sm"
                              variant="tertiary"
                              isDisabled={busy}
                              onPress={() => handleOpenEdit(remote)}
                            >
                              编辑
                            </Button>
                            <Button
                              size="sm"
                              variant="tertiary"
                              isDisabled={busy}
                              onPress={() => void handleRemove(remote.name)}
                            >
                              删除
                            </Button>
                          </div>
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button
              variant="secondary"
              isPending={busy}
              isDisabled={remotes.length === 0}
              onPress={() => void handleFetch()}
            >
              立即更新
            </Button>
            <Button
              variant="primary"
              isDisabled={busy}
              onPress={() => {
                resetAddForm();
                setAddOpen(true);
              }}
            >
              添加资源
            </Button>
          </div>
        </Card.Footer>
      </Card>

      {fetchResult && (
        <Alert status={fetchResult.warnings.length > 0 ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>更新完成</Alert.Title>
            <Alert.Description>
              成功拉取 {fetchResult.fetched} 个资源：脚本 {fetchResult.scripts}、重写 {fetchResult.rewrites}、任务{" "}
              {fetchResult.tasks}
              {fetchResult.warnings.length > 0 && `，警告 ${fetchResult.warnings.length} 条`}
            </Alert.Description>
            {fetchResult.warnings.length > 0 && (
              <ul className="mt-2 list-inside list-disc space-y-1 break-words text-sm">
                {fetchResult.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            )}
          </Alert.Content>
        </Alert>
      )}

      {/* Add Modal */}
      <Modal.Backdrop isOpen={addOpen} onOpenChange={setAddOpen}>
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>添加远程资源</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-name">名称</Label>
                <Input
                  id="remote-name"
                  aria-label="资源名"
                  value={newName}
                  onChange={(event) => setNewName(event.target.value)}
                  placeholder="my-rules"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-desc">描述</Label>
                <Input
                  id="remote-desc"
                  aria-label="资源描述"
                  value={newDescription}
                  onChange={(event) => setNewDescription(event.target.value)}
                  placeholder="资源描述（可选）"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-url">URL</Label>
                <div className="flex gap-2">
                  <Input
                    id="remote-url"
                    aria-label="资源 URL"
                    value={newUrl}
                    onChange={(event) => setNewUrl(event.target.value)}
                    onBlur={() => void handleDetect()}
                    placeholder="https://example.com/rules.conf"
                    fullWidth
                  />
                  <Button
                    variant="secondary"
                    isPending={detecting}
                    isDisabled={newUrl.trim().length === 0}
                    onPress={() => void handleDetect()}
                  >
                    嗅探
                  </Button>
                </div>
                {detectInfo && <span className="break-words text-xs text-muted">{detectInfo}</span>}
                {newIcon && !newIconFailed && (
                  <div className="mt-1 flex items-center gap-2">
                    <img
                      src={newIcon}
                      alt="资源图标"
                      className="h-8 w-8 rounded object-contain"
                      onError={() => setNewIconFailed(true)}
                    />
                    <span className="text-xs text-muted">已检测到图标</span>
                  </div>
                )}
              </div>
              <Select
                className="w-full"
                placeholder="选择类型"
                value={newKind}
                onChange={(value) => setNewKind(String(value ?? "Script"))}
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
                value={newDialect}
                onChange={(value) => setNewDialect(String(value ?? "Surge"))}
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
                <Label htmlFor="remote-interval">更新间隔（秒）</Label>
                <Input
                  id="remote-interval"
                  aria-label="更新间隔（秒）"
                  type="number"
                  min="60"
                  value={newInterval}
                  onChange={(event) => setNewInterval(event.target.value)}
                  fullWidth
                />
              </div>
              {argEdits.length > 0 && (
                <div className="flex flex-col gap-3">
                  <Label>模块参数</Label>
                  {groupArgsByTag(argEdits).map((group) => (
                    <div key={group.tag ?? "__untagged"} className="flex flex-col gap-2">
                      {group.tag && (
                        <span className="border-b border-border/40 pb-1 text-xs font-medium text-muted">
                          {group.tag}
                        </span>
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
              <Button slot="close" variant="secondary" onPress={() => setAddOpen(false)}>
                取消
              </Button>
              <Button
                variant="primary"
                isPending={busy}
                isDisabled={newName.trim().length === 0 || newUrl.trim().length === 0}
                onPress={() => void handleAdd()}
              >
                添加
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* Edit Modal */}
      <Modal.Backdrop isOpen={editOpen} onOpenChange={setEditOpen}>
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>编辑远程资源</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-edit-name">名称</Label>
                <Input
                  id="remote-edit-name"
                  aria-label="资源名"
                  value={editForm.name}
                  onChange={(event) => setEditForm({ ...editForm, name: event.target.value })}
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-edit-desc">描述</Label>
                <Input
                  id="remote-edit-desc"
                  aria-label="资源描述"
                  value={editForm.description}
                  onChange={(event) => setEditForm({ ...editForm, description: event.target.value })}
                  placeholder="资源描述（可选）"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="remote-edit-url">URL</Label>
                <div className="flex gap-2">
                  <Input
                    id="remote-edit-url"
                    aria-label="资源 URL"
                    value={editForm.url}
                    onChange={(event) => setEditForm({ ...editForm, url: event.target.value })}
                    placeholder="https://example.com/rules.conf"
                    fullWidth
                  />
                  <Button
                    variant="secondary"
                    isPending={editDetecting}
                    isDisabled={editForm.url.trim().length === 0}
                    onPress={() => void handleEditDetect()}
                  >
                    嗅探
                  </Button>
                </div>
                {editDetectInfo && <span className="break-words text-xs text-muted">{editDetectInfo}</span>}
                {editIcon && !editIconFailed && (
                  <div className="mt-1 flex items-center gap-2">
                    <img
                      src={editIcon}
                      alt="资源图标"
                      className="h-8 w-8 rounded object-contain"
                      onError={() => setEditIconFailed(true)}
                    />
                    <span className="text-xs text-muted">已检测到图标</span>
                  </div>
                )}
              </div>
              <Select
                className="w-full"
                placeholder="选择类型"
                value={editForm.kind}
                onChange={(value) => setEditForm({ ...editForm, kind: String(value ?? "Script") })}
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
                value={editForm.dialect}
                onChange={(value) => setEditForm({ ...editForm, dialect: String(value ?? "Surge") })}
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
                <Label htmlFor="remote-edit-interval">更新间隔（秒）</Label>
                <Input
                  id="remote-edit-interval"
                  aria-label="更新间隔（秒）"
                  type="number"
                  min="60"
                  value={editForm.interval}
                  onChange={(event) => setEditForm({ ...editForm, interval: event.target.value })}
                  fullWidth
                />
              </div>
              {editArgs.length > 0 && (
                <div className="flex flex-col gap-3">
                  <Label>模块参数</Label>
                  {groupArgsByTag(editArgs).map((group) => (
                    <div key={group.tag ?? "__untagged"} className="flex flex-col gap-2">
                      {group.tag && (
                        <span className="border-b border-border/40 pb-1 text-xs font-medium text-muted">
                          {group.tag}
                        </span>
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
                              onChange={(value) =>
                                setEditArgs((prev) =>
                                  prev.map((item) =>
                                    item.key === arg.key ? { ...item, value: String(value ?? "") } : item,
                                  ),
                                )
                              }
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
                              onChange={(event) =>
                                setEditArgs((prev) =>
                                  prev.map((item) =>
                                    item.key === arg.key ? { ...item, value: event.target.value } : item,
                                  ),
                                )
                              }
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
              <Button
                slot="close"
                variant="secondary"
                onPress={() => {
                  resetEditDetectState();
                  setEditOpen(false);
                }}
              >
                取消
              </Button>
              <Button
                variant="primary"
                isPending={busy}
                isDisabled={editForm.name.trim().length === 0 || editForm.url.trim().length === 0}
                onPress={() => void handleEditSave()}
              >
                保存
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}

import { formatInterval } from "./utils";
