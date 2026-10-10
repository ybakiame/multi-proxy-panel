import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Chip, Input, Spinner, Table } from "@pp/ui";
import { statsRecords, statsRecordsKey, toErrorMessage } from "@pp/client-core";
import type { RecordSort } from "@pp/client-core";
import { formatBytes, formatTime, useDebouncedValue } from "./utils";

/** Network type chip color. */
function networkColor(network: string): "default" | "success" | "warning" {
  switch (network.toLowerCase()) {
    case "tcp":
      return "success";
    case "udp":
      return "warning";
    default:
      return "default";
  }
}

/** 可排序表头：点击切换排序字段 / 升降序。 */
function SortableColumn({
  id,
  label,
  sort,
  desc,
  onSort,
}: {
  id: RecordSort;
  label: string;
  sort: RecordSort;
  desc: boolean;
  onSort: (id: RecordSort) => void;
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

interface RecordsTableProps {
  /** 核心运行中：5 秒轮询刷新明细数据。 */
  running: boolean;
}

/** 「连接明细」视图：已关闭连接的逐条记录。 */
export default function RecordsTable({ running }: RecordsTableProps) {
  const [searchInput, setSearchInput] = useState("");
  const search = useDebouncedValue(searchInput.trim(), 300);
  const [sort, setSort] = useState<RecordSort>("started");
  const [desc, setDesc] = useState(true);

  const query = { search: search || undefined, sort, desc };

  const { data, isLoading, error } = useQuery({
    queryKey: statsRecordsKey(query),
    queryFn: () => statsRecords(query),
    refetchInterval: running ? 5000 : false,
    retry: false,
  });

  /** 同字段点击切换升降序，换字段则重置为降序。 */
  const handleSort = (id: RecordSort) => {
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
        aria-label="搜索连接明细"
        value={searchInput}
        onChange={(event) => setSearchInput(event.target.value)}
        placeholder="搜索目标 / host / IP / 规则 / 出站"
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
          <span className="text-sm text-muted">暂无连接明细</span>
          <span className="text-xs text-muted/80">连接关闭后将在此留下记录</span>
        </div>
      )}

      {!isLoading && data && data.length > 0 && (
        <Table>
          <Table.ScrollContainer>
            <Table.Content aria-label="连接明细" className="min-w-[1080px]">
              <Table.Header>
                <Table.Column isRowHeader>Host</Table.Column>
                <Table.Column>链路</Table.Column>
                <Table.Column>规则</Table.Column>
                <Table.Column>出站</Table.Column>
                <Table.Column>网络</Table.Column>
                <SortableColumn id="upload" label="上行" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="download" label="下行" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="started" label="开始时间" sort={sort} desc={desc} onSort={handleSort} />
                <SortableColumn id="ended" label="结束时间" sort={sort} desc={desc} onSort={handleSort} />
              </Table.Header>
              <Table.Body>
                {data.map((row) => (
                  <Table.Row key={row.id}>
                    <Table.Cell className="max-w-[220px] truncate">
                      <span title={row.host || row.target}>{row.host || row.target}</span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[200px] truncate text-xs">
                      <span title={row.chain}>{row.chain || "-"}</span>
                    </Table.Cell>
                    <Table.Cell className="max-w-[160px] truncate text-xs">
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
                    <Table.Cell>
                      <Chip size="sm" variant="soft" color={networkColor(row.network)}>
                        {row.network.toUpperCase() || "-"}
                      </Chip>
                    </Table.Cell>
                    <Table.Cell className="text-xs">{formatBytes(row.upload)}</Table.Cell>
                    <Table.Cell className="text-xs">{formatBytes(row.download)}</Table.Cell>
                    <Table.Cell className="text-xs font-mono">{formatTime(row.started_at)}</Table.Cell>
                    <Table.Cell className="text-xs font-mono">{formatTime(row.ended_at)}</Table.Cell>
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
