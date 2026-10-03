import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import { useThemeState } from "@pp/client-core";
import type { ResolvedTheme, ThemePreference } from "@pp/client-core";

// 主题类型经本模块 re-export（消费方维持 `from "../theme"` 的既有导入路径）。
export type { ThemePreference } from "@pp/client-core";

/**
 * 用户可选择的 UI 风格（Konsta UI 双主题，经 `KonstaProvider` 动态切换）。
 * 默认 `ios`。
 */
export type UiStylePreference = "ios" | "material";

/** 无记录时的默认 UI 风格：iOS。 */
export const DEFAULT_UI_STYLE: UiStylePreference = "ios";

/** UI 风格（iOS/Material）存储键：新功能，无历史兼容负担，用项目前缀。 */
const UI_STYLE_STORAGE_KEY = "pp-ui-style";

/**
 * 主题（浅色 / 深色 / 跟随系统）部分已上移 `@pp/client-core` 的 theme 模块
 * （2026-10，桌面端自 HeroUI `useTheme` 硬编码键迁入前的统一化）：唯一存储键
 * `pp-ui-theme`、存量 `heroui-theme` 一次性迁移、DOM 同步与系统监听全部由
 * 共享模块承担；本文件只保留移动端特有的 UI 风格（iOS / Material）状态。
 */

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
  resolvedTheme: ResolvedTheme;
  /** 切换偏好（持久化到 localStorage 并同步 `<html>` 的 class/data-theme）。 */
  setTheme: (theme: ThemePreference) => void;
  /** UI 风格（iOS / Material，默认 iOS）。 */
  uiStyle: UiStylePreference;
  /** 切换 UI 风格（持久化到 localStorage；KonstaProvider 响应式切换）。 */
  setUiStyle: (style: UiStylePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "system",
  resolvedTheme: "light",
  setTheme: () => undefined,
  uiStyle: DEFAULT_UI_STYLE,
  setUiStyle: () => undefined,
});

/**
 * 主题 Provider（App 根部常驻挂载）。
 *
 * 主题部分委托 `@pp/client-core` 的 `useThemeState`（DOM 同步为 `<html>` 的
 * `light`/`dark` class + `data-theme` 属性，Konsta 暗色变体依赖 `.dark` 类；
 * index.html 预置脚本保证首帧前已就位）；UI 风格（iOS / Material）为本 Provider
 * 自有状态。
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const { theme, resolvedTheme, setTheme } = useThemeState();
  const [uiStyle, setUiStyleState] = useState<UiStylePreference>(readStoredUiStyle);

  const value = useMemo<ThemeContextValue>(
    () => ({
      theme,
      resolvedTheme,
      setTheme,
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
    [theme, resolvedTheme, setTheme, uiStyle],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** 读取当前主题偏好与切换方法（设置页「外观」卡消费）。 */
export function useThemePreference(): ThemeContextValue {
  return useContext(ThemeContext);
}
