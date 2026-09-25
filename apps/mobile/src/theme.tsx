import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

/** 用户可选择的主题偏好。 */
export type ThemePreference = "system" | "light" | "dark";

/**
 * 用户可选择的 UI 风格（Konsta UI 双主题，经 `KonstaProvider` 动态切换）。
 * 默认 `ios`。
 */
export type UiStylePreference = "ios" | "material";

/** 无记录时的默认主题：跟随系统（index.html 预置脚本同值）。 */
export const DEFAULT_THEME: ThemePreference = "system";

/** 无记录时的默认 UI 风格：iOS。 */
export const DEFAULT_UI_STYLE: UiStylePreference = "ios";

/**
 * localStorage 存储键：与 UI 库无关（`pp-ui-theme`），index.html 首帧预置脚本读取同一键。
 */
const STORAGE_KEY = "pp-ui-theme";
/** 历史键（HeroUI 时代，2026-03 引入 `pp-ui-theme`）：仅由启动时一次性迁移读取。 */
const LEGACY_STORAGE_KEY = "heroui-theme";

/**
 * 存量主题键一次性迁移（幂等）：`heroui-theme` → `pp-ui-theme`。
 *
 * 模块加载即执行一次（早于首帧渲染），此后读写只面向新键——遵守
 * `.agents/rules/data-migration.md`：迁移是一次性动作，不做常驻双键回退。
 * （index.html 预置脚本因无法复用模块代码，保留只读回退兜底，属规则允许的例外。）
 */
function migrateLegacyStorageKeys(): void {
  try {
    const legacy = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacy !== null && localStorage.getItem(STORAGE_KEY) === null) {
      localStorage.setItem(STORAGE_KEY, legacy);
    }
    if (legacy !== null) {
      localStorage.removeItem(LEGACY_STORAGE_KEY);
    }
  } catch {
    /* 存储不可用（如 WebView 私密模式）：跳过，本次会话用默认值 */
  }
}
migrateLegacyStorageKeys();

/** UI 风格（iOS/Material）存储键：新功能，无历史兼容负担，用项目前缀。 */
const UI_STYLE_STORAGE_KEY = "pp-ui-style";

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

/** 读取持久化的 UI 风格（非法值/存储不可用时回落默认 ios）。 */
function readStoredUiStyle(): UiStylePreference {
  try {
    const stored = localStorage.getItem(UI_STYLE_STORAGE_KEY);
    if (stored === "ios" || stored === "material") {
      return stored;
    }
  } catch {
    /* 存储不可用（如 WebView 私密模式）：按默认处理 */
  }
  return DEFAULT_UI_STYLE;
}

interface ThemeContextValue {
  /** 用户偏好（可能为 `system`）。 */
  theme: ThemePreference;
  /** 实际生效的主题（`system` 解析后的具体值）。 */
  resolvedTheme: "light" | "dark";
  /** 切换偏好（持久化到 localStorage 并同步 `<html>` 的 class/data-theme）。 */
  setTheme: (theme: ThemePreference) => void;
  /** UI 风格（iOS / Material，默认 iOS）。 */
  uiStyle: UiStylePreference;
  /** 切换 UI 风格（持久化到 localStorage；KonstaProvider 响应式切换）。 */
  setUiStyle: (style: UiStylePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: DEFAULT_THEME,
  resolvedTheme: "light",
  setTheme: () => undefined,
  uiStyle: DEFAULT_UI_STYLE,
  setUiStyle: () => undefined,
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
  const [uiStyle, setUiStyleState] = useState<UiStylePreference>(readStoredUiStyle);

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
      uiStyle,
      setUiStyle: (next) => {
        setUiStyleState(next);
        try {
          localStorage.setItem(UI_STYLE_STORAGE_KEY, next);
        } catch {
          /* 存储不可用：仅本次会话生效 */
        }
      },
    }),
    [theme, resolvedTheme, uiStyle],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** 读取当前主题偏好与切换方法（设置页「外观」卡消费）。 */
export function useThemePreference(): ThemeContextValue {
  return useContext(ThemeContext);
}
