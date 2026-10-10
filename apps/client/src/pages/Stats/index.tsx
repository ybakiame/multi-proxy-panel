import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Segmented, SegmentedButton, IS_MOBILE, InlineAlert } from "@pp/ui";
import { BarsArrowDownIcon, BarsArrowUpIcon, TrashIcon } from "@heroicons/react/24/outline";
import { Button, Card, ConfirmDialog, Spinner, inputClassName } from "@pp/ui";
import {
  STATS_TODAY_KEY,
  statsClear,
  statsDaily,
  statsDailyKey,
  statsRecords,
  statsRecordsKey,
  statsToday,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
} from "@pp/client-core";
import type {
  ConnRecordRow,
  DailyQuery,
  DailySort,
  DailyStatRow,
  RecordSort,
  RecordsQuery,
  TodaySummary,
} from "@pp/client-core";
import { SubPageShell } from "../../components/PageShell";
import { SelectField } from "@pp/ui";
import { formatBytes, useDebouncedValue } from "./utils";
import { DailyList, RecordsList } from "./StatsLists";

/** 轮询间隔（毫秒，核心运行时）。 */
const POLL_MS = 5000;
/** 列表上限（后端默认更高，此处限制渲染规模）。 */
const LIST_LIMIT = 500;

/** 视图分段。 */
type StatsView = "daily" | "records";

const DAILY_SORT_OPTIONS: Array<{ value: DailySort; label: string }> = [
  { value: "total", label: "总量" },
  { value: "upload", label: "上行" },
  { value: "download", label: "下行" },
  { value: "count", label: "连接数" },
  { value: "last_seen", label: "最近" },
];

const RECORD_SORT_OPTIONS: Array<{ value: RecordSort; label: string }> = [
  { value: "total", label: "合计" },
  { value: "upload", label: "上行" },
  { value: "download", label: "下行" },
  { value: "started", label: "开始" },
  { value: "ended", label: "结束" },
];

/** 列表通用空态 / 加载态。 */
function ListPlaceholder({ loading, empty, emptyHint }: { loading: boolean; empty: boolean; emptyHint: string }) {
  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-10 text-center">
        <Spinner aria-hidden="true" />
        <span className="text-sm text-zinc-500 dark:text-zinc-400">正在加载…</span>
      </div>
    );
  }
  if (empty) {
    return (
      <div className="flex flex-col items-center justify-center gap-1.5 py-10 text-center">
        <span className="text-sm text-zinc-500 dark:text-zinc-400">暂无数据</span>
        <span className="text-xs text-zinc-400 dark:text-zinc-500">{emptyHint}</span>
      </div>
    );
  }
  return null;
}

/**
 * 流量统计详情页（路由 `/stats`，不在 TabBar，由首页「今日流量」卡进入）。
 *
 * - 顶部：今日汇总卡（`stats_today`）+ 右上角「清空」（确认后 `stats_clear`
 *   并 invalidate 今日 / 聚合 / 明细全部 query）；
 * - 分段切换：「按目标聚合」（`stats_daily`，日期 + 域名/IP + 规则 + 出站维度）
 *   与「连接明细」（`stats_records`，已关闭连接）；
 * - 检索：搜索框 300ms 防抖后透传后端 `search` 参数；
 * - 排序：字段经 SelectField 选择（两视图各自字段集），方向按钮切换升降序；
 * - 核心运行时今日汇总与当前视图列表 5 秒轮询，未运行时仅展示持久化数据。
 */
export default function Stats() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const running = status?.core_running ?? false;

  const [view, setView] = useState<StatsView>("daily");
  const [searchInput, setSearchInput] = useState("");
  const search = useDebouncedValue(searchInput.trim(), 300);
  const [dailySort, setDailySort] = useState<DailySort>("total");
  const [recordSort, setRecordSort] = useState<RecordSort>(IS_MOBILE ? "total" : "started");
  const [desc, setDesc] = useState(true);
  const [pendingClear, setPendingClear] = useState(false);
  const [clearing, setClearing] = useState(false);

  // ---- 数据：今日汇总 ----
  const { data: today } = useQuery<TodaySummary>({
    queryKey: STATS_TODAY_KEY,
    queryFn: statsToday,
    refetchInterval: running ? POLL_MS : false,
    retry: false,
  });

  // 查询条件对象 memo 化：作为 queryKey 与 invoke 参数，避免每轮渲染生成新 key。
  const dailyQuery = useMemo<DailyQuery>(
    () => ({
      search: search === "" ? undefined : search,
      sort: dailySort,
      desc,
      limit: IS_MOBILE ? LIST_LIMIT : undefined,
    }),
    [search, dailySort, desc],
  );
  const recordsQuery = useMemo<RecordsQuery>(
    () => ({
      search: search === "" ? undefined : search,
      sort: recordSort,
      desc,
      limit: IS_MOBILE ? LIST_LIMIT : undefined,
    }),
    [search, recordSort, desc],
  );

  // ---- 数据：按目标聚合 / 连接明细（仅当前视图启用） ----
  const daily = useQuery<DailyStatRow[]>({
    queryKey: statsDailyKey(dailyQuery),
    queryFn: () => statsDaily(dailyQuery),
    enabled: view === "daily",
    refetchInterval: running ? POLL_MS : false,
    retry: false,
  });
  const records = useQuery<ConnRecordRow[]>({
    queryKey: statsRecordsKey(recordsQuery),
    queryFn: () => statsRecords(recordsQuery),
    enabled: view === "records",
    refetchInterval: running ? POLL_MS : false,
    retry: false,
  });

  const handleClearConfirm = async () => {
    setPendingClear(false);
    setClearing(true);
    try {
      await statsClear();
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: STATS_TODAY_KEY }),
        queryClient.invalidateQueries({ queryKey: ["stats_daily"] }),
        queryClient.invalidateQueries({ queryKey: ["stats_records"] }),
      ]);
      toastSuccess("统计数据已清空");
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setClearing(false);
  };

  const proxiedTotal = (today?.proxied_upload_bytes ?? 0) + (today?.proxied_download_bytes ?? 0);
  const sortOptions = view === "daily" ? DAILY_SORT_OPTIONS : RECORD_SORT_OPTIONS;
  const sortValue = view === "daily" ? dailySort : recordSort;

  return (
    <SubPageShell
      title="流量统计"
      action={
        <Button
          variant="tertiary"
          isIconOnly
          aria-label="清空统计数据"
          isPending={clearing}
          className="size-11"
          onPress={() => setPendingClear(true)}
        >
          {!clearing && <TrashIcon className="size-5" aria-hidden="true" />}
        </Button>
      }
    >
      {/* 今日汇总 */}
      <Card>
        <Card.Header>
          <Card.Title>今日汇总</Card.Title>
          <Card.Description>
            {today ? `${today.date} · 已代理流量（排除直连与拦截）` : "本地持久化统计"}
          </Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <span className="text-2xl font-semibold tabular-nums text-zinc-900 dark:text-zinc-100">
            {formatBytes(proxiedTotal)}
          </span>
          <div className="grid grid-cols-2 gap-4">
            {(
              [
                ["总流量", formatBytes((today?.upload_bytes ?? 0) + (today?.download_bytes ?? 0))],
                ["总上行", formatBytes(today?.upload_bytes ?? 0)],
                ["总下行", formatBytes(today?.download_bytes ?? 0)],
                ["连接数", String(today?.connection_count ?? 0)],
                ["目标数", String(today?.target_count ?? 0)],
              ] as Array<[string, string]>
            ).map(([label, value]) => (
              <div key={label} className="flex flex-col gap-0.5">
                <span className="text-xs text-zinc-500 dark:text-zinc-400">{label}</span>
                <span className="text-base font-medium tabular-nums text-zinc-900 dark:text-zinc-100">{value}</span>
              </div>
            ))}
          </div>
        </Card.Content>
      </Card>

      {/* 视图分段切换 */}
      <Segmented strong>
        <SegmentedButton active={view === "daily"} onClick={() => setView("daily")} className="min-h-11">
          按目标聚合
        </SegmentedButton>
        <SegmentedButton active={view === "records"} onClick={() => setView("records")} className="min-h-11">
          连接明细
        </SegmentedButton>
      </Segmented>

      {/* 检索 + 排序 */}
      <div className="flex flex-col gap-2">
        <input
          type="search"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          placeholder={view === "daily" ? "搜索目标 / 规则 / 出站" : "搜索域名 / IP / 规则 / 出站"}
          aria-label="搜索流量记录"
          className={inputClassName}
        />
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1">
            <SelectField
              label="排序字段"
              options={sortOptions}
              value={sortValue}
              onChange={(next) => {
                if (view === "daily") {
                  setDailySort(next as DailySort);
                } else {
                  setRecordSort(next as RecordSort);
                }
              }}
            />
          </div>
          <Button
            variant="secondary"
            aria-label={desc ? "当前降序，点击切换升序" : "当前升序，点击切换降序"}
            className="min-h-12 shrink-0"
            onPress={() => setDesc((prev) => !prev)}
          >
            {desc ? (
              <BarsArrowDownIcon className="size-5" aria-hidden="true" />
            ) : (
              <BarsArrowUpIcon className="size-5" aria-hidden="true" />
            )}
            {desc ? "降序" : "升序"}
          </Button>
        </div>
      </div>

      {/* 列表 */}
      <Card>
        <Card.Content className="p-0">
          {view === "daily" ? (
            <ListPlaceholder
              loading={daily.isLoading}
              empty={(daily.data ?? []).length === 0}
              emptyHint={search === "" ? "产生代理流量后会自动记录" : "无匹配的聚合记录，换个关键词试试"}
            />
          ) : (
            <ListPlaceholder
              loading={records.isLoading}
              empty={(records.data ?? []).length === 0}
              emptyHint={search === "" ? "连接关闭后会自动记录明细" : "无匹配的连接记录，换个关键词试试"}
            />
          )}
          {view === "daily" && daily.error && (
            <InlineAlert kind="danger" title="统计读取失败">
              {toErrorMessage(daily.error)}
            </InlineAlert>
          )}
          {view === "records" && records.error && (
            <InlineAlert kind="danger" title="统计读取失败">
              {toErrorMessage(records.error)}
            </InlineAlert>
          )}
          {view === "daily" && (daily.data ?? []).length > 0 && (
            <DailyList
              rows={daily.data ?? []}
              sort={dailySort}
              desc={desc}
              onSort={(next) => {
                if (next === dailySort) setDesc(!desc);
                else {
                  setDailySort(next);
                  setDesc(true);
                }
              }}
            />
          )}
          {view === "records" && (records.data ?? []).length > 0 && (
            <RecordsList
              rows={records.data ?? []}
              sort={recordSort}
              desc={desc}
              onSort={(next) => {
                if (next === recordSort) setDesc(!desc);
                else {
                  setRecordSort(next);
                  setDesc(true);
                }
              }}
            />
          )}
        </Card.Content>
      </Card>

      {/* 清空确认 */}
      <ConfirmDialog
        opened={pendingClear}
        title="清空统计数据"
        danger
        confirmText="清空"
        onConfirm={() => void handleClearConfirm()}
        onClose={() => setPendingClear(false)}
      >
        <p className="break-words">将清空全部已记录的流量统计（聚合与连接明细），此操作不可撤销，确定继续吗？</p>
      </ConfirmDialog>
    </SubPageShell>
  );
}
