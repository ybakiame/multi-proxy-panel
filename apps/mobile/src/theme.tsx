import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

/** 用户可选择的主题偏好。 */
export type ThemePreference = "system" | "light" | "dark";

/** 无记录时的默认主题：深色（历史版本固定深色，行为不变，index.html 预置脚本同值）。 */
export const DEFAULT_THEME: ThemePreference = "dark";

/**
 * localStorage 存储键：沿用历史键名 `heroui-theme`——index.html 首帧预置脚本
 * 读取同一键，改名会导致存量用户主题偏好丢失且需同步改脚本。
 */
const STORAGE_KEY = "heroui-theme";

/** 读取持久化的主题偏好（非法值/存储不可用时回落默认）。 */
function readStoredPreference(): ThemePreference {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
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

interface ThemeContextValue {
  /** 用户偏好（可能为 `system`）。 */
  theme: ThemePreference;
  /** 实际生效的主题（`system` 解析后的具体值）。 */
  resolvedTheme: "light" | "dark";
  /** 切换偏好（持久化到 localStorage 并同步 `<html>` 的 class/data-theme）。 */
  setTheme: (theme: ThemePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: DEFAULT_THEME,
  resolvedTheme: "dark",
  setTheme: () => undefined,
});

/**
 * 主题 Provider（App 根部常驻挂载）。
 *
 * Konsta UI 迁移后改为自管理（原封装 HeroUI `useTheme`）：单实例常驻保证
 * 「跟随系统」时持续监听系统深浅色变化；DOM 同步为 `<html>` 的
 * `light`/`dark` class + `data-theme` 属性（Konsta 暗色变体依赖 `.dark` 类，
 * 见 konsta/styles/base.css 的 @custom-variant dark），index.html 预置脚本
 * 保证首帧前已就位，本 Provider 挂载后无缝接管。
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemePreference>(readStoredPreference);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  // 监听系统深浅色变化（仅「跟随系统」时影响 resolvedTheme；常开监听成本可忽略）。
  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  const resolvedTheme: "light" | "dark" = theme === "system" ? (systemDark ? "dark" : "light") : theme;

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.classList.toggle("light", resolvedTheme === "light");
    root.setAttribute("data-theme", resolvedTheme);
  }, [resolvedTheme]);

  const value = useMemo<ThemeContextValue>(
    () => ({
      theme,
      resolvedTheme,
      setTheme: (next) => {
        setThemeState(next);
        try {
          localStorage.setItem(STORAGE_KEY, next);
        } catch {
          /* 存储不可用：仅本次会话生效 */
        }
      },
    }),
    [theme, resolvedTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** 读取当前主题偏好与切换方法（设置页「外观」卡消费）。 */
export function useThemePreference(): ThemeContextValue {
  return useContext(ThemeContext);
}
