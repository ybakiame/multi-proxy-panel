import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Chip, Input, Spinner, Table } from "@heroui/react";
import { statsDaily, statsDailyKey, toErrorMessage } from "@pp/client-core";
import type { DailySort } from "@pp/client-core";
import { formatBytes, formatTime, useDebouncedValue } from "./utils";

/** 可排序表头：点击切换排序字段 / 升降序。 */
function SortableColumn({
  id,
  label,
  sort,
  desc,
  onSort,
}: {
  id: DailySort;
  label: string;
  sort: DailySort;
  desc: boolean;
  onSort: (id: DailySort) => void;
}) {
  const active = sort === id;
  return (
    <Table.Column>
      <button
        type="button"
        className={`flex items-center gap-1 hover:text-foreground ${active ? "text-foreground" : ""}`}
        onClick={() => onSort(id)}
      >
        {label}
        {active && <span aria-hidden="true">{desc ? "↓" : "↑"}</span>}
      </button>
    </Table.Column>
  );
}

interface DailyTableProps {
  /** 核心运行中：5 秒轮询刷新聚合数据。 */
  running: boolean;
}

/** 「按目标聚合」视图：日期 + 域名/IP + 规则 + 出站 维度的日聚合表。 */
export default function DailyTable({ running }: DailyTableProps) {
  const [searchInput, setSearchInput] = useState("");
  const search = useDebouncedValue(searchInput.trim(), 300);
  const [sort, setSort] = useState<DailySort>("total");
  const [desc, setDesc] = useState(true);

  const query = { search: search || undefined, sort, desc };

  const { data, isLoading, error } = useQuery({
    queryKey: statsDailyKey(query),
    queryFn: () => statsDaily(query),
    refetchInterval: running ? 5000 : false,
    retry: false,
  });

  /** 同字段点击切换升降序，换字段则重置为降序。 */
  const handleSort = (id: DailySort) => {
    if (id === sort) {
      setDesc((prev) => !prev);
    } else {
      setSort(id);
      setDesc(true);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <Input
        aria-label="搜索聚合统计"
        value={searchInput}
        onChange={(event) => setSearchInput(event.target.value)}
        placeholder="搜索目标 / IP / 规则 / 出站"
        fullWidth
      />

      {error && <span className="text-xs text-danger">{toErrorMessage(error)}</span>}

      {isLoading && (
        <div className="flex items-center justify-center py-12">
          <Spinner />
        </div>
      )}

      {!isLoading && !error && (!data || data.length === 0) && (
        <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
          <span className="text-sm text-muted">暂无聚合数据</span>
          <span className="text-xs text-muted/80">产生流量后将按「目标 + 规则 + 出站」聚合展示</span>
        </div>
      )}

      {!isLoading && data && data.length > 0 && (
        <Table>
          <Table.ScrollContainer>
            <Table.Content aria-label="按目标聚合统计" className="min-w-[1080px]">
              <Table.Header>
                <Table.Column isRowHeader>目标</Table.Column>
                <Table.Column>目的 IP</Table.Column>
                <Table.Column>命中规则</Table.Column>
                <Table.Column>出站节点</Table.Column>
                <SortableColumn id="count" label="连接数" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="upload" label="上行" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="download" label="下行" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="total" label="合计" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="last_seen" label="最后时间" sort={sort} desc={desc} onSort={handleSort} />
              </Table.Header>
              <Table.Body>
                {data.map((row) => (
                  <Table.Row key={`${row.date}|${row.target}|${row.rule}|${row.rule_payload}|${row.outbound}`}>
                    <Table.Cell className="max-w-[220px] truncate">
                      <span title={row.target}>{row.target}</span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[140px] truncate text-xs">
                      <span title={row.destination_ip}>{row.destination_ip || "-"}</span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[180px] truncate text-xs">
                      <span title={`${row.rule}${row.rule_payload ? `: ${row.rule_payload}` : ""}`}>
                        {row.rule}
                        {row.rule_payload ? ` (${row.rule_payload})` : ""}
                      </span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[160px] truncate text-xs">
                      <span title={row.outbound}>
                        <Chip size="sm" variant="soft" color="default">
                          {row.outbound || "-"}
                        </Chip>
                      </span>
                    </Table.Cell>
                    <Table.Cell className="text-xs">{row.conn_count}</Table.Cell>
                    <Table.Cell className="text-xs">{formatBytes(row.upload_bytes)}</Table.Cell>
                    <Table.Cell className="text-xs">{formatBytes(row.download_bytes)}</Table.Cell>
                    <Table.Cell className="text-xs">{formatBytes(row.upload_bytes + row.download_bytes)}</Table.Cell>
                    <Table.Cell className="text-xs font-mono">{formatTime(row.last_seen)}</Table.Cell>
                  </Table.Row>
                ))}
              </Table.Body>
            </Table.Content>
          </Table.ScrollContainer>
        </Table>
      )}
    </div>
  );
}
