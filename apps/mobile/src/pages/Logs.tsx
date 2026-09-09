import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, ArrowUpTrayIcon, TrashIcon } from "@heroicons/react/24/outline";
import { Alert, AlertDialog, Button, Card, Spinner } from "@heroui/react";
import {
  LOG_FILES_KEY,
  clearLogs,
  exportLogs,
  listLogFiles,
  readLogFileTail,
  toastError,
  toastSuccess,
  toErrorMessage,
} from "@pp/client-core";
import { BackHeader } from "../components/BackHeader";

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

/**
 * 日志页（ADR-0003 M5，路由 `/logs`，不在 TabBar）。
 *
 * - 文件列表：`listLogFiles`（`data_dir/logs` 下滚动/内核日志，文件名 + 说明副标题）；
 *   点击选中后经 `readLogFileTail(name, 1000)` 读取尾部内容（等宽滚动区，自动滚到底部）；
 * - 操作：刷新（BackHeader 右上，重拉文件列表 + 重读选中文件）、导出（`exportLogs`，
 *   Android 走 Kotlin 插件 zip 到系统 Downloads，成功 toast 展示导出路径）、
 *   清空（`clearLogs` 仅清空内存日志缓冲、不影响磁盘文件，确认弹窗）；
 * - 空态：无日志文件时引导点击「刷新文件」。
 * 不做实时流式（tail 手动刷新即可），不复制 desktop 的内存环形缓冲视图。
 */
export default function Logs() {
  const queryClient = useQueryClient();

  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [pendingClear, setPendingClear] = useState(false);

  const { data: logFiles = [], isLoading: filesLoading } = useQuery<string[]>({
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
      toastSuccess(`日志已导出：${path}`);
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setExporting(false);
  };

  const handleClearConfirm = async () => {
    setPendingClear(false);
    try {
      await clearLogs();
      toastSuccess("内存日志缓冲已清空");
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader
        title="日志"
        action={
          <Button
            variant="tertiary"
            isIconOnly
            aria-label="刷新日志"
            isPending={refreshing}
            onPress={() => void handleRefresh()}
          >
            {!refreshing && <ArrowPathIcon className="size-5" aria-hidden="true" />}
          </Button>
        }
      />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {/* 日志文件列表 */}
        <Card>
          <Card.Header>
            <Card.Title>日志文件</Card.Title>
            <Card.Description>`data_dir/logs` 下的滚动与内核日志文件</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-2">
            {filesLoading && logFiles.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
                <Spinner aria-hidden="true" />
                <span className="text-sm text-muted">正在读取日志目录…</span>
              </div>
            ) : logFiles.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
                <span className="text-sm text-muted">暂无日志文件</span>
                <span className="text-xs text-muted/80">产生运行日志后会自动生成滚动文件，可点击右上角刷新重试</span>
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
                      onClick={() => void handleSelectFile(name)}
                      className={`flex flex-col items-start gap-0.5 rounded-xl border px-3 py-2.5 text-left transition-colors active:opacity-80 ${
                        selected
                          ? "border-primary/60 bg-primary/10"
                          : "border-border/60 bg-surface hover:bg-surface-secondary/60"
                      }`}
                    >
                      <span className="w-full truncate font-mono text-sm font-medium text-foreground" title={name}>
                        {name}
                      </span>
                      <span className="truncate text-xs text-muted">{describeLogFile(name)}</span>
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
                <span className="text-sm text-muted">正在读取日志…</span>
              </div>
            ) : readError ? (
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>日志读取失败</Alert.Title>
                  <Alert.Description className="break-all">{readError}</Alert.Description>
                </Alert.Content>
              </Alert>
            ) : fileContent === null ? (
              <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <span className="text-sm text-muted">未选择文件</span>
                <span className="text-xs text-muted/80">从上方列表选择一个日志文件开始查看</span>
              </div>
            ) : (
              <div
                ref={tailBoxRef}
                className="max-h-[55vh] overflow-auto rounded-medium border border-border bg-default-50 p-3"
              >
                <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-5 text-foreground">
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
              <p className="w-full break-all font-mono text-xs text-muted" title={exportPath}>
                已导出：{exportPath}
              </p>
            )}
          </Card.Footer>
        </Card>
      </div>

      {/* 清空确认：clearLogs 仅清内存缓冲，不删磁盘文件 */}
      <AlertDialog.Backdrop
        isOpen={pendingClear}
        onOpenChange={(open) => {
          if (!open) setPendingClear(false);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>清空日志</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">将清空内存中的日志缓冲（不影响磁盘上的日志文件），确定继续吗？</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingClear(false)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={() => void handleClearConfirm()}>
                清空
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
