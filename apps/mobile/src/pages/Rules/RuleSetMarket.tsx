import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Alert, Button, Card, Chip, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  RULESET_MARKET,
  buildSaveInput,
  localOverrideGet,
  localOverrideSave,
  toErrorMessage,
  toastError,
  toastSuccess,
} from "@pp/client-core";
import type { CustomRuleSetInput, LocalOverrideView, RuleSetMarketEntry } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";

/** 分类筛选中「全部」的哨兵值。 */
const ALL_CATEGORY = "全部";

/** 条目格式 chip 文案：binary = srs 二进制，source = json 源码。 */
function formatLabel(format: RuleSetMarketEntry["format"]): string {
  return format === "binary" ? "srs" : "json";
}

/**
 * 规则集市场页（路由 `/rules/rulesets/market`）。
 *
 * 第一版为**静态内置精选目录**（`RULESET_MARKET`，数据源 DustinWin
 * `sing-box-ruleset` tag，sing-box 1.14+ / version 5 兼容）：
 * - 顶部分类水平滚动 Chip 筛选；
 * - 条目卡展示中文名 / 描述 / 分类 chip / 格式 chip；
 * - 一键添加：写入 `custom_rule_sets`（Remote + `.srs`）后 toast 提示
 *   「已添加，点击立即更新下载」并 invalidate；下载仍走现有「立即更新」链路
 *   （含 `github_proxy_prefix` 代理）；
 * - 已添加判定：`custom_rule_sets` 中存在**相同 URL** 的 Remote 条目 → 卡片
 *   显示「已添加」禁用态；tag 冲突（同 tag 不同 URL）由 Rust 校验兜底，保存
 *   失败 toast 报错并保留现场。
 */
export default function RuleSetMarket() {
  const queryClient = useQueryClient();
  const {
    data: rawOverride,
    isLoading,
    error,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  // 结构守卫：缓存残留异构形态时视为未加载，渲染加载/空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const customSets = asArray(overrideData?.custom_rule_sets);
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [category, setCategory] = useState(ALL_CATEGORY);
  const [addingId, setAddingId] = useState<string | null>(null);

  const categories = useMemo(
    () => [ALL_CATEGORY, ...Array.from(new Set(RULESET_MARKET.map((item) => item.category)))],
    [],
  );

  // 已添加判定：按 URL 匹配（同 tag 不同 URL 由 Rust 校验报冲突）。
  const addedUrls = useMemo(
    () => new Set(customSets.flatMap((rs) => (rs.source.kind === "remote" ? [rs.source.url] : []))),
    [customSets],
  );

  const visibleEntries = useMemo(
    () => (category === ALL_CATEGORY ? RULESET_MARKET : RULESET_MARKET.filter((item) => item.category === category)),
    [category],
  );

  /** 一键添加：custom 段整段替换追加（Remote + srs）；tag 冲突由 Rust 兜底报错。 */
  const handleAdd = async (target: RuleSetMarketEntry) => {
    if (!overrideData || addingId !== null || addedUrls.has(target.url)) return;
    setAddingId(target.id);
    const input: CustomRuleSetInput = {
      id: crypto.randomUUID(),
      name: target.name,
      tag: target.id,
      source: { kind: "remote", url: target.url, format: target.format },
      // 新条目尚未下载，last_updated 归零，待「立即更新」。
      last_updated: 0,
    };
    const base = buildSaveInput(overrideData);
    try {
      await localOverrideSave({ ...base, custom_rule_sets: [...base.custom_rule_sets, input] });
      toastSuccess("已添加，点击立即更新下载");
      invalidate();
    } catch (err) {
      // 保存失败（如 tag 冲突）保留现场，仅报错。
      toastError(toErrorMessage(err));
      invalidate();
    } finally {
      setAddingId(null);
    }
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="规则集市场" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <span className="text-xs text-muted">
          精选 DustinWin 规则集（sing-box 1.14+，srs 二进制）；添加后点击「立即更新」下载，下载成功前不会注入
        </span>

        {isLoading && !overrideData && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载规则集…</span>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !isLoading && error && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>加载失败</Alert.Title>
                  <Alert.Description>{toErrorMessage(error)}</Alert.Description>
                </Alert.Content>
              </Alert>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !isLoading && !error && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">规则集数据不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {overrideData && (
          <>
            {/* 分类筛选：水平滚动 Chip */}
            <div className="-mx-1 flex items-center gap-2 overflow-x-auto px-1 pb-0.5">
              {categories.map((item) => {
                const active = item === category;
                return (
                  <button
                    key={item}
                    type="button"
                    onClick={() => setCategory(item)}
                    aria-pressed={active}
                    className={`min-h-11 shrink-0 rounded-full border px-4 text-sm font-medium transition-colors ${
                      active
                        ? "border-primary/60 bg-primary/10 text-primary"
                        : "border-border/60 bg-surface text-foreground active:opacity-70"
                    }`}
                  >
                    {item}
                  </button>
                );
              })}
            </div>

            {/* 条目卡列表 */}
            <div className="flex flex-col gap-2">
              {visibleEntries.map((item) => {
                const added = addedUrls.has(item.url);
                const pending = addingId === item.id;
                return (
                  <Card key={item.id}>
                    <Card.Content className="flex flex-col gap-3 p-3">
                      <div className="flex min-w-0 flex-col gap-0.5">
                        <span className="truncate text-sm font-medium text-foreground">{item.name}</span>
                        <span className="text-xs text-muted">{item.description}</span>
                      </div>
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex min-w-0 items-center gap-1.5">
                          <Chip size="sm" variant="soft" color="default" className="shrink-0">
                            {item.category}
                          </Chip>
                          <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                            {formatLabel(item.format)}
                          </Chip>
                        </div>
                        {added ? (
                          <Button variant="tertiary" isDisabled className="min-h-11 shrink-0 px-3">
                            <CheckIcon className="size-4" aria-hidden="true" />
                            已添加
                          </Button>
                        ) : (
                          <Button
                            variant="primary"
                            className="min-h-11 shrink-0 px-3"
                            isDisabled={addingId !== null}
                            isPending={pending}
                            onPress={() => void handleAdd(item)}
                          >
                            {!pending && <PlusIcon className="size-4" aria-hidden="true" />}
                            添加
                          </Button>
                        )}
                      </div>
                    </Card.Content>
                  </Card>
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
