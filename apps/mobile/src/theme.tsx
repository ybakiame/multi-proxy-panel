import { createContext, useContext, type ReactNode } from "react";
import { useTheme } from "@heroui/react";

/** 用户可选择的主题偏好（与 HeroUI `useTheme` 值域一致，持久化于 localStorage）。 */
export type ThemePreference = "system" | "light" | "dark";

/** 无记录时的默认主题：深色（历史版本固定深色，行为不变，index.html 预置脚本同值）。 */
export const DEFAULT_THEME: ThemePreference = "dark";

interface ThemeContextValue {
  /** 用户偏好（可能为 `system`）。 */
  theme: ThemePreference;
  /** 实际生效的主题（`system` 解析后的具体值）。 */
  resolvedTheme: "light" | "dark" | undefined;
  /** 切换偏好（HeroUI 内部持久化到 localStorage 并同步 `<html>` 的 class/data-theme）。 */
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
 * 封装 HeroUI `useTheme` 并经 Context 下发：单实例常驻保证「跟随系统」时监听
 * 系统深浅色变化（若挂在设置页内，离开页面后监听随组件卸载失效）。DOM 同步由
 * HeroUI 完成（`<html>` class + data-theme），index.html 预置脚本保证首帧前
 * 已就位，本 Provider 挂载后无缝接管。
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const { theme, resolvedTheme, setTheme } = useTheme(DEFAULT_THEME);
  return (
    <ThemeContext.Provider
      value={{
        theme: theme as ThemePreference,
        resolvedTheme: resolvedTheme as "light" | "dark" | undefined,
        setTheme,
      }}
    >
      {children}
    </ThemeContext.Provider>
  );
}

/** 读取当前主题偏好与切换方法（设置页「外观」卡消费）。 */
export function useThemePreference(): ThemeContextValue {
  return useContext(ThemeContext);
}
