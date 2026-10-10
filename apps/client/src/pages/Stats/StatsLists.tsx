import { DataList, type DataListColumn, Chip } from "@pp/ui";
import type { DailyStatRow, ConnRecordRow, DailySort, RecordSort } from "@pp/client-core";
import { formatBytes, formatTime, formatRule } from "./utils";

function SortHeader<T extends string>({
  id,
  label,
  sort,
  desc,
  onSort,
}: {
  id: T;
  label: string;
  sort: T;
  desc: boolean;
  onSort: (id: T) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSort(id)}
      aria-label={`${label}排序${sort === id ? (desc ? "，当前降序" : "，当前升序") : ""}`}
      className="flex items-center gap-1 whitespace-nowrap hover:text-foreground"
    >
      {label}
      {sort === id && <span aria-hidden="true">{desc ? "↓" : "↑"}</span>}
    </button>
  );
}
interface ListProps<T, S extends string> {
  rows: T[];
  sort: S;
  desc: boolean;
  onSort: (id: S) => void;
}
export function DailyList({ rows, sort, desc, onSort }: ListProps<DailyStatRow, DailySort>) {
  const sorted = (id: DailySort, label: string) => (
    <SortHeader id={id} label={label} sort={sort} desc={desc} onSort={onSort} />
  );
  const columns: DataListColumn<DailyStatRow>[] = [
    { id: "target", header: "目标", accessorKey: "target" },
    { id: "ip", header: "目的 IP", accessorKey: "destination_ip" },
    { id: "rule", header: "命中规则", cell: (r) => formatRule(r.rule, r.rule_payload) },
    { id: "outbound", header: "出站节点", accessorKey: "outbound" },
    { id: "count", header: sorted("count", "连接数"), accessorKey: "conn_count" },
    { id: "upload", header: sorted("upload", "上行"), cell: (r) => formatBytes(r.upload_bytes) },
    { id: "download", header: sorted("download", "下行"), cell: (r) => formatBytes(r.download_bytes) },
    { id: "total", header: sorted("total", "合计"), cell: (r) => formatBytes(r.upload_bytes + r.download_bytes) },
    { id: "last", header: sorted("last_seen", "最后时间"), cell: (r) => formatTime(r.last_seen) },
  ];
  return (
    <DataList
      aria-label="按目标聚合统计"
      rows={rows}
      columns={columns}
      getRowId={(r) => JSON.stringify([r.date, r.target, r.destination_ip, r.rule, r.rule_payload, r.outbound])}
      renderCard={(row) => (
        <div className="flex flex-col gap-1 px-4 py-3">
          <span className="truncate text-sm font-semibold text-zinc-900 dark:text-zinc-100" title={row.target}>
            {row.target}
          </span>
          <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">
            {row.outbound} · {formatRule(row.rule, row.rule_payload)}
          </span>
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span className="tabular-nums">
              ↑ {formatBytes(row.upload_bytes)} · ↓ {formatBytes(row.download_bytes)}
            </span>
            <span className="tabular-nums">
              {row.conn_count} 次 · {formatTime(row.last_seen)}
            </span>
          </div>
        </div>
      )}
    />
  );
}
export function RecordsList({ rows, sort, desc, onSort }: ListProps<ConnRecordRow, RecordSort>) {
  const sorted = (id: RecordSort, label: string) => (
    <SortHeader id={id} label={label} sort={sort} desc={desc} onSort={onSort} />
  );
  const columns: DataListColumn<ConnRecordRow>[] = [
    { id: "host", header: "Host", cell: (r) => r.host || r.target },
    { id: "chain", header: "链路", accessorKey: "chain" },
    { id: "rule", header: "规则", cell: (r) => formatRule(r.rule, r.rule_payload) },
    { id: "outbound", header: "出站", accessorKey: "outbound" },
    {
      id: "network",
      header: "网络",
      cell: (r) => (
        <Chip
          color={
            r.network.toLowerCase() === "tcp" ? "success" : r.network.toLowerCase() === "udp" ? "warning" : "default"
          }
        >
          {r.network.toUpperCase()}
        </Chip>
      ),
    },
    { id: "upload", header: sorted("upload", "上行"), cell: (r) => formatBytes(r.upload) },
    { id: "download", header: sorted("download", "下行"), cell: (r) => formatBytes(r.download) },
    { id: "total", header: sorted("total", "合计"), cell: (r) => formatBytes(r.upload + r.download) },
    { id: "started", header: sorted("started", "开始时间"), cell: (r) => formatTime(r.started_at) },
    { id: "ended", header: sorted("ended", "结束时间"), cell: (r) => formatTime(r.ended_at) },
  ];
  return (
    <DataList
      aria-label="连接明细"
      rows={rows}
      columns={columns}
      getRowId={(r) => String(r.id)}
      renderCard={(row) => (
        <div className="flex flex-col gap-1 px-4 py-3">
          <span
            className="truncate text-sm font-semibold text-zinc-900 dark:text-zinc-100"
            title={row.host || row.target}
          >
            {row.host || row.target}
          </span>
          <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">
            {row.chain} · {formatRule(row.rule, row.rule_payload)}
          </span>
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span className="tabular-nums">
              ↑ {formatBytes(row.upload)} · ↓ {formatBytes(row.download)}
            </span>
            <span className="tabular-nums">
              {formatTime(row.started_at)} ~ {formatTime(row.ended_at)}
            </span>
          </div>
        </div>
      )}
    />
  );
}
