import { useState } from "react";
import {
  CheckCircleIcon,
  ExclamationCircleIcon,
  InformationCircleIcon,
  MinusCircleIcon,
  PlayIcon,
} from "@heroicons/react/24/outline";
import { diagnoseConnectivityRun, toErrorMessage, toastError } from "@pp/client-core";
import type { DiagReport, DiagStatus } from "@pp/client-core";
import { SubPageShell } from "../../components/SubPageShell";
import { Button, Card, Chip, Spinner, inputClassName } from "../../components/ui";
import type { ChipColor } from "../../components/ui";

const inputClass = `${inputClassName} font-mono`;

const STATUS_META: Record<DiagStatus, { label: string; color: ChipColor }> = {
  ok: { label: "正常", color: "success" },
  fail: { label: "失败", color: "danger" },
  skip: { label: "跳过", color: "default" },
  info: { label: "说明", color: "accent" },
};

function StatusIcon({ status }: { status: DiagStatus }) {
  const className = "size-5 shrink-0";
  switch (status) {
    case "ok":
      return <CheckCircleIcon className={`${className} text-green-500`} aria-hidden="true" />;
    case "fail":
      return <ExclamationCircleIcon className={`${className} text-red-500`} aria-hidden="true" />;
    case "info":
      return <InformationCircleIcon className={`${className} text-primary`} aria-hidden="true" />;
    default:
      return <MinusCircleIcon className={`${className} text-zinc-400`} aria-hidden="true" />;
  }
}

/**
 * 连通性诊断页（开发者工具，路由 `/settings/dev-tools/diagnose`）。
 *
 * 输入目标域名（默认 google.com）后沿流量路径分步诊断：系统 DNS / 直连 TCP / 各生效
 * DNS 服务器真实查询 / 核心状态（含降级）/ 经核心 mixed 入站全链路 / 主分组出站延迟 /
 * TUN 说明。结果按步骤卡片渲染（状态图标 + 耗时 + 结论 + 明细），顶部汇总失败数。
 * 用于定位「开代理后断连」类问题：DNS 全挂 → 核心 DNS 模块；全链路失败而出站正常 →
 * 核心路由/入站侧；出站失败 → 节点问题。
 */
export default function DiagnosePage() {
  const [domain, setDomain] = useState("google.com");
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<DiagReport | null>(null);

  const canRun = domain.trim() !== "" && !running;

  const handleRun = async () => {
    if (!canRun) return;
    setRunning(true);
    try {
      setReport(await diagnoseConnectivityRun(domain.trim()));
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setRunning(false);
    }
  };

  return (
    <SubPageShell title="连通性诊断" backTo="/settings/dev-tools">
      {/* 目标输入 + 运行 */}
      <Card>
        <Card.Content className="flex flex-col gap-3">
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">目标域名</span>
            <input
              value={domain}
              onChange={(event) => setDomain(event.target.value)}
              placeholder="google.com"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              inputMode="url"
              disabled={running}
              aria-label="目标域名"
              className={inputClass}
            />
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              沿流量路径分步检查：DNS 解析 → 直连 → 各 DNS 服务器 → 核心全链路 → 代理出站
            </span>
          </label>
          <Button
            variant="primary"
            className="min-h-12 w-full"
            isDisabled={!canRun}
            isPending={running}
            onPress={() => void handleRun()}
          >
            <PlayIcon className="size-4" aria-hidden="true" />
            {running ? "诊断中…" : "开始诊断"}
          </Button>
        </Card.Content>
      </Card>

      {/* 汇总 */}
      {report && (
        <Card>
          <Card.Content className="flex items-center justify-between gap-3 py-3">
            <span className="text-sm text-zinc-900 dark:text-zinc-100">诊断完成：{report.domain}</span>
            <Chip color={report.failed_steps === 0 ? "success" : "danger"}>
              {report.failed_steps === 0 ? "全部通过" : `${report.failed_steps} 项失败`}
            </Chip>
          </Card.Content>
        </Card>
      )}

      {/* 分步结果 */}
      {report?.steps.map((step) => {
        const meta = STATUS_META[step.status];
        return (
          <Card key={step.key}>
            <Card.Content className="flex flex-col gap-2 py-3">
              <div className="flex items-center gap-2">
                <StatusIcon status={step.status} />
                <span className="min-w-0 flex-1 truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
                  {step.name}
                </span>
                <Chip color={meta.color} className="shrink-0">
                  {meta.label}
                </Chip>
                {step.duration_ms > 0 && (
                  <span className="shrink-0 text-xs text-zinc-500 dark:text-zinc-400">{step.duration_ms}ms</span>
                )}
              </div>
              <span className="text-xs text-zinc-700 dark:text-zinc-300">{step.summary}</span>
              {step.detail !== "" && (
                <pre className="overflow-x-auto whitespace-pre-wrap rounded-lg bg-zinc-100 p-2 font-mono text-xs text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400">
                  {step.detail}
                </pre>
              )}
            </Card.Content>
          </Card>
        );
      })}

      {/* 初始空态 */}
      {!report && !running && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-2 py-10 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">输入目标域名后点击「开始诊断」</span>
            <span className="text-xs text-zinc-400 dark:text-zinc-500">
              推荐对照：google.com（境外，预期直连失败、代理成功）与一个境内域名（预期直连成功）
            </span>
          </Card.Content>
        </Card>
      )}

      {running && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-10 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-zinc-500 dark:text-zinc-400">正在分步诊断，弱网下最长约一分钟…</span>
          </Card.Content>
        </Card>
      )}
    </SubPageShell>
  );
}
