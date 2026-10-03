import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Alert, Button, Card, Label, ListBox, Select } from "@heroui/react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import {
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  defaultConfigSlices,
  markRestartRequired,
  subscriptionNodeTags,
  subscriptionNodeTagsKey,
  toastError,
  toastSuccess,
  toErrorMessage,
  useClientConfig,
  useProxyStatus,
} from "@pp/client-core";
import type { ConfigSlices, DnsStrategy, NodeTagView, RouteSlice } from "@pp/client-core";
import {
  ROUTE_STRATEGY_OPTIONS,
  buildFinalTagOptions,
  buildResolverServerOptions,
  isConfigSlices,
  isRouteSliceValid,
  validateRouteSlice,
} from "@pp/client-core";

/**
 * 路由切片配置页（桌面端，路由 `/config/route`；语义对齐移动端 `Config/Route/index.tsx`）。
 *
 * 编辑 `route.final`（默认出站）与 `route.default_domain_resolver`（出站域名解析器）。
 * 无切片总开关：非空字段即覆写运行配置。规则管理与规则集管理在桌面端由侧边栏
 * 「规则」页承载（此处不重复入口）。
 *
 * 数据流：同 DNS 页（内存草稿 copy-on-write，保存整份落盘 + invalidate + toast +
 * 上报重启脏标记）。校验对齐 Rust `RouteSlice::validate`：非空的 `final_tag` /
 * `resolver.server` 不得含空白。
 */
export default function RoutePage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const coreRunning = status?.core_running ?? false;

  const {
    data: rawSlices,
    isLoading,
    error: queryError,
  } = useQuery<ConfigSlices>({
    queryKey: CONFIG_SLICES_KEY,
    queryFn: configSlicesGet,
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
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | null>(null);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    // 旧客户端可能缺 route 段：回退默认切片，保证草稿始终可编辑。
    setDraft(slices.route ?? defaultConfigSlices().route);
  }
  if (!slices && draft !== null) {
    setPrevSlices(null);
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
      markRestartRequired("route", coreRunning);
      toastSuccess(coreRunning ? "路由配置已保存，重启代理后生效" : "路由配置已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const handleChangeFinalTag = (final_tag: string) =>
    setDraft((current) => (current ? { ...current, final_tag } : current));
  const handleChangeResolverServer = (server: string) =>
    setDraft((current) => (current ? { ...current, resolver: { ...current.resolver, server } } : current));
  const handleChangeResolverStrategy = (strategy: DnsStrategy | null) =>
    setDraft((current) => (current ? { ...current, resolver: { ...current.resolver, strategy } } : current));

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      {/* 页头：返回 + 标题 + 保存 */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" isIconOnly aria-label="返回配置管理" onPress={() => navigate("/config")}>
            <ArrowLeftIcon className="size-4" />
          </Button>
          <div>
            <h1 className="text-xl font-semibold">路由管理</h1>
            <p className="text-sm text-muted">默认出站与默认域名解析器</p>
          </div>
        </div>
        <Button
          size="sm"
          variant="primary"
          isDisabled={!valid || !dirty || saving}
          isPending={saving}
          onPress={() => void handleSave()}
        >
          {dirty ? "保存" : "已保存"}
        </Button>
      </div>

      {isLoading && !slices && (
        <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
          <span className="text-sm text-muted">正在加载路由配置…</span>
        </div>
      )}

      {!isLoading && queryError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{toErrorMessage(queryError)}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {!isLoading && !queryError && !draft && (
        <div className="flex flex-col items-center gap-3 rounded-lg border border-border/60 bg-surface p-6 text-center">
          <span className="text-sm text-muted">路由配置不可用</span>
          <Button size="sm" variant="secondary" onPress={invalidate}>
            重新加载
          </Button>
        </div>
      )}

      {draft && errors && (
        <Card>
          <Card.Header>
            <Card.Title>路由设置</Card.Title>
            <Card.Description>默认出站与出站域名解析器</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="route-final-tag">默认出站</Label>
              <Select
                id="route-final-tag"
                aria-label="默认出站"
                value={draft.final_tag}
                onChange={(key) => handleChangeFinalTag(String(key ?? ""))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {finalTagOptions.map((option) => (
                      <ListBox.Item key={option.value || "__default__"} id={option.value} textValue={option.label}>
                        {option.label}
                        {option.description ? (
                          <span className="text-xs text-muted">（{option.description}）</span>
                        ) : null}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
              {errors.finalTag ? (
                <span className="text-xs text-amber-500">{errors.finalTag}</span>
              ) : (
                <span className="text-xs text-muted">未匹配任何规则时使用的兜底出站</span>
              )}
            </div>

            <div className="flex flex-col gap-1.5">
              <Label htmlFor="route-resolver-server">默认域名解析服务器</Label>
              <Select
                id="route-resolver-server"
                aria-label="默认域名解析服务器"
                value={draft.resolver.server}
                onChange={(key) => handleChangeResolverServer(String(key ?? ""))}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {resolverServerOptions.map((option) => (
                      <ListBox.Item key={option.value || "__default__"} id={option.value} textValue={option.label}>
                        {option.label}
                        {option.description ? (
                          <span className="text-xs text-muted">（{option.description}）</span>
                        ) : null}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
              {errors.resolverServer ? (
                <span className="text-xs text-amber-500">{errors.resolverServer}</span>
              ) : (
                <span className="text-xs text-muted">出站域名使用的 DNS 服务器</span>
              )}
            </div>

            <div className="flex flex-col gap-1.5">
              <Label htmlFor="route-resolver-strategy">解析策略</Label>
              <Select
                id="route-resolver-strategy"
                aria-label="解析策略"
                value={draft.resolver.strategy ?? ""}
                onChange={(key) => {
                  const value = String(key ?? "");
                  handleChangeResolverStrategy(value === "" ? null : (value as DnsStrategy));
                }}
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {ROUTE_STRATEGY_OPTIONS.map((option) => (
                      <ListBox.Item key={option.value || "__follow__"} id={option.value} textValue={option.label}>
                        {option.label}
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
              <span className="text-xs text-muted">出站域名解析策略</span>
            </div>
          </Card.Content>
        </Card>
      )}
    </div>
  );
}
