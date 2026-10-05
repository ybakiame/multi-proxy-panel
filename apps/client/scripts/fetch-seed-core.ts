/**
 * Fetch the bundled seed core for the Windows installer (ADR-0008 D5).
 *
 * Usage: bun scripts/fetch-seed-core.ts <amd64|arm64> [--force]
 *
 * Reads `src-tauri/seed-manifest.json` (pinned versions + SHA256), downloads the
 * sing-box release asset and the wintun build, verifies hashes, and extracts into
 * `src-tauri/resources/seed/`:
 *
 *   resources/seed/sing-box.exe            核心二进制（目标架构）
 *   resources/seed/wintun.dll              TUN 依赖（目标架构）
 *   resources/seed/manifest.json           运行时清单（{sing_box_version, arch}）
 *   resources/seed/licenses/               上游许可证文本（GPL 合规）
 *
 * The directory is consumed by `src-tauri/tauri.windows.conf.json`
 * (`bundle.resources`) and seeded into the user data dir on first run by
 * `pp_client::cores::seed_bundled_core`.
 */

import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { unzipSync } from "fflate";

const ARCHES = ["amd64", "arm64"] as const;
type Arch = (typeof ARCHES)[number];

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(appDir, "src-tauri", "seed-manifest.json");
const outDir = join(appDir, "src-tauri", "resources", "seed");
const licensesDir = join(outDir, "licenses");

interface Manifest {
  sing_box: {
    version: string;
    release_url_base: string;
    assets: Record<Arch, { file: string; sha256: string }>;
  };
  wintun: {
    version: string;
    url: string;
    sha256: string;
    dll_path: Record<Arch, string>;
  };
  gpl3: {
    url: string;
    sha256: string;
  };
}

function parseArgs(): { arch: Arch; force: boolean } {
  const args = process.argv.slice(2);
  const force = args.includes("--force");
  const arch = args.find((a) => !a.startsWith("--"));
  if (!arch || !ARCHES.includes(arch as Arch)) {
    console.error(`usage: bun scripts/fetch-seed-core.ts <${ARCHES.join("|")}> [--force]`);
    process.exit(1);
  }
  return { arch: arch as Arch, force };
}

async function downloadVerified(url: string, sha256: string, label: string): Promise<Uint8Array> {
  console.log(`下载 ${label}: ${url}`);
  const resp = await fetch(url, { headers: { "User-Agent": "proxy-panel-client" } });
  if (!resp.ok) {
    throw new Error(`${label} 下载失败: HTTP ${resp.status}`);
  }
  const bytes = new Uint8Array(await resp.arrayBuffer());
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== sha256) {
    throw new Error(`${label} SHA256 校验失败: 期望 ${sha256}，实际 ${actual}`);
  }
  console.log(`  ✓ SHA256 校验通过 (${bytes.length} 字节)`);
  return bytes;
}

/** 在 zip 条目里按后缀查找（sing-box zip 内条目带 `sing-box-<ver>-windows-<arch>/` 前缀）。 */
function findEntry(entries: Record<string, Uint8Array>, suffix: string, label: string): Uint8Array {
  const hit = Object.entries(entries).find(
    ([name, data]) => name.endsWith(suffix) && data.length > 0,
  );
  if (!hit) {
    throw new Error(`${label} 内未找到条目 *${suffix}`);
  }
  return hit[1];
}

async function main(): Promise<void> {
  const { arch, force } = parseArgs();
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as Manifest;
  const version = manifest.sing_box.version;

  const runtimeManifestPath = join(outDir, "manifest.json");
  if (!force && existsSync(runtimeManifestPath)) {
    const existing = JSON.parse(readFileSync(runtimeManifestPath, "utf8")) as {
      sing_box_version?: string;
      arch?: string;
    };
    if (
      existing.sing_box_version === version &&
      existing.arch === arch &&
      existsSync(join(outDir, "sing-box.exe")) &&
      existsSync(join(outDir, "wintun.dll"))
    ) {
      console.log(`种子已是最新（sing-box ${version} / ${arch}），跳过。--force 强制重取。`);
      return;
    }
  }

  mkdirSync(licensesDir, { recursive: true });

  // sing-box 核心（zip 内含 sing-box.exe + LICENSE；libcronet.dll 仅 Naive 出站用，
  // 客户端不使用该出站，与运行时下载通道保持一致的提取范围）。
  const asset = manifest.sing_box.assets[arch];
  const sbZip = await downloadVerified(
    `${manifest.sing_box.release_url_base}/${asset.file}`,
    asset.sha256,
    `sing-box ${version} (${arch})`,
  );
  const sbEntries = unzipSync(sbZip);
  writeFileSync(join(outDir, "sing-box.exe"), findEntry(sbEntries, "/sing-box.exe", asset.file));
  writeFileSync(
    join(licensesDir, "sing-box-LICENSE.txt"),
    findEntry(sbEntries, "/LICENSE", asset.file),
  );

  // wintun（TUN 依赖，sing-box 不内嵌；与运行时 ensure_wintun 同版本 0.14.1）。
  const wt = manifest.wintun;
  const wtZip = await downloadVerified(wt.url, wt.sha256, `wintun ${wt.version}`);
  const wtEntries = unzipSync(wtZip);
  writeFileSync(join(outDir, "wintun.dll"), findEntry(wtEntries, wt.dll_path[arch], "wintun"));
  writeFileSync(
    join(licensesDir, "wintun-LICENSE.txt"),
    findEntry(wtEntries, "/LICENSE.txt", "wintun"),
  );

  // GPL-3.0 全文（sing-box 随附的 LICENSE 仅为声明性短文本，GPL 合规要求随二进制
  // 附带许可证全文；wintun 为预编译二进制许可证，上文已从其 zip 提取全文）。
  const gplText = await downloadVerified(manifest.gpl3.url, manifest.gpl3.sha256, "GPL-3.0 全文");
  writeFileSync(join(licensesDir, "gpl-3.0.txt"), gplText);

  if (process.platform !== "win32") {
    chmodSync(join(outDir, "sing-box.exe"), 0o644);
  }
  writeFileSync(
    runtimeManifestPath,
    `${JSON.stringify({ sing_box_version: version, arch }, null, 2)}\n`,
  );
  console.log(`✓ 种子就绪：${outDir}（sing-box ${version} / ${arch} + wintun ${wt.version}）`);
}

await main();
