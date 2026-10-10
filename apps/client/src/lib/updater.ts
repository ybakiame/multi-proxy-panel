/**
 * 桌面端自动更新（ADR-0008 D2）。
 *
 * 封装 `@tauri-apps/plugin-updater` 的检查 / 下载 / 安装 + `plugin-process` 的重启。
 * 仅桌面 UI 引用（Android 不注册对应插件，壳侧能力亦按 platforms 门控）；
 * 更新源为 GitHub Releases 的 latest.json（release workflow 产出，ed25519 签名校验）。
 */

import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";

export interface UpdateCheckResult {
  /** 有可用更新时为版本号（如 `0.2.0`）；已是最新为 null。 */
  version: string | null;
  /** 有可用更新时：下载安装并重启；失败抛错由调用方 toast。 */
  install: (() => Promise<void>) | null;
}

/** 检查更新；网络 / 端点错误直接抛给调用方。 */
export async function checkForUpdate(): Promise<UpdateCheckResult> {
  const update = await check();
  if (!update) {
    return { version: null, install: null };
  }
  return {
    version: update.version,
    install: async () => {
      await update.downloadAndInstall();
      await relaunch();
    },
  };
}
