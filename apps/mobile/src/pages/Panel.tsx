import { Card } from "@heroui/react";
import { useProxyStatus } from "@pp/client-core";
import { BackHeader } from "../components/BackHeader";

/**
 * Clash 面板页（路由 `/panel`，不在 TabBar）。
 *
 * 内嵌内核自带的 Clash 面板（zashboard）：sing-box 的 `external_ui` 目录经 Clash API 的
 * `/ui` 路径提供（见 `pp-client` core_config 注释「URL path remains `/ui`」），因此
 * `src = <clash_api_url>/ui`；`clash_api_url` 形如 `http://127.0.0.1:<port>`（无路径）。
 *
 * - 核心运行中且 Clash API 就绪：全高度 iframe，占满 BackHeader 以下的视口高度（二级页无
 *   TabBar，底部留 `env(safe-area-inset-bottom)` 与密钥说明小字）；
 * - 未运行 / API 未就绪：空态提示「启动代理后可使用面板」；
 * - secret 不透传：zashboard 从同源 `/ui` 服务时自动识别地址，用户若配置了密钥会在其自身
 *   UI 内提示输入，本页仅在底部以一行小字说明；
 * - iframe 跨域加载失败无法可靠检测，不做超时错误态，保持简单。
 */
export default function Panel() {
  const { data: status } = useProxyStatus();
  const running = status?.core_running ?? false;
  const clashApiUrl = status?.clash_api_url ?? null;
  const available = running && clashApiUrl !== null && clashApiUrl !== "";

  return (
    <div className="flex h-full min-h-0 flex-col">
      <BackHeader title="面板" />
      {available ? (
        <>
          <iframe
            title="Clash 面板"
            src={`${clashApiUrl}/ui`}
            className="min-h-0 w-full flex-1 border-0 bg-background"
          />
          <p className="shrink-0 px-4 pt-1.5 pb-[max(0.5rem,env(safe-area-inset-bottom))] text-center text-xs text-muted">
            面板由内核内置服务提供；若提示需要密钥，请填写设置中的 Clash API 密钥
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
              <span className="text-sm text-muted">{running ? "面板暂不可用" : "启动代理后可使用面板"}</span>
              <span className="text-xs text-muted/80">
                {running ? "Clash API 未就绪，请检查设置" : "面板依赖内核内置的 Clash API，运行中自动加载"}
              </span>
            </Card.Content>
          </Card>
        </div>
      )}
    </div>
  );
}
