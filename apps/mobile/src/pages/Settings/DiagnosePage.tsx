import { useState } from "react";
import { Button, Card, Chip, Spinner } from "@heroui/react";
import {
  CheckCircleIcon,
  ExclamationCircleIcon,
  InformationCircleIcon,
  MinusCircleIcon,
  PlayIcon,
} from "@heroicons/react/24/outline";
import { diagnoseConnectivityRun, toErrorMessage, toastError } from "@pp/client-core";
import type { DiagReport, DiagStatus } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 font-mono text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

const STATUS_META: Record<DiagStatus, { label: string; color: "success" | "danger" | "default" | "accent" }> = {
  ok: { label: "正常", color: "success" },
  fail: { label: "失败", color: "danger" },
  skip: { label: "跳过", color: "default" },
  info: { label: "说明", color: "accent" },
};

function StatusIcon({ status }: { status: DiagStatus }) {
  const className = "size-5 shrink-0";
  switch (status) {
    case "ok":
      return <CheckCircleIcon className={`${className} text-success`} aria-hidden="true" />;
    case "fail":
      return <ExclamationCircleIcon className={`${className} text-danger`} aria-hidden="true" />;
    case "info":
      return <InformationCircleIcon className={`${className} text-accent`} aria-hidden="true" />;
    default:
      return <MinusCircleIcon className={`${className} text-muted`} aria-hidden="true" />;
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
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="连通性诊断" backTo="/settings/dev-tools" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {/* 目标输入 + 运行 */}
        <Card>
          <Card.Content className="flex flex-col gap-3">
            <label className="flex flex-col gap-1.5">
              <span className="text-sm font-medium text-foreground">目标域名</span>
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
              <span className="text-xs text-muted">
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
              <span className="text-sm text-foreground">诊断完成：{report.domain}</span>
              <Chip size="sm" variant="soft" color={report.failed_steps === 0 ? "success" : "danger"}>
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
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">{step.name}</span>
                  <Chip size="sm" variant="soft" color={meta.color} className="shrink-0">
                    {meta.label}
                  </Chip>
                  {step.duration_ms > 0 && <span className="shrink-0 text-xs text-muted">{step.duration_ms}ms</span>}
                </div>
                <span className="text-xs text-foreground/90">{step.summary}</span>
                {step.detail !== "" && (
                  <pre className="overflow-x-auto rounded-lg bg-surface-secondary/60 p-2 font-mono text-xs whitespace-pre-wrap text-muted">
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
              <span className="text-sm text-muted">输入目标域名后点击「开始诊断」</span>
              <span className="text-xs text-muted/80">
                推荐对照：google.com（境外，预期直连失败、代理成功）与一个境内域名（预期直连成功）
              </span>
            </Card.Content>
          </Card>
        )}

        {running && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-10 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在分步诊断，弱网下最长约一分钟…</span>
            </Card.Content>
          </Card>
        )}
      </div>
    </div>
  );
}
