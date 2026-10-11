import { getCurrentWindow } from "@tauri-apps/api/window";

/** 桌面窗口操作入口，与其他原生能力一样由共享库提供。 */
export const clientWindow = getCurrentWindow;
