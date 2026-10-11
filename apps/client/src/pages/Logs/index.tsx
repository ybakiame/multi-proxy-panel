import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, ArrowUpTrayIcon, TrashIcon } from "@heroicons/react/24/outline";
import { InlineAlert, IS_MOBILE, SelectField } from "@pp/ui";
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

/** 将应用日志、核心日志与诊断快照分别选择。 */
function logType(name: string): string {
  if (name === "libbox.log") return "core";
  if (name === "last_start_config.json") return "config";
  return "app";
}
const LOG_TYPES = [
  { value: "app", label: "应用日志" },
  { value: "core", label: "核心日志" },
  { value: "config", label: "启动配置快照" },
];

/** 双端共用磁盘日志、导出与清空；桌面额外显示运行日志缓冲。 */
export default function Logs() {
  const queryClient = useQueryClient();

  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [selectedType, setSelectedType] = useState("app");
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

  const {
    data: fileContent = null,
    error: readError,
    isFetching: reading,
    refetch: refreshTail,
  } = useQuery({
    queryKey: [...LOG_FILES_KEY, "tail", selectedFile],
    queryFn: () => readLogFileTail(selectedFile!, TAIL_LINES),
    enabled: selectedFile !== null,
    retry: false,
  });
  const visibleFiles = logFiles.filter((name) => logType(name) === selectedType);
  const changeType = (value: string) => {
    setSelectedType(value);
    setSelectedFile(null);
  };

  // tail 内容滚动容器：内容更新后自动滚到底部（最新日志在文件末尾）。
  const tailBoxRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (fileContent === null) return;
    const el = tailBoxRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [fileContent]);

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await queryClient.invalidateQueries({ queryKey: LOG_FILES_KEY, exact: true });
      const files = queryClient.getQueryData<string[]>(LOG_FILES_KEY);
      if (selectedFile && files && !files.includes(selectedFile)) setSelectedFile(null);
      else if (selectedFile) await refreshTail();
    } finally {
      setRefreshing(false);
    }
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

      <Card>
        <Card.Header>
          <Card.Title>日志记录</Card.Title>
          <Card.Description>选择日志类型和文件，查看末尾 1000 行</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="grid min-w-0 gap-3 sm:grid-cols-2">
            <div className="flex min-w-0 flex-col gap-2">
              <span className="text-sm text-muted">日志类型</span>
              <SelectField label="日志类型" value={selectedType} options={LOG_TYPES} onChange={changeType} />
            </div>
            <div className="flex min-w-0 flex-col gap-2">
              <span className="text-sm text-muted">日志文件</span>
              <SelectField
                label="日志文件"
                value={selectedFile ?? ""}
                options={visibleFiles.map((name) => ({ value: name, label: name }))}
                onChange={setSelectedFile}
                placeholder={filesLoading ? "正在读取目录…" : visibleFiles.length ? "请选择文件" : "暂无此类文件"}
                disabled={filesLoading || refreshing}
              />
            </div>
          </div>
          {filesError && (
            <InlineAlert kind="danger" title="日志目录读取失败">
              {toErrorMessage(filesError)}
            </InlineAlert>
          )}
          {reading && fileContent === null ? (
            <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-zinc-500 dark:text-zinc-400">正在读取日志…</span>
            </div>
          ) : readError ? (
            <InlineAlert kind="danger" title="日志读取失败">
              <span className="break-all">{toErrorMessage(readError)}</span>
            </InlineAlert>
          ) : fileContent === null ? (
            <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
              <span className="text-sm text-zinc-500 dark:text-zinc-400">未选择文件</span>
              <span className="text-xs text-zinc-400 dark:text-zinc-500">先选择日志类型，再选择文件查看内容</span>
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
