import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner, Switch } from "@heroui/react";
import {
  CONFIG_KEY,
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  toErrorMessage,
  toastError,
  toastSuccess,
  useClientConfig,
  useProxyStatus,
  useSaveConfig,
} from "@pp/client-core";
import type { CacheFileSlice, ClientConfig, ConfigSlices, ExperimentalSlice } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { useSettingsConfig, randomClashApiSecret } from "../Settings/useSettingsConfig";
import { ClashApiCard } from "./ClashApiCard";
import { isConfigSlices } from "./Dns/dnsUtils";

/** 文本输入样式（对齐 DnsServerFormSheet / OutboundField 的 inputClass）。 */
const INPUT_CLASS =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/**
 * 实验性配置子页（ADR-0005 P2-E3b，路由 `/config/experimental`；2026-09 起并入
 * Clash API 设置，取代独立 `/config/clash-api` 页；原名 Experimental 页）。
 *
 * 结构自上而下：BackHeader（右侧保存动作，仅作用于 Cache File 切片草稿）→
 * 实验性能力提示（置顶）→ Clash API 卡（端口 / 必填密钥，即时保存，ClientConfig 存储，
 * 不经页面保存按钮）→ Cache File 分区（enabled / path / cache_id / store_fakeip）。
 * 无切片总开关：`cache_file.enabled` 即注入开关。
 *
 * 存量纠正（进入本页一次性静默落盘）：`clash_api_enabled: false`（历史关闭开关）纠正为
 * true；`clash_api_secret` 为空（密钥必填前的存量）自动生成随机密钥填充。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量 `ConfigSlices`；编辑只改内存中的
 * experimental 切片草稿（copy-on-write），点击保存才整份 `configSlicesSave` 落盘，
 * 成功后 invalidate + toast；核心运行中追加「重启代理后生效」。校验对齐 Rust
 * `ExperimentalSlice::validate`：`path` 非空时不得为纯空白。
 */
export default function ExperimentalPage() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 切片在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  // Clash API 卡（ClientConfig 即时保存体系，与本页切片草稿互不干扰）。
  const settings = useSettingsConfig();
  const { data: config } = useClientConfig();
  const { mutateAsync: saveConfigAsync } = useSaveConfig();

  // 必选切片恒启用 + 密钥必填：仅本页负责纠正存量（一次性，失败下次进入重试）。
  // 读缓存最新基底叠加补丁（对齐 useSettingsConfig.persist），避免展开渲染期快照覆盖
  // 并发保存（lost update）。
  const correctedRef = useRef(false);
  useEffect(() => {
    if (correctedRef.current || !config) {
      return;
    }
    const needEnable = !config.clash_api_enabled;
    const needSecret = (config.clash_api_secret ?? "").trim() === "";
    if (!needEnable && !needSecret) {
      return;
    }
    const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
    if (!current) {
      return;
    }
    correctedRef.current = true;
    void saveConfigAsync({
      ...current,
      clash_api_enabled: true,
      clash_api_secret: needSecret ? randomClashApiSecret() : current.clash_api_secret,
    }).catch(() => {
      // 静默纠正失败：保留原值不阻塞页面，重置标记以便下次进入重试。
      correctedRef.current = false;
    });
  }, [config, queryClient, saveConfigAsync]);

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

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  const [draft, setDraft] = useState<ExperimentalSlice | null>(null);
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | undefined>(undefined);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    setDraft(slices.experimental);
  }
  if (!slices && draft !== null) {
    setPrevSlices(undefined);
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
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader
        title="实验性配置"
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
              <span className="text-sm text-muted">正在加载 Experimental 配置…</span>
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
              <span className="text-sm text-muted">Experimental 配置不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={invalidate}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Description>实验性功能，可能会存在不稳定现象。</Alert.Description>
          </Alert.Content>
        </Alert>

        {/* Clash API（ClientConfig 即时保存，不经本页保存按钮） */}
        <ClashApiCard settings={settings} />

        {draft && (
          <>
            <Card>
              <Card.Header>
                <Card.Title>Cache File</Card.Title>
                <Card.Description>持久化缓存文件（experimental.cache_file）</Card.Description>
              </Card.Header>
              <Card.Content className="flex flex-col gap-4">
                <ToggleRow
                  label="启用缓存文件"
                  ariaLabel="启用缓存文件"
                  isSelected={draft.cache_file.enabled}
                  onChange={(next) => updateCacheFile({ enabled: next })}
                />
                <Field
                  id="experimental-cache-path"
                  label="路径"
                  value={draft.cache_file.path}
                  onChange={(path) => updateCacheFile({ path })}
                  placeholder="默认 cache.db"
                  error={pathError}
                />
                <Field
                  id="experimental-cache-id"
                  label="Cache ID"
                  value={draft.cache_file.cache_id}
                  onChange={(cache_id) => updateCacheFile({ cache_id })}
                  placeholder="留空则不使用独立 store"
                />
                <ToggleRow
                  label="缓存 fakeip 映射"
                  ariaLabel="缓存 fakeip 映射"
                  description="缓存 fakeip 映射，重启后保留"
                  isSelected={draft.cache_file.store_fakeip}
                  onChange={(next) => updateCacheFile({ store_fakeip: next })}
                />
              </Card.Content>
            </Card>
          </>
        )}
      </div>
    </div>
  );
}

interface FieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  error?: string | null;
}

/** 带标签 / 行内错误的文本输入行（错误优先于占位提示）。 */
function Field({ id, label, value, onChange, placeholder, error }: FieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-sm font-medium text-foreground">
        {label}
      </label>
      <input
        id={id}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        className={INPUT_CLASS}
      />
      {error ? <span className="text-xs text-warning">{error}</span> : null}
    </div>
  );
}

interface ToggleRowProps {
  label: string;
  ariaLabel: string;
  description?: string;
  isSelected: boolean;
  onChange: (next: boolean) => void;
}

/** 左标签（+可选说明）+ 右开关的整行开关（触达区 ≥44px）。 */
function ToggleRow({ label, ariaLabel, description, isSelected, onChange }: ToggleRowProps) {
  return (
    <div className="flex min-h-11 items-center justify-between gap-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-sm text-foreground">{label}</span>
        {description && <span className="text-xs text-muted">{description}</span>}
      </div>
      <Switch aria-label={ariaLabel} isSelected={isSelected} onChange={onChange} className="shrink-0">
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
    </div>
  );
}
