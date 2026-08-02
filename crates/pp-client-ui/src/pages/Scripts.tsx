import { useCallback, useEffect, useRef, useState } from "react";
import {
  Alert,
  Avatar,
  Button,
  Card,
  Input,
  Label,
  ListBox,
  Modal,
  Select,
  Switch,
  Table,
  Tabs,
  TextArea,
} from "@heroui/react";
import {
  addRemote,
  detectRemote,
  fetchRemotes,
  getRemoteIcon,
  importConfig,
  listRemotes,
  listTasks,
  removeRemote,
  runTask,
  toErrorMessage,
  updateRemote,
} from "../api";
import type { ArgSpecView, DetectRemoteView, FetchReport, ImportSummary, RemoteResource, TaskScriptView } from "../api";

/**
 * 远程资源添加表单的方言选项（仅保留 Surge / Loon）。
 *
 * 旧数据中的 QuantumultX 由 {@link normalizeDialect} 归一为 Loon
 * （Loon 方言同时注入 QuantumultX 与 Surge 两套 API）。
 */
const REMOTE_DIALECT_OPTIONS = [
  { id: "Surge", label: "Surge" },
  { id: "Loon", label: "Loon" },
] as const;

/** 配置导入的方言选项（与 `import_config` 命令的小写方言值一致）。 */
const IMPORT_DIALECT_OPTIONS = [
  { id: "surge", label: "Surge" },
  { id: "loon", label: "Loon" },
] as const;

/** 添加表单里一条模块参数的编辑态（值由用户填写）。 */
interface ArgEdit {
  key: string;
  default_value: string;
  description: string | null;
  kind: "Input" | "Select";
  options: string[];
  tag: string | null;
  value: string;
}

/** 编辑 Modal 中一条参数值的编辑态（声明 + 用户填写值）。 */
interface ArgValueEdit extends ArgSpecView {
  value: string;
}

/** 将 `detect_remote` 返回的类型字符串归一为选项值（大小写防御）。 */
function normalizeKind(kind: string | null): string | null {
  if (!kind) {
    return null;
  }
  const lower = kind.trim().toLowerCase();
  if (lower === "script") {
    return "Script";
  }
  if (lower === "snippet") {
    return "Snippet";
  }
  return kind.trim();
}

/** 将方言字符串归一为 UI 选项：QuantumultX 兼容旧数据 → Loon。 */
function normalizeDialect(dialect: string | null | undefined): string | null {
  if (!dialect) {
    return null;
  }
  const trimmed = dialect.trim();
  if (trimmed === "QuantumultX" || trimmed.toLowerCase() === "quantumultx") {
    return "Loon";
  }
  return trimmed;
}

/** 从 URL 派生资源名：取路径末段文件名并去掉常见后缀（与后端 `derive_name_from_url` 一致）。 */
function deriveNameFromUrl(url: string): string {
  try {
    const parsed = new URL(url);
    const segments = parsed.pathname.split("/").filter(Boolean);
    const last = segments[segments.length - 1];
    if (last) {
      const stem = last.replace(/\.(js|conf|sgmodule|plugin|loon)$/i, "");
      if (stem.trim() !== "") {
        return stem;
      }
    }
  } catch {
    // URL 无法解析时回退为原串
  }
  return url;
}

/** RFC3339 时间戳转为本地可读时间；空值显示占位符。 */
function formatTime(iso: string | null): string {
  if (!iso) {
    return "-";
  }
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/** 更新间隔（秒）转可读文本。 */
function formatInterval(secs: number): string {
  if (secs % 86400 === 0) {
    return `${secs / 86400} 天`;
  }
  if (secs % 3600 === 0) {
    return `${secs / 3600} 小时`;
  }
  return `${secs} 秒`;
}

/** 按参数分组标签（`tag`）分组；无 tag 的归入默认组。 */
function groupArgsByTag<T extends { key: string; tag: string | null }>(args: T[]): { tag: string | null; args: T[] }[] {
  const groups = new Map<string | null, T[]>();
  for (const arg of args) {
    const tag = arg.tag ?? null;
    const group = groups.get(tag) ?? [];
    group.push(arg);
    groups.set(tag, group);
  }
  return Array.from(groups.entries()).map(([tag, items]) => ({ tag, args: items }));
}

export default function Scripts() {
  const [remotes, setRemotes] = useState<RemoteResource[]>([]);
  // 本地图标缓存（name → data URL）：优先本地，远程 URL 兜底。
  const [iconCache, setIconCache] = useState<Record<string, string>>({});
  const [tasks, setTasks] = useState<TaskScriptView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fetchResult, setFetchResult] = useState<FetchReport | null>(null);
  const [runResult, setRunResult] = useState<{ name: string; output: string } | null>(null);
  const [importResult, setImportResult] = useState<ImportSummary | null>(null);

  // 添加资源对话框
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
  // 嗅探解析出的模块参数声明，值由用户填写，随 add_remote 以 argument_values 提交。
  const [argEdits, setArgEdits] = useState<ArgEdit[]>([]);
  // 已嗅探过的 URL：避免失焦与「嗅探」按钮同时触发时重复拉取。
  const lastDetectedUrlRef = useRef("");

  // 已添加资源的编辑对话框（名称/描述/URL/类型/方言/更新间隔 + 模块参数 + 图标）
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
  // 编辑对话框的重新嗅探状态：图标由嗅探结果覆盖，失败标记用于 img onError 回退。
  const [editIcon, setEditIcon] = useState<string | null>(null);
  const [editIconFailed, setEditIconFailed] = useState(false);
  const [editDetecting, setEditDetecting] = useState(false);
  const [editDetectInfo, setEditDetectInfo] = useState<string | null>(null);

  // 配置导入
  const [importText, setImportText] = useState("");
  const [importDialect, setImportDialect] = useState<string>("loon");

  const refreshRemotes = useCallback(async () => {
    try {
      const list = await listRemotes();
      setRemotes(list);
      // 并行加载本地图标缓存：成功写入 data URL，失败忽略（回退远程 URL）。
      const icons: Record<string, string> = {};
      await Promise.allSettled(
        list
          .filter((r) => r.icon)
          .map(async (r) => {
            const dataUrl = await getRemoteIcon(r.name);
            if (dataUrl) {
              icons[r.name] = dataUrl;
            }
          }),
      );
      setIconCache(icons);
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  const refreshTasks = useCallback(async () => {
    try {
      setTasks(await listTasks());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshRemotes();
    void refreshTasks();
  }, [refreshRemotes, refreshTasks]);

  /** 嗅探远端资源：按后缀判定类型/方言，Snippet 可访问时解析元数据并预填表单。 */
  const handleDetect = useCallback(async () => {
    const url = newUrl.trim();
    if (!url || url === lastDetectedUrlRef.current) {
      return;
    }
    lastDetectedUrlRef.current = url;
    setDetecting(true);
    setError(null);
    setDetectInfo(null);
    try {
      const result: DetectRemoteView = await detectRemote(url);
      const kind = normalizeKind(result.kind);
      const dialect = normalizeDialect(result.dialect);
      if (kind) {
        setNewKind(kind);
      }
      if (dialect) {
        setNewDialect(dialect);
      }
      // 名称仅在用户未填写时填充：优先 meta.name，其次从 URL 派生。
      setNewName((prev) => (prev.trim() !== "" ? prev : result.meta?.name?.trim() || deriveNameFromUrl(url)));
      // 描述仅在用户未填写时填充。
      setNewDescription((prev) => (prev.trim() !== "" ? prev : (result.meta?.desc?.trim() ?? "")));
      if (result.meta) {
        setNewIcon(result.meta.icon ?? null);
        setNewIconFailed(false);
        setArgEdits(
          (result.meta.arguments ?? []).map((arg: ArgSpecView) => ({
            key: arg.key,
            default_value: arg.default_value,
            description: arg.description,
            kind: arg.kind ?? "Input",
            options: arg.options ?? [],
            tag: arg.tag ?? null,
            value: "",
          })),
        );
      }
      // 汇总识别结果提示。
      const parts: string[] = [];
      if (kind) {
        parts.push(kind === "Script" ? "脚本" : "片段");
      }
      if (dialect) {
        parts.push(dialect);
      }
      if (result.meta?.arguments?.length) {
        parts.push(`${result.meta.arguments.length} 个模块参数`);
      }
      if (result.meta?.name) {
        setDetectInfo(`已识别：${result.meta.name}${parts.length ? `（${parts.join(" / ")}）` : ""}`);
      } else if (parts.length > 0) {
        setDetectInfo(`已识别类型：${parts.join(" / ")}`);
      } else {
        setDetectInfo("未识别出类型与元数据，可手动填写");
      }
    } catch (err) {
      // 失败时清除去重标记，允许再次嗅探。
      lastDetectedUrlRef.current = "";
      setDetectInfo(null);
      setError(toErrorMessage(err));
    } finally {
      setDetecting(false);
    }
  }, [newUrl]);

  /** 重置添加资源表单（新增/嗅探成功添加后调用）。 */
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
        // 参数声明随添加持久化，供后续「编辑」Modal 按 kind/options 渲染控件。
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

  /** 打开已添加资源的编辑对话框：预填基础字段，并把声明（arguments）与已存值（argument_values）合并。 */
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

  /** 编辑对话框的重新嗅探：覆盖类型/方言/描述/图标与参数声明（同 key 参数保留已填 value），名称不覆盖。 */
  const handleEditDetect = useCallback(async () => {
    const url = editForm.url.trim();
    if (!url) {
      return;
    }
    setEditDetecting(true);
    setError(null);
    setEditDetectInfo(null);
    try {
      const result: DetectRemoteView = await detectRemote(url);
      const kind = normalizeKind(result.kind);
      const dialect = normalizeDialect(result.dialect);
      if (kind) {
        setEditForm((prev) => ({ ...prev, kind }));
      }
      if (dialect) {
        setEditForm((prev) => ({ ...prev, dialect }));
      }
      const metaDesc = result.meta?.desc?.trim();
      if (metaDesc) {
        setEditForm((prev) => ({ ...prev, description: metaDesc }));
      }
      if (result.meta?.icon) {
        setEditIcon(result.meta.icon);
        setEditIconFailed(false);
      }
      // 替换参数声明为新声明，按 key 保留 editArgs 中已填写的 value。
      const newSpecs = result.meta?.arguments ?? [];
      if (newSpecs.length > 0) {
        setEditArgs((prev) =>
          newSpecs.map((arg) => {
            const found = prev.find((item) => item.key === arg.key);
            return { ...arg, value: found?.value ?? "" };
          }),
        );
      }
      // 汇总识别结果提示（风格参考添加对话框）。
      const parts: string[] = [];
      if (kind) {
        parts.push(kind);
      }
      if (dialect) {
        parts.push(dialect);
      }
      if (newSpecs.length > 0) {
        parts.push(`${newSpecs.length} 个模块参数`);
      }
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
  }, [editForm.url]);

  /** 重置编辑对话框的嗅探/图标覆盖状态（取消或保存成功后调用）。 */
  const resetEditDetectState = useCallback(() => {
    setEditIcon(null);
    setEditIconFailed(false);
    setEditDetecting(false);
    setEditDetectInfo(null);
  }, []);

  /** 保存编辑：按 name 全量更新（`update_remote`），参数值仅提交非空项，图标显式提交嗅探覆盖值。 */
  const handleEditSave = async () => {
    if (!editRemote) {
      return;
    }
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

  const handleRunTask = async (name: string) => {
    setBusy(true);
    setError(null);
    setRunResult(null);
    try {
      const output = await runTask(name);
      setRunResult({ name, output });
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleImport = async () => {
    setBusy(true);
    setError(null);
    setImportResult(null);
    try {
      setImportResult(await importConfig(importText, importDialect));
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">脚本</h1>
        <p className="text-sm text-muted">远程脚本 / 配置片段订阅、定时任务调度与三方配置导入</p>
      </div>

      <Tabs>
        <Tabs.ListContainer>
          <Tabs.List aria-label="脚本管理">
            <Tabs.Tab id="remotes">
              远程资源
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="tasks">
              定时任务
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="import">
              配置导入
              <Tabs.Indicator />
            </Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>

        <Tabs.Panel className="flex flex-col gap-4 pt-4" id="remotes">
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
                                  <Avatar.Image
                                    src={iconCache[remote.name] ?? remote.icon}
                                    alt={`${remote.name} 图标`}
                                  />
                                ) : null}
                                <Avatar.Fallback color="accent">
                                  {(remote.name.charAt(0) || "?").toUpperCase()}
                                </Avatar.Fallback>
                              </Avatar>
                            </Table.Cell>
                            <Table.Cell>{remote.name}</Table.Cell>
                            <Table.Cell className="max-w-[200px] truncate">{remote.description ?? "-"}</Table.Cell>
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
                  <ul className="mt-2 list-inside list-disc space-y-1 text-sm">
                    {fetchResult.warnings.map((w) => (
                      <li key={w}>{w}</li>
                    ))}
                  </ul>
                )}
              </Alert.Content>
            </Alert>
          )}
        </Tabs.Panel>

        <Tabs.Panel className="flex flex-col gap-4 pt-4" id="tasks">
          <Card>
            <Card.Header>
              <Card.Title>定时任务</Card.Title>
              <Card.Description>远程订阅中的 cron 任务脚本，需代理运行中且 MITM 已启用</Card.Description>
            </Card.Header>
            <Card.Content>
              {tasks.length === 0 ? (
                <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                  <span className="text-sm text-muted">暂无定时任务</span>
                  <span className="text-xs text-muted/80">远程资源中的 [task_local] / cron 脚本会在此列出</span>
                </div>
              ) : (
                <Table>
                  <Table.ScrollContainer>
                    <Table.Content aria-label="定时任务" className="min-w-[720px]">
                      <Table.Header>
                        <Table.Column isRowHeader>名称</Table.Column>
                        <Table.Column>cron</Table.Column>
                        <Table.Column>下次执行</Table.Column>
                        <Table.Column>上次执行</Table.Column>
                        <Table.Column>上次错误</Table.Column>
                        <Table.Column>操作</Table.Column>
                      </Table.Header>
                      <Table.Body>
                        {tasks.map((task) => (
                          <Table.Row key={task.name}>
                            <Table.Cell>{task.name}</Table.Cell>
                            <Table.Cell className="font-mono text-xs">{task.cron_expr}</Table.Cell>
                            <Table.Cell>{formatTime(task.next_run)}</Table.Cell>
                            <Table.Cell>{formatTime(task.last_run)}</Table.Cell>
                            <Table.Cell className="max-w-[200px] truncate">{task.last_error ?? "-"}</Table.Cell>
                            <Table.Cell>
                              <Button
                                size="sm"
                                variant="secondary"
                                isDisabled={busy}
                                onPress={() => void handleRunTask(task.name)}
                              >
                                运行
                              </Button>
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
              <Button variant="secondary" isDisabled={busy} onPress={() => void refreshTasks()}>
                刷新
              </Button>
            </Card.Footer>
          </Card>

          {runResult && (
            <Alert status="success">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>任务「{runResult.name}」已运行</Alert.Title>
                <Alert.Description className="break-all font-mono text-xs">
                  {runResult.output || "$done()"}
                </Alert.Description>
              </Alert.Content>
            </Alert>
          )}
        </Tabs.Panel>

        <Tabs.Panel className="flex flex-col gap-4 pt-4" id="import">
          <Card>
            <Card.Header>
              <Card.Title>导入配置</Card.Title>
              <Card.Description>粘贴 Surge / Loon 的 rewrite / script / mitm 片段，合并进本地缓存</Card.Description>
            </Card.Header>
            <Card.Content className="flex flex-col gap-4">
              <Select
                className="w-full sm:max-w-[240px]"
                placeholder="选择方言"
                value={importDialect}
                onChange={(value) => setImportDialect(String(value ?? ""))}
              >
                <Label>方言</Label>
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {IMPORT_DIALECT_OPTIONS.map((option) => (
                      <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                        {option.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
              <TextArea
                aria-label="配置片段"
                value={importText}
                onChange={(event) => setImportText(event.target.value)}
                placeholder={
                  "[rewrite_local]\n^https?://example\\.com/api/ url-and-header https://cdn.example.com/$1\n\n[mitm]\nhostname = *.example.com"
                }
                rows={12}
                fullWidth
              />
            </Card.Content>
            <Card.Footer>
              <Button
                variant="primary"
                isPending={busy}
                isDisabled={importText.trim().length === 0}
                onPress={() => void handleImport()}
              >
                导入
              </Button>
            </Card.Footer>
          </Card>

          {importResult && (
            <Alert status={importResult.warnings.length > 0 ? "warning" : "success"}>
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>导入完成</Alert.Title>
                <Alert.Description>
                  重写 {importResult.rewrites}、脚本 {importResult.scripts}、任务 {importResult.tasks}、 主机名{" "}
                  {importResult.hostnames}
                  {importResult.warnings.length > 0 && `，警告 ${importResult.warnings.length} 条`}
                </Alert.Description>
                {importResult.meta?.name && (
                  <div className="mt-2 text-sm">
                    识别为：{importResult.meta.name}
                    {importResult.meta.desc ? ` — ${importResult.meta.desc}` : ""}
                  </div>
                )}
                {importResult.warnings.length > 0 && (
                  <ul className="mt-2 list-inside list-disc space-y-1 text-sm">
                    {importResult.warnings.map((w) => (
                      <li key={w}>{w}</li>
                    ))}
                  </ul>
                )}
              </Alert.Content>
            </Alert>
          )}
        </Tabs.Panel>
      </Tabs>

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

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
                {detectInfo && <span className="text-xs text-muted">{detectInfo}</span>}
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
                            {arg.description && <span className="truncate text-xs text-muted">{arg.description}</span>}
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
                {editDetectInfo && <span className="text-xs text-muted">{editDetectInfo}</span>}
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
                            {arg.description && <span className="truncate text-xs text-muted">{arg.description}</span>}
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
