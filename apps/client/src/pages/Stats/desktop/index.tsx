import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Card, Tabs } from "@pp/ui";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { statsClear, statsToday, STATS_TODAY_KEY, toErrorMessage } from "@pp/client-core";
import { toastError, toastSuccess, useProxyStatus } from "@pp/client-core";
import DailyTable from "./DailyTable";
import RecordsTable from "./RecordsTable";
import { formatBytes } from "./utils";

// 清空后需要失效的前缀 key（keys.ts 中 statsDailyKey / statsRecordsKey 的首段）。
const STATS_DAILY_PREFIX = ["stats_daily"] as const;
const STATS_RECORDS_PREFIX = ["stats_records"] as const;

/** 汇总条单项。 */
function SummaryItem({ label, value }: { label: string; value: string }) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>{label}</Card.Title>
      </Card.Header>
      <Card.Content>
        <span className="text-lg font-semibold">{value}</span>
      </Card.Content>
    </Card>
  );
}

/** 流量统计详情页：今日汇总 + 按目标聚合 / 连接明细 两个视图（检索 + 排序）。 */
export default function Stats() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const running = status?.core_running ?? false;

  const [clearOpen, setClearOpen] = useState(false);

  // 统计持久化在本地 SQLite：核心停止时也展示已有数据，仅运行中轮询刷新。
  const { data: today } = useQuery({
    queryKey: STATS_TODAY_KEY,
    queryFn: statsToday,
    refetchInterval: running ? 5000 : false,
    retry: false,
  });

  const clearMutation = useMutation({
    mutationFn: statsClear,
    onSuccess: async () => {
      toastSuccess("统计数据已清空");
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: STATS_TODAY_KEY }),
        queryClient.invalidateQueries({ queryKey: STATS_DAILY_PREFIX }),
        queryClient.invalidateQueries({ queryKey: STATS_RECORDS_PREFIX }),
      ]);
    },
    onError: (err: unknown) => {
      toastError(toErrorMessage(err));
    },
  });

  const proxied = (today?.proxied_upload_bytes ?? 0) + (today?.proxied_download_bytes ?? 0);
  const total = (today?.upload_bytes ?? 0) + (today?.download_bytes ?? 0);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Button variant="tertiary" size="sm" aria-label="返回" onPress={() => navigate(-1)}>
            <ArrowLeftIcon className="size-4" aria-hidden="true" />
          </Button>
          <div>
            <h1 className="text-xl font-semibold">流量统计</h1>
            <p className="text-sm text-muted">按目标聚合的日统计与连接明细（本地持久化）</p>
          </div>
        </div>
        <Button variant="danger" size="sm" onPress={() => setClearOpen(true)}>
          清空统计
        </Button>
      </div>

      {/* 今日汇总条 */}
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <SummaryItem label="已代理流量" value={today ? formatBytes(proxied) : "-"} />
        <SummaryItem label="总流量" value={today ? formatBytes(total) : "-"} />
        <SummaryItem label="连接数" value={today ? String(today.connection_count) : "-"} />
        <SummaryItem label="目标数" value={today ? String(today.target_count) : "-"} />
      </div>

      <Tabs>
        <Tabs.ListContainer>
          <Tabs.List aria-label="统计视图">
            <Tabs.Tab id="daily">
              按目标聚合
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="records">
              连接明细
              <Tabs.Indicator />
            </Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>

        <Tabs.Panel className="pt-4" id="daily">
          <DailyTable running={running} />
        </Tabs.Panel>
        <Tabs.Panel className="pt-4" id="records">
          <RecordsTable running={running} />
        </Tabs.Panel>
      </Tabs>

      {/* 清空统计确认对话框 */}
      <AlertDialog.Backdrop
        isOpen={clearOpen}
        onOpenChange={(open) => {
          if (!open) setClearOpen(false);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>清空统计</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">确定清空全部流量统计（聚合 + 明细）吗？该操作不可撤销。</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setClearOpen(false)}>
                取消
              </Button>
              <Button
                slot="close"
                variant="danger"
                isPending={clearMutation.isPending}
                onPress={() => {
                  clearMutation.mutate();
                  setClearOpen(false);
                }}
              >
                清空
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
