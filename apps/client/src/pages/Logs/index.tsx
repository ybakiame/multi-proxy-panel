import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, ArrowUpTrayIcon, TrashIcon } from "@heroicons/react/24/outline";
import { InlineAlert, IS_MOBILE } from "@pp/ui";
import RuntimeLogs from "./RuntimeLogs";
import { Button, Card, ConfirmDialog, Spinner } from "@pp/ui";
import {
  LOG_FILES_KEY,
  LOGS_KEY,
  clearLogs,
  exportLogs,
  listLogFiles,
  readLogFileTail,
  toastError,
  toastSuccess,
  toErrorMessage,
} from "@pp/client-core";
import { SubPageShell } from "../../components/PageShell";

/** 每次读取的尾部行数（后端默认/上限 1000）。 */
const TAIL_LINES = 1000;

/** 日志文件中文说明：按已知文件名规则给出可读副标题（后端只返回文件名列表）。 */
function describeLogFile(name: string): string {
  if (name === "libbox.log") return "内核日志（sing-box / libbox）";
  if (name === "last_start_config.json") return "上次启动的核心配置快照";
  const daily = name.match(/^app\.log\.(\d{4}-\d{2}-\d{2})$/);
  if (daily) return `滚动日志 · ${daily[1]}`;
  if (name === "app.log") return "当前运行日志";
  return "日志文件";
}

/** 双端共用磁盘日志、导出与清空；桌面额外显示运行日志缓冲。 */
export default function Logs() {
  const queryClient = useQueryClient();

  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [exportCopied, setExportCopied] = useState(false);
  const [pendingClear, setPendingClear] = useState(false);

  const {
    data: logFiles = [],
    isLoading: filesLoading,
    error: filesError,
  } = useQuery<string[]>({
    queryKey: LOG_FILES_KEY,
    queryFn: listLogFiles,
    retry: false,
  });

  // tail 内容滚动容器：内容更新后自动滚到底部（最新日志在文件末尾）。
  const tailBoxRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (fileContent === null) return;
    const el = tailBoxRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [fileContent]);

  /** 读取指定文件的尾部内容；写入选区与错误状态。 */
  const loadTail = async (name: string) => {
    setReading(true);
    setReadError(null);
    try {
      setFileContent(await readLogFileTail(name, TAIL_LINES));
    } catch (err) {
      setReadError(toErrorMessage(err));
      setFileContent(null);
    }
    setReading(false);
  };

  const handleSelectFile = async (name: string) => {
    if (name === selectedFile) {
      return;
    }
    setSelectedFile(name);
    setFileContent(null);
    await loadTail(name);
  };

  /** 手动刷新：重拉文件列表；若选中文件仍存在则重读其尾部，已消失则清空选择。 */
  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await queryClient.invalidateQueries({ queryKey: LOG_FILES_KEY });
      const files = queryClient.getQueryData<string[]>(LOG_FILES_KEY);
      if (selectedFile && files && !files.includes(selectedFile)) {
        setSelectedFile(null);
        setFileContent(null);
        setReadError(null);
      } else if (selectedFile) {
        await loadTail(selectedFile);
      }
    } catch {
      // invalidateQueries 不抛异常
    }
    setRefreshing(false);
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const path = await exportLogs();
      setExportPath(path);
      setExportCopied(false);
      toastSuccess(`日志已导出：${path}`);
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setExporting(false);
  };

  const handleCopyPath = async () => {
    if (!exportPath) return;
    try {
      await navigator.clipboard.writeText(exportPath);
      setExportCopied(true);
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  const handleClearConfirm = async () => {
    setPendingClear(false);
    try {
      await clearLogs();
      await queryClient.invalidateQueries({ queryKey: LOGS_KEY });
      toastSuccess("内存日志缓冲已清空");
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  return (
    <SubPageShell
      title="日志"
      action={
        <Button
          variant="tertiary"
          isIconOnly
          aria-label="刷新日志"
          isPending={refreshing}
          isDisabled={reading}
          className="size-11"
          onPress={() => void handleRefresh()}
        >
          {!refreshing && <ArrowPathIcon className="size-5" aria-hidden="true" />}
        </Button>
      }
    >
      {!IS_MOBILE && <RuntimeLogs />}

      {/* 日志文件列表 */}
      <Card>
        <Card.Header>
          <Card.Title>日志文件</Card.Title>
          <Card.Description>应用保存的滚动与内核日志文件</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-2">
          {filesError && (
            <InlineAlert kind="danger" title="日志目录读取失败">
              {toErrorMessage(filesError)}
            </InlineAlert>
          )}
          {filesLoading && logFiles.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-zinc-500 dark:text-zinc-400">正在读取日志目录…</span>
            </div>
          ) : logFiles.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
              <span className="text-sm text-zinc-500 dark:text-zinc-400">暂无日志文件</span>
              <span className="text-xs text-zinc-400 dark:text-zinc-500">
                产生运行日志后会自动生成滚动文件，可点击右上角刷新重试
              </span>
            </div>
          ) : (
            <div className="flex flex-col gap-1.5">
              {logFiles.map((name) => {
                const selected = name === selectedFile;
                return (
                  <button
                    key={name}
                    type="button"
                    aria-pressed={selected}
                    disabled={reading || refreshing}
                    onClick={() => void handleSelectFile(name)}
                    className={`flex flex-col items-start gap-0.5 rounded-xl border px-3 py-2.5 text-left transition-colors active:opacity-80 ${
                      selected
                        ? "border-primary/60 bg-primary/10"
                        : "border-zinc-200 bg-white hover:bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800 dark:hover:bg-zinc-700"
                    }`}
                  >
                    <span
                      className="w-full truncate font-mono text-sm font-medium text-zinc-900 dark:text-zinc-100"
                      title={name}
                    >
                      {name}
                    </span>
                    <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{describeLogFile(name)}</span>
                  </button>
                );
              })}
            </div>
          )}
        </Card.Content>
      </Card>

      {/* 尾部内容查看 */}
      <Card>
        <Card.Header>
          <Card.Title>{selectedFile ? "日志内容" : "文件预览"}</Card.Title>
          <Card.Description>
            {selectedFile ? `最多显示末尾 ${TAIL_LINES} 行 · 自动滚动到底部` : "点击上方文件查看其尾部内容"}
          </Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-2">
          {reading && fileContent === null ? (
            <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-zinc-500 dark:text-zinc-400">正在读取日志…</span>
            </div>
          ) : readError ? (
            <InlineAlert kind="danger" title="日志读取失败">
              <span className="break-all">{readError}</span>
            </InlineAlert>
          ) : fileContent === null ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-zinc-500 dark:text-zinc-400">未选择文件</span>
              <span className="text-xs text-zinc-400 dark:text-zinc-500">从上方列表选择一个日志文件开始查看</span>
            </div>
          ) : (
            <div
              ref={tailBoxRef}
              className="max-h-[55vh] overflow-auto rounded-lg border border-zinc-200 bg-zinc-50 p-3 dark:border-zinc-700 dark:bg-zinc-800"
            >
              <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-5 text-zinc-900 dark:text-zinc-100">
                {fileContent || "(空文件)"}
              </pre>
            </div>
          )}
        </Card.Content>
        <Card.Footer className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              className="min-h-11 flex-1"
              isPending={exporting}
              onPress={() => void handleExport()}
            >
              {!exporting && <ArrowUpTrayIcon className="size-4" aria-hidden="true" />}
              导出日志
            </Button>
            <Button variant="tertiary" className="min-h-11 flex-1" onPress={() => setPendingClear(true)}>
              <TrashIcon className="size-4" aria-hidden="true" />
              清空
            </Button>
          </div>
          {exportPath && (
            <div className="flex w-full min-w-0 items-center gap-2">
              <p className="flex-1 break-all font-mono text-xs text-muted" title={exportPath}>
                已导出：{exportPath}
              </p>
              {!IS_MOBILE && (
                <Button size="sm" variant="tertiary" onPress={() => void handleCopyPath()}>
                  {exportCopied ? "已复制" : "复制"}
                </Button>
              )}
            </div>
          )}
        </Card.Footer>
      </Card>

      {/* 清空确认：clearLogs 仅清内存缓冲，不删磁盘文件 */}
      <ConfirmDialog
        opened={pendingClear}
        title="清空日志"
        danger
        confirmText="清空"
        onConfirm={() => void handleClearConfirm()}
        onClose={() => setPendingClear(false)}
      >
        <p className="break-words">将清空内存中的日志缓冲（不影响磁盘上的日志文件），确定继续吗？</p>
      </ConfirmDialog>
    </SubPageShell>
  );
}
