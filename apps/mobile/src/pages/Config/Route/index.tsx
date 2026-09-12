import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ListBulletIcon, SwatchIcon } from "@heroicons/react/24/outline";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  defaultConfigSlices,
  subscriptionNodeTags,
  subscriptionNodeTagsKey,
  toErrorMessage,
  toastError,
  toastSuccess,
  useClientConfig,
  useProxyStatus,
} from "@pp/client-core";
import type { ConfigSlices, DnsStrategy, NodeTagView, RouteSlice } from "@pp/client-core";
import { useNavigate } from "react-router-dom";
import { BackHeader } from "../../../components/BackHeader";
import { EntryLinkCard } from "../../../components/EntryLinkCard";
import { isConfigSlices } from "../Dns/dnsUtils";
import { RouteSettingsCard } from "./RouteSettingsCard";
import { buildFinalTagOptions, buildResolverServerOptions, isRouteSliceValid, validateRouteSlice } from "./routeUtils";

/**
 * 路由切片配置子页（ADR-0005，路由 `/config/route`）。
 *
 * 结构自上而下：BackHeader（右侧保存动作）→ 路由设置区
 * （`route.final` + `route.default_domain_resolver`）→ 入口区（规则管理 / 规则集管理）。
 * 无切片总开关：非空字段即覆写运行配置。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量 `ConfigSlices`；所有编辑只改内存中的
 * route 切片草稿（copy-on-write），点击保存才整份 `configSlicesSave` 落盘，成功后
 * invalidate + toast；核心运行中追加「重启代理后生效」。校验对齐 Rust
 * `RouteSlice::validate`：非空的 `final_tag` / `resolver.server` 不得含空白。
 */
export default function RoutePage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 切片在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;

  const {
    data: rawSlices,
    isLoading,
    error: queryError,
  } = useQuery<ConfigSlices>({
    queryKey: CONFIG_SLICES_KEY,
    queryFn: configSlicesGet,
    // 表单页草稿期间避免窗口聚焦触发的后台重取覆盖未保存编辑；保存后仍显式 invalidate。
    refetchOnWindowFocus: false,
  });

  // 结构守卫：异构/异常缓存视为未加载，渲染空态而非崩溃。
  const slices = isConfigSlices(rawSlices) ? rawSlices : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });

  // 订阅节点（静态源）与 FakeIP 开关：供 final / resolver 候选并集使用。
  const { data: config } = useClientConfig();
  const activeSubscriptionId = config?.active_subscription_id ?? null;
  const { data: subscriptionNodes } = useQuery<NodeTagView[]>({
    queryKey: subscriptionNodeTagsKey(activeSubscriptionId ?? ""),
    queryFn: () => subscriptionNodeTags(activeSubscriptionId ?? ""),
    enabled: !!activeSubscriptionId,
    retry: false,
  });

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  const [draft, setDraft] = useState<RouteSlice | null>(null);
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | undefined>(undefined);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    // 旧客户端可能缺 route 段：回退默认切片，保证草稿始终可编辑。
    setDraft(slices.route ?? defaultConfigSlices().route);
  }
  if (!slices && draft !== null) {
    setPrevSlices(undefined);
    setDraft(null);
  }

  const [saving, setSaving] = useState(false);

  const finalTagOptions = useMemo(() => buildFinalTagOptions(slices, subscriptionNodes), [slices, subscriptionNodes]);
  const resolverServerOptions = useMemo(
    () => buildResolverServerOptions(slices, config?.dns_fakeip_enabled ?? false),
    [slices, config?.dns_fakeip_enabled],
  );

  const errors = draft ? validateRouteSlice(draft) : null;
  const valid = errors !== null && isRouteSliceValid(errors);
  const dirty =
    draft !== null &&
    slices !== null &&
    JSON.stringify(draft) !== JSON.stringify(slices.route ?? defaultConfigSlices().route);

  const handleSave = async () => {
    if (!slices || !draft || !valid || !dirty || saving) return;
    setSaving(true);
    try {
      await configSlicesSave({ ...slices, route: draft });
      toastSuccess(coreRunning ? "路由配置已保存，重启代理后生效" : "路由配置已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  // ---- 标量字段（final / resolver） ----
  const handleChangeFinalTag = (final_tag: string) =>
    setDraft((current) => (current ? { ...current, final_tag } : current));
  const handleChangeResolverServer = (server: string) =>
    setDraft((current) => (current ? { ...current, resolver: { ...current.resolver, server } } : current));
  const handleChangeResolverStrategy = (strategy: DnsStrategy | null) =>
    setDraft((current) => (current ? { ...current, resolver: { ...current.resolver, strategy } } : current));

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader
        title="路由"
        action={
          <Button
            variant="primary"
            className="h-11 shrink-0 px-4"
            isDisabled={!valid || !dirty || saving}
            isPending={saving}
            onPress={() => void handleSave()}
          >
            保存
          </Button>
        }
      />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {isLoading && !slices && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载路由配置…</span>
            </Card.Content>
          </Card>
        )}

        {!isLoading && queryError && (
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

        {!isLoading && !queryError && !draft && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">路由配置不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={invalidate}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {draft && errors && (
          <>
            <RouteSettingsCard
              finalTag={draft.final_tag}
              resolverServer={draft.resolver.server}
              resolverStrategy={draft.resolver.strategy}
              finalTagOptions={finalTagOptions}
              resolverServerOptions={resolverServerOptions}
              finalTagError={errors.finalTag}
              resolverServerError={errors.resolverServer}
              onChangeFinalTag={handleChangeFinalTag}
              onChangeResolverServer={handleChangeResolverServer}
              onChangeResolverStrategy={handleChangeResolverStrategy}
            />

            <EntryLinkCard
              icon={<ListBulletIcon className="size-6" aria-hidden="true" />}
              title="规则管理"
              description="添加与管理你的分流规则"
              onPress={() => navigate("/config/route/rules")}
            />
            <EntryLinkCard
              icon={<SwatchIcon className="size-6" aria-hidden="true" />}
              title="规则集管理"
              description="社区与自定义规则集的增删与更新"
              onPress={() => navigate("/config/route/rulesets")}
            />
          </>
        )}
      </div>
    </div>
  );
}
