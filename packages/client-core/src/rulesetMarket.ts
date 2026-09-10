/**
 * 规则集市场内置目录（Desktop / Mobile 双端共享）。
 *
 * 第一版为**静态内置精选目录**：数据源固定为 DustinWin/ruleset_geodata 的
 * `sing-box-ruleset` tag（固定 tag → 稳定 URL；规则集 `version: 5`，兼容
 * sing-box 1.14.0+）。精选条目均为 `.srs` 二进制规则集（`format: "binary"`）。
 *
 * 下载链路由现有 `github_proxy_prefix` 统一代理（`download_custom_rule_set`），
 * 本模块只提供目录常量，不含网络 / UI 依赖。条目 `id` 同时用作添加后的规则集
 * 引用 tag，`name` 用作添加后的规则集名称。
 */

/** 规则集市场条目（`format` 对齐 `CustomRuleSetSource.format`）。 */
export interface RuleSetMarketEntry {
  /** 条目 id，同时用作添加后的规则集引用 tag（须唯一，如 `ads`）。 */
  id: string;
  /** 中文名称（添加后的规则集名称）。 */
  name: string;
  /** 中文描述。 */
  description: string;
  /** 分类（用于市场页筛选）。 */
  category: string;
  /** 规则集格式：`.srs` 二进制 = binary。 */
  format: "binary" | "source";
  /** 规则集下载 URL。 */
  url: string;
}

/** DustinWin `sing-box-ruleset` tag 的固定下载前缀。 */
const RULESET_MARKET_BASE_URL = "https://github.com/DustinWin/ruleset_geodata/releases/download/sing-box-ruleset";

/** 构造 `.srs`（binary）条目：URL 固定为 `<base>/<id>.srs`。 */
function entry(id: string, name: string, description: string, category: string): RuleSetMarketEntry {
  return {
    id,
    name,
    description,
    category,
    format: "binary",
    url: `${RULESET_MARKET_BASE_URL}/${id}.srs`,
  };
}

/**
 * 精选目录（20 条，覆盖 广告 / AI / 中国 / 流媒体 / 通讯 / 代理 / 应用）。
 *
 * 以 DustinWin/ruleset_geodata `sing-box-ruleset` release 实际资产为准
 * （66 个资产中精选高频域名类规则集）。
 */
export const RULESET_MARKET: RuleSetMarketEntry[] = [
  entry("ads", "广告拦截", "常见广告与追踪域名，适合 reject 规则", "广告"),
  entry("ai", "AI 服务", "OpenAI / Claude / Gemini 等 AI 服务域名", "AI"),
  entry("proxy", "代理域名", "常见代理协议与机场域名", "代理"),
  entry("gfw", "GFW 列表", "被 GFW 屏蔽的域名集合", "代理"),
  entry("cn", "中国大陆", "中国大陆域名全集，适合直连", "中国"),
  entry("cn-lite", "中国大陆（精简）", "中国大陆域名精简版，体积更小", "中国"),
  entry("apple-cn", "Apple 中国", "Apple 在中国大陆的域名", "中国"),
  entry("google-cn", "Google 中国", "Google 在中国大陆可直连的域名", "中国"),
  entry("microsoft-cn", "微软中国", "微软在中国大陆的域名", "中国"),
  entry("netflix", "Netflix", "Netflix 流媒体域名", "流媒体"),
  entry("youtube", "YouTube", "YouTube 流媒体域名", "流媒体"),
  entry("disney", "Disney+", "Disney+ 流媒体域名", "流媒体"),
  entry("primevideo", "Prime Video", "Amazon Prime Video 流媒体域名", "流媒体"),
  entry("spotify", "Spotify", "Spotify 音乐流媒体域名", "流媒体"),
  entry("appletv", "Apple TV+", "Apple TV+ 流媒体域名", "流媒体"),
  entry("bilibili", "哔哩哔哩", "哔哩哔哩（B 站）域名", "流媒体"),
  entry("tiktok", "TikTok", "TikTok / 抖音域名", "流媒体"),
  entry("telegramip", "Telegram", "Telegram 服务器 IP 段", "通讯"),
  entry("applications", "应用分流", "常见应用与软件的分流域名", "应用"),
  entry("games", "游戏平台", "游戏平台域名（Steam / Epic 等）", "应用"),
];
