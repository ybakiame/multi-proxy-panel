/**
 * UI 无关的主题（浅色 / 深色 / 跟随系统）管理：存储键、存量迁移、DOM 同步与
 * React 绑定。
 *
 * 历史：桌面端曾直接封装 HeroUI v3 `useTheme`，其存储键 `heroui-theme` 硬编码
 * 不可配（UI 库耦合）；移动端 2026-03 已迁往 UI 无关键 `pp-ui-theme`（ Konsta
 * 迁移时自管理）。2026-10 客户端合并准备期统一上移到本模块：双端（及合并后的
 * apps/client）共享唯一键 `pp-ui-theme`，index.html 首帧预置脚本读取同一键。
 *
 * DOM 契约（与 HeroUI `useTheme` / Konsta 暗色变体双方兼容）：`<html>` 的
 * `light`/`dark` class 二选一 + `data-theme` 属性；`system` 时持续监听系统
 * 深浅色变化。首帧由 index.html 预置脚本就位，React 挂载后本模块无缝接管。
 */

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

/** 用户可选择的主题偏好。 */
export type ThemePreference = "system" | "light" | "dark";

/** 解析后的具体主题。 */
export type ResolvedTheme = "light" | "dark";

/** 无记录时的默认主题：跟随系统（index.html 预置脚本同值）。 */
export const DEFAULT_THEME: ThemePreference = "system";

/**
 * localStorage 存储键：与 UI 库无关（`pp-ui-theme`），index.html 首帧预置脚本
 * 读取同一键。
 */
export const THEME_STORAGE_KEY = "pp-ui-theme";

/** 历史键（HeroUI `useTheme` 硬编码键）：仅由启动时一次性迁移读取。 */
const LEGACY_STORAGE_KEY = "heroui-theme";

/**
 * 存量主题键一次性迁移（幂等）：`heroui-theme` → `pp-ui-theme`。
 *
 * 模块加载即执行一次（早于首帧渲染之外的任何读写），此后读写只面向新键——
 * 遵守 `.agents/rules/data-migration.md`：迁移是一次性动作，不做常驻双键回退。
 * （index.html 预置脚本因无法复用模块代码，保留只读回退兜底，属规则允许的例外。）
 */
function migrateLegacyStorageKeys(): void {
  try {
    const legacy = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacy !== null && localStorage.getItem(THEME_STORAGE_KEY) === null) {
      localStorage.setItem(THEME_STORAGE_KEY, legacy);
    }
    if (legacy !== null) {
      localStorage.removeItem(LEGACY_STORAGE_KEY);
    }
  } catch {
    /* 存储不可用（如 WebView 私密模式）：跳过，本次会话用默认值 */
  }
}
migrateLegacyStorageKeys();

/** 读取持久化的主题偏好（非法值/存储不可用时回落默认）。 */
function readStoredPreference(): ThemePreference {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "light" || stored === "dark" || stored === "system") {
      return stored;
    }
  } catch {
    /* 存储不可用（如 WebView 私密模式）：按默认处理 */
  }
  return DEFAULT_THEME;
}

function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches === true;
}

export interface ThemeState {
  /** 用户偏好（可能为 `system`）。 */
  theme: ThemePreference;
  /** 实际生效的主题（`system` 解析后的具体值）。 */
  resolvedTheme: ResolvedTheme;
  /** 切换偏好（持久化到 localStorage 并同步 `<html>` 的 class/data-theme）。 */
  setTheme: (theme: ThemePreference) => void;
}

/**
 * 主题状态钩子（自含：偏好 state + 系统深浅色监听 + DOM 同步）。
 *
 * 供双端各自的主题 Provider 复用；无 Provider 场景（桌面端）可直接用下方的
 * 现成 [`ThemeProvider`] / [`useThemePreference`]。
 */
export function useThemeState(): ThemeState {
  const [theme, setThemeState] = useState<ThemePreference>(readStoredPreference);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // 监听系统深浅色变化（仅「跟随系统」时影响 resolvedTheme；常开监听成本可忽略）。
  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  const resolvedTheme: ResolvedTheme = theme === "system" ? (systemDark ? "dark" : "light") : theme;

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.classList.toggle("light", resolvedTheme === "light");
    root.setAttribute("data-theme", resolvedTheme);
  }, [resolvedTheme]);

  return useMemo(
    () => ({
      theme,
      resolvedTheme,
      setTheme: (next: ThemePreference) => {
        setThemeState(next);
        try {
          localStorage.setItem(THEME_STORAGE_KEY, next);
        } catch {
          /* 存储不可用：仅本次会话生效 */
        }
      },
    }),
    [theme, resolvedTheme],
  );
}

const ThemeContext = createContext<ThemeState>({
  theme: DEFAULT_THEME,
  resolvedTheme: "light",
  setTheme: () => undefined,
});

/**
 * 主题 Provider（App 根部常驻挂载）：单实例常驻保证「跟随系统」时持续监听系统
 * 深浅色变化；index.html 预置脚本保证首帧前已就位，本 Provider 挂载后无缝接管。
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const value = useThemeState();
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** 读取当前主题偏好与切换方法（设置页「外观」卡消费）。 */
export function useThemePreference(): ThemeState {
  return useContext(ThemeContext);
}
