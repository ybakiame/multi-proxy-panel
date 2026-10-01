import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Alert, Button, Card, Input, Label, Switch } from "@heroui/react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import {
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  markRestartRequired,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
} from "@pp/client-core";
import type { CacheFileSlice, ConfigSlices, ExperimentalSlice } from "@pp/client-core";
import { isConfigSlices } from "@pp/client-core";

/**
 * 实验性配置页（桌面端，路由 `/config/experimental`；语义对齐移动端
 * `Config/ExperimentalPage.tsx`，桌面端 Clash API 由「设置 → Clash 面板」承载，
 * 本页仅编辑 `experimental.cache_file` 切片）。
 *
 * 数据流：同 DNS 页（内存草稿 copy-on-write，保存整份落盘 + invalidate + toast +
 * 上报重启脏标记）。校验对齐 Rust `ExperimentalSlice::validate`：`path` 非空时不得
 * 为纯空白。
 */
export default function ExperimentalPage() {
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

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  const [draft, setDraft] = useState<ExperimentalSlice | null>(null);
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | null>(null);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    setDraft(slices.experimental);
  }
  if (!slices && draft !== null) {
    setPrevSlices(null);
    setDraft(null);
  }

  const [saving, setSaving] = useState(false);

  // 校验对齐 Rust：非空 path 不得为纯空白。
  const pathError =
    draft && draft.cache_file.path !== "" && draft.cache_file.path.trim() === "" ? "路径不能为空白字符" : null;
  const valid = draft !== null && pathError === null;
  const dirty = draft !== null && slices !== null && JSON.stringify(draft) !== JSON.stringify(slices.experimental);

  const handleSave = async () => {
    if (!slices || !draft || !valid || !dirty || saving) return;
    setSaving(true);
    try {
      await configSlicesSave({ ...slices, experimental: draft });
      markRestartRequired("experimental", coreRunning);
      toastSuccess(coreRunning ? "Experimental 配置已保存，重启代理后生效" : "Experimental 配置已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const updateCacheFile = (patch: Partial<CacheFileSlice>) =>
    setDraft((current) => (current ? { ...current, cache_file: { ...current.cache_file, ...patch } } : current));

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      {/* 页头：返回 + 标题 + 保存 */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" isIconOnly aria-label="返回配置管理" onPress={() => navigate("/config")}>
            <ArrowLeftIcon className="size-4" />
          </Button>
          <div>
            <h1 className="text-xl font-semibold">Experimental</h1>
            <p className="text-sm text-muted">实验性配置（cache_file 缓存）</p>
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
          <span className="text-sm text-muted">正在加载 Experimental 配置…</span>
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
          <span className="text-sm text-muted">Experimental 配置不可用</span>
          <Button size="sm" variant="secondary" onPress={invalidate}>
            重新加载
          </Button>
        </div>
      )}

      <Alert status="default">
        <Alert.Indicator />
        <Alert.Content>
          <Alert.Description>实验性功能，可能会存在不稳定现象。</Alert.Description>
        </Alert.Content>
      </Alert>

      {draft && (
        <Card>
          <Card.Header>
            <Card.Title>Cache File</Card.Title>
            <Card.Description>持久化缓存文件</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-4">
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm text-foreground">启用缓存文件</span>
              <Switch
                aria-label="启用缓存文件"
                isSelected={draft.cache_file.enabled}
                onChange={(next) => updateCacheFile({ enabled: next })}
              >
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                </Switch.Content>
              </Switch>
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="experimental-cache-path">路径</Label>
              <Input
                id="experimental-cache-path"
                aria-label="缓存文件路径"
                value={draft.cache_file.path}
                onChange={(event) => updateCacheFile({ path: event.target.value })}
                placeholder="默认 cache.db"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className="font-mono"
              />
              {pathError && <span className="text-xs text-amber-500">{pathError}</span>}
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="experimental-cache-id">Cache ID</Label>
              <Input
                id="experimental-cache-id"
                aria-label="Cache ID"
                value={draft.cache_file.cache_id}
                onChange={(event) => updateCacheFile({ cache_id: event.target.value })}
                placeholder="留空则不使用独立 store"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className="font-mono"
              />
            </div>
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="text-sm text-foreground">缓存 fakeip 映射</span>
                <span className="text-xs text-muted">缓存 fakeip 映射，重启后保留</span>
              </div>
              <Switch
                aria-label="缓存 fakeip 映射"
                isSelected={draft.cache_file.store_fakeip}
                onChange={(next) => updateCacheFile({ store_fakeip: next })}
              >
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                </Switch.Content>
              </Switch>
            </div>
          </Card.Content>
        </Card>
      )}
    </div>
  );
}
