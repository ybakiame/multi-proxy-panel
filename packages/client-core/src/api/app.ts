import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";

export const PROJECT_URL = "https://github.com/ybakiame/multi-proxy-panel";

/** 读取壳的实际应用版本，避免 UI 常量与安装包版本漂移。 */
export const getAppVersion = getVersion;

/** 通过原生 opener 在系统浏览器中打开项目主页。 */
export function openProjectUrl(): Promise<void> {
  return openUrl(PROJECT_URL);
}
