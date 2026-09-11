import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowsRightLeftIcon,
  BeakerIcon,
  BoltIcon,
  GlobeAltIcon,
  ListBulletIcon,
  SwatchIcon,
  WifiIcon,
} from "@heroicons/react/24/outline";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideGet,
  localOverrideSave,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
  viewToInput,
} from "@pp/client-core";
import type { LocalOverrideView } from "@pp/client-core";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";
import { isLocalOverrideView } from "./localOverrideGuards";
import { MasterSwitchCard } from "./MasterSwitchCard";

/**
 * 配置管理入口页（ADR-0005 §3.3，路由 `/config`，Tab 2）。
 *
 * 自上而下：
 * 1. 总开关卡：`singbox.enabled`（关闭后本地规则与规则集不注入运行配置）；
 * 2. 配置切片入口：DNS（`/config/dns`）、自定义出站（`/config/outbounds`）、
 *    网络（TUN，`/config/network`）、Clash API（`/config/clash-api`）、
 *    Experimental（`/config/experimental`）；
 * 3. 规则入口：自定义规则（`/config/rules`）、规则集管理（`/config/rulesets`）。
 *
 * 网络（TUN）与 Clash API 为必选切片（ADR-0005 后续决策）：始终启用、无切片级关闭开关，
 * 入口仅提供参数调整（混合端口 / API 端口与密钥）。
 *
 * 规则列表 CRUD 与规则集管理已迁至对应子页；本页只消费 `singbox` 桶与
 * `custom_rule_sets` 段（经 `buildSaveInput` 整段透传）。
 */
export default function Config() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 本地规则 / 规则集在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const {
    data: rawOverride,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  // 结构守卫（见 localOverrideGuards.ts）：缓存中残留异构形态（如历史版本规则集
  // 管理页写入的 { override, ruleSets }）一律视为未加载 → 渲染可恢复的空态，避免
  // 访问 undefined 字段导致整页崩溃黑屏。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** 总开关写操作：全量 patch（buildSaveInput + localOverrideSave），失败 toast。 */
  const handleToggleEnabled = async (next: boolean) => {
    if (!overrideData || !currentCore) return;
    try {
      await localOverrideSave(buildSaveInput(overrideData, { ...viewToInput(currentCore), enabled: next }));
      toastRuleSaved(next ? "本地规则已启用" : "本地规则已关闭");
      invalidate();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">配置管理</h1>
        <p className="text-sm text-muted">DNS · 出站 · 规则 · 规则集</p>
      </div>

      {queryError && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
            <Alert status="danger">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>加载失败</Alert.Title>
                <Alert.Description>{toErrorMessage(queryError)}</Alert.Description>
              </Alert.Content>
            </Alert>
          </Card.Content>
        </Card>
      )}

      {isLoading && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-muted">正在加载规则配置…</span>
          </Card.Content>
        </Card>
      )}

      {!isLoading && !queryError && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
            <span className="text-sm text-muted">规则数据不可用</span>
            <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
              重新加载
            </Button>
          </Card.Content>
        </Card>
      )}

      {overrideData && currentCore && (
        <div className="flex flex-col gap-4">
          {/* 1. 总开关 */}
          <MasterSwitchCard enabled={currentCore.enabled} onToggle={(next) => void handleToggleEnabled(next)} />

          {/* 2. 配置切片入口：DNS → 自定义出站 → 网络（TUN） */}
          <EntryLinkCard
            icon={<GlobeAltIcon className="size-6" aria-hidden="true" />}
            title="DNS"
            description="DNS 服务器与分流规则配置"
            onPress={() => navigate("/config/dns")}
          />
          <EntryLinkCard
            icon={<ArrowsRightLeftIcon className="size-6" aria-hidden="true" />}
            title="自定义出站"
            description="可视化添加与管理自定义出站"
            onPress={() => navigate("/config/outbounds")}
          />
          <EntryLinkCard
            icon={<WifiIcon className="size-6" aria-hidden="true" />}
            title="网络（TUN）"
            description="TUN 始终启用 · 调整本地混合端口"
            onPress={() => navigate("/config/network")}
          />

          {/* 3. 规则入口：自定义规则 → 规则集管理 → Clash API */}
          <EntryLinkCard
            icon={<ListBulletIcon className="size-6" aria-hidden="true" />}
            title="自定义规则"
            description="添加与管理你的分流规则"
            onPress={() => navigate("/config/rules")}
          />
          <EntryLinkCard
            icon={<SwatchIcon className="size-6" aria-hidden="true" />}
            title="规则集管理"
            description="社区与自定义规则集的增删与更新"
            onPress={() => navigate("/config/rulesets")}
          />
          <EntryLinkCard
            icon={<BoltIcon className="size-6" aria-hidden="true" />}
            title="Clash API"
            description="仪表盘与节点页数据源 · 调整端口与密钥"
            onPress={() => navigate("/config/clash-api")}
          />
          <EntryLinkCard
            icon={<BeakerIcon className="size-6" aria-hidden="true" />}
            title="Experimental"
            description="实验性缓存与 fakeip 持久化配置"
            onPress={() => navigate("/config/experimental")}
          />
        </div>
      )}
    </PageShell>
  );
}
