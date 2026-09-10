/**
 * 规则集市场源纯逻辑（Desktop / Mobile 共享）。
 *
 * 与 Rust 侧 `commands/local_override/market.rs::detect_market_source` 保持同一套
 * 识别规则：单输入框支持 `owner/repo` 简写、完整 GitHub URL（可含
 * `/releases/tag/<tag>`）或 JSON 目录 URL；`tag` 为 GitHub 源可选 release tag
 * （空 = latest）。
 */

/** 前端展示用的源类型（Rust View 的 `kind` 字段同名映射）。 */
export type MarketSourceKindId = "github" | "json";

export interface DetectedMarketSource {
  kind: MarketSourceKindId;
  /** GitHub `owner/repo`（仅 github）。 */
  ownerRepo?: string;
  /** GitHub release tag（空 = latest）。 */
  tag: string;
}

/** GitHub owner 仅字母数字与连字符（避免把 `example.com/x.json` 误判）。 */
const GITHUB_OWNER_RE = /^[A-Za-z0-9-]+$/;

/** 识别单输入框的市场源类型（与 Rust 侧规则一致）。 */
export function detectMarketSource(input: string, tag?: string): DetectedMarketSource {
  const raw = input.trim();
  const explicitTag = (tag ?? "").trim();
  const hostAndPath = raw.replace(/^https?:\/\//, "").replace(/^www\./, "");

  const ghPrefix = "github.com/";
  if (hostAndPath.startsWith(ghPrefix)) {
    const segs = hostAndPath.slice(ghPrefix.length).split("/").filter(Boolean);
    const owner = segs[0] ?? "";
    const repo = (segs[1] ?? "").replace(/\.git$/, "");
    if (owner && repo) {
      const urlTag = segs.length >= 5 && segs[2] === "releases" && segs[3] === "tag" ? segs[4] : "";
      return { kind: "github", ownerRepo: `${owner}/${repo}`, tag: explicitTag || urlTag };
    }
  }

  // `owner/repo` 简写：无 scheme 且恰好一个 `/`。
  if (!raw.includes("://") && raw.split("/").length === 2) {
    const [owner, repo] = raw.split("/");
    if (GITHUB_OWNER_RE.test(owner) && repo && !/[/:#?\s]/.test(repo)) {
      return { kind: "github", ownerRepo: `${owner}/${repo}`, tag: explicitTag };
    }
  }

  return { kind: "json", tag: "" };
}

/** 由单输入框推导源显示名（表单不再单列名称字段）。 */
export function deriveMarketSourceName(input: string, tag?: string): string {
  const detected = detectMarketSource(input, tag);
  if (detected.kind === "github" && detected.ownerRepo) {
    return detected.tag ? `${detected.ownerRepo}@${detected.tag}` : detected.ownerRepo;
  }
  const raw = input.trim();
  try {
    const url = new URL(raw);
    const path = url.pathname && url.pathname !== "/" ? url.pathname : "";
    return `${url.hostname}${path}`;
  } catch {
    return raw;
  }
}

/** 推荐市场源（空态卡片「一键添加」）。 */
export interface RecommendedMarketSource {
  /** GitHub `owner/repo`。 */
  ownerRepo: string;
  /** Release tag（空 = latest）。 */
  tag: string;
  /** 卡片标题。 */
  name: string;
  /** 卡片描述。 */
  description: string;
}

/**
 * 内置推荐源（GitHub releases 资产，自动识别 + 一键添加）。
 *
 * 注：原定的 `MetaCubeX/meta-rules-dat`（tag `sing`）已不再发布 `.srs` / `.json`
 * release 资产（仅 `.dat` / `.db` / `.mmdb`，且无 `sing` tag），无法被本市场消费，
 * 故替换为同为自动更新的 `Lynricsy/HyperADRules`（广告 / 恶意域名规则集）。
 */
export const RECOMMENDED_MARKET_SOURCES: RecommendedMarketSource[] = [
  {
    ownerRepo: "DustinWin/ruleset_geodata",
    tag: "sing-box-ruleset",
    name: "DustinWin 规则集",
    description: "sing-box 分流规则集（geosite / geoip，提供 srs 与 json）",
  },
  {
    ownerRepo: "Lynricsy/HyperADRules",
    tag: "",
    name: "HyperADRules",
    description: "广告 / 恶意域名聚合规则集（自动更新）",
  },
];
