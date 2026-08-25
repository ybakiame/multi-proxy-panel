import { useTranslation } from "react-i18next";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Metric } from "../../api/types";
import { formatBytes } from "../../utils/format";

interface MetricsChartProps {
  metrics: Metric[];
  height?: number;
}

interface MetricPoint {
  time: string;
  cpu: number;
  mem: number;
  disk: number;
  rxRate: number;
  txRate: number;
}

function toPoints(metrics: Metric[]): MetricPoint[] {
  const sorted = [...metrics].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
  );
  return sorted.map((m, i) => {
    const prev = i > 0 ? sorted[i - 1] : null;
    const dt = prev
      ? (new Date(m.timestamp).getTime() - new Date(prev.timestamp).getTime()) / 1000
      : 0;
    const rate = (curr: number, before: number) => (dt > 0 ? Math.max(0, (curr - before) / dt) : 0);
    return {
      time: new Date(m.timestamp).toLocaleTimeString(),
      cpu: m.cpu_percent ?? 0,
      mem: m.mem_total > 0 ? (m.mem_used / m.mem_total) * 100 : 0,
      disk: m.disk_total > 0 ? (m.disk_used / m.disk_total) * 100 : 0,
      rxRate: prev ? rate(m.net_rx, prev.net_rx) : 0,
      txRate: prev ? rate(m.net_tx, prev.net_tx) : 0,
    };
  });
}

const tooltipStyle = {
  backgroundColor: "var(--surface)",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-md)",
  color: "var(--foreground)",
  fontSize: 12,
};

const axisTick = { fill: "var(--muted)", fontSize: 12 };

export function MetricsChart({ metrics, height = 260 }: MetricsChartProps) {
  const { t } = useTranslation();
  const points = toPoints(metrics);

  if (points.length === 0) {
    return (
      <div
        className="flex items-center justify-center text-sm text-muted-foreground"
        style={{ height }}
      >
        {t("metrics.empty")}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
      <div>
        <p className="mb-2 text-sm font-medium text-muted-foreground">{t("metrics.chartUsage")}</p>
        <ResponsiveContainer width="100%" height={height}>
          <LineChart data={points} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" vertical={false} />
            <XAxis
              dataKey="time"
              tick={axisTick}
              tickLine={false}
              axisLine={{ stroke: "var(--border)" }}
              minTickGap={32}
            />
            <YAxis
              tickFormatter={(value: number) => `${Math.round(value)}%`}
              tick={axisTick}
              tickLine={false}
              axisLine={false}
              domain={[0, 100]}
              width={48}
            />
            <Tooltip
              contentStyle={tooltipStyle}
              labelStyle={{ color: "var(--foreground)" }}
              formatter={(value, name) => [`${Number(value).toFixed(1)}%`, name]}
            />
            <Line
              type="monotone"
              dataKey="cpu"
              name={t("metrics.cpu")}
              stroke="var(--accent)"
              dot={false}
              strokeWidth={2}
            />
            <Line
              type="monotone"
              dataKey="mem"
              name={t("metrics.memory")}
              stroke="var(--success)"
              dot={false}
              strokeWidth={2}
            />
            <Line
              type="monotone"
              dataKey="disk"
              name={t("metrics.disk")}
              stroke="var(--warning)"
              dot={false}
              strokeWidth={2}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
      <div>
        <p className="mb-2 text-sm font-medium text-muted-foreground">
          {t("metrics.chartNetwork")}
        </p>
        <ResponsiveContainer width="100%" height={height}>
          <LineChart data={points} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" vertical={false} />
            <XAxis
              dataKey="time"
              tick={axisTick}
              tickLine={false}
              axisLine={{ stroke: "var(--border)" }}
              minTickGap={32}
            />
            <YAxis
              tickFormatter={(value: number) => `${formatBytes(value)}/s`}
              tick={axisTick}
              tickLine={false}
              axisLine={false}
              width={96}
            />
            <Tooltip
              contentStyle={tooltipStyle}
              labelStyle={{ color: "var(--foreground)" }}
              formatter={(value, name) => [`${formatBytes(Number(value))}/s`, name]}
            />
            <Line
              type="monotone"
              dataKey="rxRate"
              name={t("metrics.netRx")}
              stroke="var(--accent)"
              dot={false}
              strokeWidth={2}
            />
            <Line
              type="monotone"
              dataKey="txRate"
              name={t("metrics.netTx")}
              stroke="var(--danger)"
              dot={false}
              strokeWidth={2}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
