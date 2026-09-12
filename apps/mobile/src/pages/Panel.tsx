import { useNavigate } from "react-router-dom";
import { Button, Card } from "@heroui/react";
import { toastError, useClientConfig, useProxyStatus } from "@pp/client-core";
import { BackHeader } from "../components/BackHeader";

/**
 * 组装内嵌面板 iframe URL：密钥同时放进 search（yacd 风格 `?hostname&port&secret`）与
 * hash setup 段（zashboard / metacubexd 风格 `#/setup?...`），三种可选面板 UI 都能自动
 * 读取，无需用户在面板内重复输入密钥。
 */
export function buildPanelUrl(clashApiUrl: string, secret: string): string {
  const parsed = new URL(clashApiUrl);
  const params = `hostname=${encodeURIComponent(parsed.hostname)}&port=${encodeURIComponent(parsed.port)}&secret=${encodeURIComponent(secret)}`;
  return `${parsed.origin}/ui/?${params}#/setup?${params}`;
}

/**
 * Clash 面板页（路由 `/panel`，不在 TabBar）。
 *
 * 内嵌内核自带的 Clash 面板（zashboard）：sing-box 的 `external_ui` 目录经 Clash API 的
 * `/ui` 路径提供（见 `pp-client` core_config 注释「URL path remains `/ui`」）。
 *
 * - 跳转携带密钥（2026-09 起密钥必填）：iframe src 由 [`buildPanelUrl`] 组装，密钥同时放
 *   search 与 hash setup 段，三种面板 UI 自动完成鉴权；密钥为空（存量未纠正）时不渲染
 *   iframe，提示并引导到 配置 → Experimental 设置；
 * - 核心运行中且 Clash API 就绪：全高度 iframe，占满 BackHeader 以下的视口高度（二级页无
 *   TabBar，底部留 `env(safe-area-inset-bottom)` 说明小字）；
 * - 未运行 / API 未就绪：空态提示「启动代理后可使用面板」；
 * - iframe 跨域加载失败无法可靠检测，不做超时错误态，保持简单。
 */
export default function Panel() {
  const navigate = useNavigate();
  const { data: status } = useProxyStatus();
  const { data: config } = useClientConfig();
  const running = status?.core_running ?? false;
  const clashApiUrl = status?.clash_api_url ?? null;
  const secret = (config?.clash_api_secret ?? "").trim();
  const available = running && clashApiUrl !== null && clashApiUrl !== "";
  // 密钥必填：为空不允许进入面板（引导到 Experimental 页设置/生成）。
  const missingSecret = available && secret === "";

  return (
    <div className="flex h-full min-h-0 flex-col">
      <BackHeader title="面板" />
      {available && !missingSecret ? (
        <>
          <iframe
            title="Clash 面板"
            src={buildPanelUrl(clashApiUrl, secret)}
            className="min-h-0 w-full flex-1 border-0 bg-background"
          />
          <p className="shrink-0 px-4 pt-1.5 pb-[max(0.5rem,env(safe-area-inset-bottom))] text-center text-xs text-muted">
            面板由内核内置服务提供，已自动携带密钥鉴权
          </p>
        </>
      ) : (
        <div
          className="flex min-h-0 flex-1 items-center justify-center p-6"
          style={{
            paddingBottom: "max(1.5rem, env(safe-area-inset-bottom))",
            paddingLeft: "max(1rem, env(safe-area-inset-left))",
            paddingRight: "max(1rem, env(safe-area-inset-right))",
          }}
        >
          <Card className="w-full">
            <Card.Content className="flex flex-col items-center justify-center gap-2 py-12 text-center">
              {missingSecret ? (
                <>
                  <span className="text-sm text-muted">尚未设置 Clash API 密钥</span>
                  <span className="text-xs text-muted/80">面板访问需携带密钥鉴权，请先设置或随机生成</span>
                  <Button
                    variant="primary"
                    className="mt-2 min-h-11 px-4"
                    onPress={() => {
                      toastError("请先设置 Clash API 密钥");
                      navigate("/config/experimental");
                    }}
                  >
                    去设置密钥
                  </Button>
                </>
              ) : (
                <>
                  <span className="text-sm text-muted">{running ? "面板暂不可用" : "启动代理后可使用面板"}</span>
                  <span className="text-xs text-muted/80">
                    {running ? "Clash API 未就绪，请检查设置" : "面板依赖内核内置的 Clash API，运行中自动加载"}
                  </span>
                </>
              )}
            </Card.Content>
          </Card>
        </div>
      )}
    </div>
  );
}
