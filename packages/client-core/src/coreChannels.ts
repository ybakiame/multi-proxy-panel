/**
 * 核心版本通道（稳定版 / 测试版 / 预发布版）纯工具：分类与语义化版本比较。
 *
 * 通道模型参考 `.reference/GUI.for.SingBox`（Stable / Alpha 分支）；分类规则与
 * Rust 侧 `pp_client::cores::channel_of_version` 完全一致（纯 tag 后缀判断，
 * 不依赖 GitHub `prerelease` 标志），前端可据此把本地已装版本归入通道。
 */

/** 核心版本通道：`stable`（稳定版）/ `beta`（测试版）/ `prerelease`（预发布版）。 */
export type CoreChannel = "stable" | "beta" | "prerelease";

/** 通道展示元数据（核心管理页按此顺序渲染）。 */
export const CORE_CHANNELS: readonly { id: CoreChannel; label: string; description: string }[] = [
  { id: "stable", label: "稳定版", description: "正式发布的稳定版本，推荐日常使用" },
  { id: "beta", label: "测试版", description: "beta / rc 预发布，包含即将发布的新特性" },
  { id: "prerelease", label: "预发布版", description: "alpha 等早期构建，可能不稳定" },
];

/**
 * 按版本号的预发布后缀分类通道：无 `-` 后缀 → `stable`；后缀以 `beta` / `rc`
 * 开头 → `beta`；其余（`alpha` 等）→ `prerelease`。
 */
export function coreChannelOfVersion(version: string): CoreChannel {
  const v = version.trim().replace(/^v/, "").toLowerCase();
  const idx = v.indexOf("-");
  if (idx === -1) {
    return "stable";
  }
  const marker = v.slice(idx + 1);
  return marker.startsWith("beta") || marker.startsWith("rc") ? "beta" : "prerelease";
}

/**
 * 语义化版本比较（与 Rust `cores::version::compare_core_versions` 语义一致）：
 * 数字段逐段比较（段数更少者小）；数字段相同时无预发布后缀者大于有后缀者，
 * 两侧均有后缀时按后缀字符串比较。返回值同 `Array.prototype.sort` 比较函数。
 */
export function compareCoreVersions(a: string, b: string): number {
  const [aPre, aCore] = splitVersionIdentity(a);
  const [bPre, bCore] = splitVersionIdentity(b);
  const aNums = parseNumericSegments(aCore);
  const bNums = parseNumericSegments(bCore);
  for (let i = 0; i < Math.min(aNums.length, bNums.length); i++) {
    if (aNums[i] !== bNums[i]) {
      return aNums[i] - bNums[i];
    }
  }
  if (aNums.length !== bNums.length) {
    return aNums.length - bNums.length;
  }
  if (aPre === null && bPre === null) {
    return 0;
  }
  if (aPre === null) {
    return 1;
  }
  if (bPre === null) {
    return -1;
  }
  if (aPre !== bPre) {
    return aPre < bPre ? -1 : 1;
  }
  return aCore < bCore ? -1 : aCore > bCore ? 1 : 0;
}

/** 拆分版本为 [预发布后缀（小写，无则 null）, 数字核心段]。 */
function splitVersionIdentity(version: string): [string | null, string] {
  const v = version.trim();
  const idx = v.indexOf("-");
  if (idx > 0 && /^\d/.test(v)) {
    return [v.slice(idx + 1).toLowerCase(), v.slice(0, idx)];
  }
  return [null, v];
}

/** 解析 `.` 分隔的数字段（非数字尾部截断，空段丢弃）。 */
function parseNumericSegments(core: string): number[] {
  return core
    .split(".")
    .map((seg) => seg.replace(/[^\d].*$/, ""))
    .filter((seg) => seg !== "")
    .map((seg) => Number.parseInt(seg, 10))
    .filter((n) => Number.isFinite(n));
}

/** 取版本列表中指定通道的最新版本（无则 `undefined`）。 */
export function latestVersionOfChannel(versions: string[], channel: CoreChannel): string | undefined {
  return versions
    .filter((version) => coreChannelOfVersion(version) === channel)
    .sort((a, b) => compareCoreVersions(b, a))[0];
}
