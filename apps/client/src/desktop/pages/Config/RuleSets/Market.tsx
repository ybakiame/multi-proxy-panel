import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Alert, Button, Card, Chip, Input, Label, ListBox, Select, Table } from "@heroui/react";
import { ArrowLeftIcon, SparklesIcon } from "@heroicons/react/24/outline";
import { META_CUBE_SOURCE, useRuleSetMarket } from "@pp/client-core";
import type { MarketCategoryFilter, MetaCubeEntry } from "@pp/client-core";

/** 常用检索词快捷 Chip（点击填充检索框）。 */
const POPULAR_KEYWORDS = ["cn", "ads", "google", "netflix", "youtube", "telegram"];

/** 分类过滤选项（`all` = 全部）。 */
const CATEGORY_OPTIONS: { id: MarketCategoryFilter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "geoip", label: "GeoIP（IP 段）" },
  { id: "geosite", label: "GeoSite（域名）" },
];

function groupLabel(entry: MetaCubeEntry): string {
  return entry.group === "ip" ? "GeoIP" : "GeoSite";
}

/**
 * 规则集市场页（桌面端，路由 `/config/route/rulesets/market`；语义对齐移动端
 * `Config/RuleSetMarket`）。
 *
 * 市场为 MetaCubeX/meta-rules-dat 固定源：检索纯本地（`metaCubeCatalog` 静态清单，
 * 无运行时 GitHub API 调用）；一键添加经共享视图模型 [`useRuleSetMarket`] 写入
 * `custom_rule_sets`（Remote + binary），随后在规则集管理页「立即更新」下载。
 */
export default function RuleSetMarket() {
  const navigate = useNavigate();
  const market = useRuleSetMarket();
  const [addedFlash, setAddedFlash] = useState<string | null>(null);

  const handleAdd = async (entry: MetaCubeEntry) => {
    await market.addEntry(entry);
    setAddedFlash(entry.id);
    window.setTimeout(() => setAddedFlash(null), 1500);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* 页头：返回 + 标题 */}
      <div className="flex items-center gap-3">
        <Button
          size="sm"
          variant="ghost"
          isIconOnly
          aria-label="返回规则集管理"
          onPress={() => navigate("/config/route/rulesets")}
        >
          <ArrowLeftIcon className="size-4" />
        </Button>
        <div>
          <h1 className="text-xl font-semibold">规则集市场</h1>
          <p className="text-sm text-muted">一键添加社区规则集（远程 URL），添加后在规则集管理「立即更新」下载</p>
        </div>
      </div>

      {market.error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{market.error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {/* 固定源说明卡 */}
      <Card>
        <Card.Content className="flex flex-col gap-2 p-4">
          <div className="flex items-center gap-2">
            <SparklesIcon className="size-5 shrink-0 text-accent" aria-hidden="true" />
            <span className="text-sm font-medium">{META_CUBE_SOURCE.name}</span>
          </div>
          <span className="text-xs text-muted">{META_CUBE_SOURCE.description}</span>
          <span className="text-xs text-muted">更新频率：{META_CUBE_SOURCE.updateFrequency}</span>
        </Card.Content>
      </Card>

      {/* 检索区 */}
      <div className="flex flex-col gap-3">
        <div className="flex items-end gap-3">
          <div className="flex flex-1 flex-col gap-1.5">
            <Label htmlFor="market-search">检索规则集</Label>
            <Input
              id="market-search"
              aria-label="检索规则集"
              value={market.keyword}
              onChange={(event) => market.setKeyword(event.target.value)}
              placeholder="输入关键词（如 cn / netflix / google）"
              fullWidth
            />
          </div>
          <div className="flex w-48 flex-col gap-1.5">
            <Label htmlFor="market-category">分类</Label>
            <Select
              id="market-category"
              aria-label="分类过滤"
              value={market.category}
              onChange={(key) => market.setCategory(String(key ?? "all") as MarketCategoryFilter)}
              fullWidth
            >
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {CATEGORY_OPTIONS.map((option) => (
                    <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                      {option.label}
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  ))}
                </ListBox>
              </Select.Popover>
            </Select>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted">常用：</span>
          {POPULAR_KEYWORDS.map((word) => (
            <Chip key={word} size="sm" variant="soft" color="default" onClick={() => market.setKeyword(word)}>
              {word}
            </Chip>
          ))}
        </div>
      </div>

      {/* 结果列表 */}
      {!market.ready && market.isLoading && (
        <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
          <span className="text-sm text-muted">正在加载市场…</span>
        </div>
      )}
      {market.ready && !market.hasKeyword && (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          输入关键词检索 MetaCubeX 规则集（本地静态清单检索，不请求 GitHub API）
        </div>
      )}
      {market.ready && market.hasKeyword && market.results.length === 0 && (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          无匹配结果，换个关键词试试
        </div>
      )}
      {market.ready && market.results.length > 0 && (
        <Table>
          <Table.ScrollContainer>
            <Table.Content aria-label="市场检索结果" className="min-w-[560px]">
              <Table.Header>
                <Table.Column isRowHeader>名称</Table.Column>
                <Table.Column>分类</Table.Column>
                <Table.Column>操作</Table.Column>
              </Table.Header>
              <Table.Body>
                {market.results.map((entry) => {
                  const added = market.isAdded(entry);
                  const adding = market.addingKey === entry.id;
                  return (
                    <Table.Row key={entry.id}>
                      <Table.Cell className="max-w-[280px] truncate">
                        <span title={entry.url}>{entry.name}</span>
                      </Table.Cell>
                      <Table.Cell>
                        <Chip size="sm" variant="soft" color="accent">
                          {groupLabel(entry)}
                        </Chip>
                      </Table.Cell>
                      <Table.Cell>
                        <Button
                          size="sm"
                          variant={added ? "secondary" : "primary"}
                          isDisabled={added || market.addingKey !== null}
                          isPending={adding}
                          onPress={() => void handleAdd(entry)}
                        >
                          {added ? "已添加" : addedFlash === entry.id ? "已添加" : "添加"}
                        </Button>
                      </Table.Cell>
                    </Table.Row>
                  );
                })}
              </Table.Body>
            </Table.Content>
          </Table.ScrollContainer>
        </Table>
      )}
    </div>
  );
}
