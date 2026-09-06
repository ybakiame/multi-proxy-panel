import { useState } from "react";
import { Alert, Button, Card, Label, ListBox, Select, Switch } from "@heroui/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  clearLogs,
  exportLogs,
  getLogs,
  listLogFiles,
  openExportDir,
  platformInfo,
  readLogFileTail,
  toErrorMessage,
} from "@pp/client-core";
import type { LogEntry } from "@pp/client-core";
import { LOGS_KEY, LOG_FILES_KEY, PLATFORM_KEY } from "@pp/client-core";
import { MobileBackHeader } from "../layout/mobile/MobileBackHeader";
import { toastError, toastSuccess } from "@pp/client-core";

/** 级别过滤选项（空串 = 不过滤；其余与后端 `min_level` 对齐，默认 info）。 */
const LEVEL_OPTIONS = [
  { id: "", label: "全部" },
  { id: "error", label: "error" },
  { id: "warn", label: "warn" },
  { id: "info", label: "info" },
  { id: "debug", label: "debug" },
] as const;

/** 条数选择（id 为字符串以匹配 Select 的 Key）。 */
const LIMIT_OPTIONS = [
  { id: "200", label: "200" },
  { id: "500", label: "500" },
  { id: "1000", label: "1000" },
] as const;

/** 日志级别展示名（后端级别为大写：`ERROR`/`WARN`/`INFO`/`DEBUG`/`TRACE`）。 */
const LEVEL_LABELS: Record<string, string> = {
  error: "ERROR",
  warn: "WARN",
  info: "INFO",
  debug: "DEBUG",
  trace: "TRACE",
};

/** 级别行配色：libbox 源（Kotlin 侧 sing-box 日志）用 accent 区分；
 *  其余按级别 error 红 / warn 黄 / 默认前景色。 */
function levelClass(level: string, target: string): string {
  if (target === "libbox") {
    return "text-accent";
  }
  const normalized = level.toLowerCase();
  if (normalized === "error") {
    return "text-danger";
  }
  if (normalized === "warn") {
    return "text-warning";
  }
  return "text-foreground";
}

/** RFC3339 时间戳转为本地可读时间；解析失败时原样展示。 */
function formatTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/**
 * Android 应用数据目录路径美化：`/data/user/0/` 是 Android 多用户符号链接
 * （`0` = 主用户），与 `/data/data/` 指向同一目录；展示为更通用、用户更熟悉的
 * `/data/data/` 等价形式。非 Android 路径原样返回。
 */
function beautifyAndroidPath(path: string): string {
  return path.replace(/^\/data\/user\/0\//, "/data/data/");
}

const LOGS_REFETCH_INTERVAL_MS = 2000;

export default function Logs() {
  const queryClient = useQueryClient();
  const [minLevel, setMinLevel] = useState<string>("info");
  const [limit, setLimit] = useState(500);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [exportCopied, setExportCopied] = useState(false);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [filesLoading, setFilesLoading] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);

  const { data: os } = useQuery<string>({
    queryKey: PLATFORM_KEY,
    queryFn: async () => {
      const info = await platformInfo();
      return info.os;
    },
    staleTime: Infinity,
    retry: false,
  });

  const isAndroid = os === "android";

  const {
    data: entries = [],
    error: logsError,
    isLoading: logsLoading,
  } = useQuery<LogEntry[]>({
    queryKey: [...LOGS_KEY, limit, minLevel],
    queryFn: () => getLogs(limit, minLevel || undefined),
    refetchInterval: autoRefresh ? LOGS_REFETCH_INTERVAL_MS : false,
    retry: false,
  });

  const { data: logFiles = [] } = useQuery<string[]>({
    queryKey: LOG_FILES_KEY,
    queryFn: listLogFiles,
    retry: false,
  });

  // 当前选中文件被日志滚动清理/移除时，同步清空选择与内容。
  const prevLogFilesRef = useState(logFiles);
  if (prevLogFilesRef[0] !== logFiles) {
    if (selectedFile && !logFiles.includes(selectedFile)) {
      setSelectedFile(null);
      setFileContent(null);
    }
    prevLogFilesRef[1](logFiles);
  }

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await queryClient.invalidateQueries({ queryKey: LOGS_KEY });
    } catch {
      // invalidateQueries 不抛异常
    }
    setRefreshing(false);
  };

  const handleExport = async () => {
    try {
      const path = await exportLogs();
      setExportPath(path);
      toastSuccess("日志已导出");
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  const handleCopyPath = async () => {
    if (!exportPath) {
      return;
    }
    await navigator.clipboard.writeText(exportPath);
    setExportCopied(true);
    window.setTimeout(() => setExportCopied(false), 2000);
  };

  const handleClear = async () => {
    try {
      await clearLogs();
      toastSuccess("日志已清空");
      await queryClient.invalidateQueries({ queryKey: LOGS_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  const handleRefreshFiles = async () => {
    setFilesLoading(true);
    try {
      await queryClient.invalidateQueries({ queryKey: LOG_FILES_KEY });
    } catch {
      // ignore
    }
    setFilesLoading(false);
  };

  const handleSelectFile = async (name: string) => {
    if (!name) {
      setSelectedFile(null);
      setFileContent(null);
      return;
    }
    setSelectedFile(name);
    setFileContent(null);
    setFileError(null);
    try {
      setFileContent(await readLogFileTail(name, 1000));
    } catch (err) {
      setFileError(toErrorMessage(err));
      setFileContent(null);
    }
  };

  const handleOpenDownloads = async () => {
    try {
      await openExportDir();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <MobileBackHeader title="日志" />
      <div>
        <h1 className="text-xl font-semibold">日志</h1>
        <p className="text-sm text-muted">后端与前端错误日志（内存环形缓冲，最新在前）</p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>运行日志</Card.Title>
          <Card.Description>前端错误经 log_frontend 写入同一管道，级别过滤默认 info</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="logs-level">级别</Label>
              <Select
                id="logs-level"
                aria-label="日志级别"
                value={minLevel}
                onChange={(value) => setMinLevel(String(value ?? "info"))}
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {LEVEL_OPTIONS.map((option) => (
                      <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                        {option.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>

            <div className="flex flex-col gap-1">
              <Label htmlFor="logs-limit">条数</Label>
              <Select
                id="logs-limit"
                aria-label="日志条数"
                value={String(limit)}
                onChange={(value) => setLimit(Number(value ?? 500))}
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {LIMIT_OPTIONS.map((option) => (
                      <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                        {option.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>

            <Switch isSelected={autoRefresh} onChange={(next) => setAutoRefresh(next)}>
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
                自动刷新
              </Switch.Content>
            </Switch>

            <Button variant="secondary" isPending={refreshing || logsLoading} onPress={() => void handleRefresh()}>
              刷新
            </Button>
          </div>

          {/* 日志列表：外层限制高度滚动，内层移动端横向滚动，避免横向溢出 body。 */}
          <div className="max-h-[60vh] overflow-auto">
            {entries.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
                <span className="text-sm text-muted">暂无日志</span>
                <span className="text-xs text-muted/80">产生运行或前端日志后，将显示在此</span>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <ul className="min-w-[640px] space-y-0.5 font-mono text-xs leading-5">
                  {entries.map((entry, index) => (
                    <li
                      key={`${entry.ts}-${index}`}
                      className={`whitespace-nowrap ${levelClass(entry.level, entry.target)}`}
                    >
                      <span className="text-muted">[{formatTime(entry.ts)}]</span>{" "}
                      <span className="font-semibold">{LEVEL_LABELS[entry.level.toLowerCase()] ?? entry.level}</span>{" "}
                      <span className="text-muted">{entry.target}</span>: {entry.message}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </Card.Content>
        <Card.Footer className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Button variant="secondary" onPress={() => void handleExport()}>
              导出日志
            </Button>
            <Button variant="tertiary" onPress={() => void handleClear()}>
              清空
            </Button>
          </div>
          {exportPath && (
            <div className="flex min-w-0 items-center gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate font-mono text-muted" title={beautifyAndroidPath(exportPath)}>
                {beautifyAndroidPath(exportPath)}
              </span>
              {isAndroid && (
                <Button size="sm" variant="secondary" onPress={() => void handleOpenDownloads()}>
                  打开下载目录
                </Button>
              )}
              <Button size="sm" variant="tertiary" onPress={() => void handleCopyPath()}>
                {exportCopied ? "已复制" : "复制"}
              </Button>
            </div>
          )}
        </Card.Footer>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>历史日志</Card.Title>
          <Card.Description>按文件查看 `data_dir/logs` 下的滚动/固定日志文件，最多 1000 行</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="flex min-w-64 flex-1 flex-col gap-1">
              {logFiles.length === 0 ? (
                <p className="text-xs text-warning">暂无日志文件，点击「刷新文件」重试</p>
              ) : (
                <>
                  <Label htmlFor="logs-history-file">文件</Label>
                  <Select
                    id="logs-history-file"
                    aria-label="历史日志文件"
                    placeholder="选择文件"
                    value={selectedFile ?? ""}
                    onChange={(value) => void handleSelectFile(String(value ?? ""))}
                    fullWidth
                  >
                    <Select.Trigger>
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover>
                      <ListBox>
                        {logFiles.map((file) => (
                          <ListBox.Item key={file} id={file} textValue={file}>
                            {file}
                            <ListBox.ItemIndicator />
                          </ListBox.Item>
                        ))}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                </>
              )}
            </div>
            <Button variant="secondary" isPending={filesLoading} onPress={() => void handleRefreshFiles()}>
              刷新文件
            </Button>
          </div>

          {fileError && (
            <Alert status="danger">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>历史日志读取失败</Alert.Title>
                <Alert.Description className="break-all">{fileError}</Alert.Description>
              </Alert.Content>
            </Alert>
          )}

          <div className="flex flex-col gap-2">
            <span className="text-xs text-muted">按文件查看，最多 1000 行</span>
            <div className="max-h-96 overflow-auto rounded-medium border border-border bg-default-50 p-3">
              {fileContent === null ? (
                <p className="text-xs text-muted">选择左侧文件后显示其尾部内容</p>
              ) : (
                <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-5 text-foreground">
                  {fileContent || "(空文件)"}
                </pre>
              )}
            </div>
          </div>
        </Card.Content>
      </Card>

      {logsError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>日志加载失败</Alert.Title>
            <Alert.Description className="break-all">{toErrorMessage(logsError)}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
