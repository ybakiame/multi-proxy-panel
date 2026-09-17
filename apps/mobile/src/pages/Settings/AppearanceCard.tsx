import { ComputerDesktopIcon, MoonIcon, SunIcon } from "@heroicons/react/24/outline";
import { Card } from "@heroui/react";
import { DEFAULT_THEME, useThemePreference } from "../../theme";
import type { ThemePreference } from "../../theme";

/** 主题选项（顺序即展示顺序；深色为无记录时的默认值）。 */
const THEME_OPTIONS: { value: ThemePreference; label: string; description: string }[] = [
  { value: "system", label: "跟随系统", description: "随 Android 系统深浅色自动切换" },
  { value: "light", label: "浅色", description: "浅色主题" },
  { value: "dark", label: `深色${DEFAULT_THEME === "dark" ? "（默认）" : ""}`, description: "深色主题" },
];

/**
 * 外观设置卡（设置主页直挂，非二级页）。
 *
 * 三选一分段控件：跟随系统 / 浅色 / 深色。选择经 HeroUI `useTheme` 持久化到
 * localStorage 并即时同步 `<html>`（index.html 预置脚本在首帧前读取同一键，
 * 冷启动无闪变）；「跟随系统」下展示当前解析结果。
 */
export function AppearanceCard() {
  const { theme, resolvedTheme, setTheme } = useThemePreference();

  return (
    <Card>
      <Card.Header>
        <Card.Title>外观</Card.Title>
        <Card.Description>应用主题 · 更改即时生效，重启应用后保持</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-2">
        <fieldset className="flex min-w-0 gap-1 rounded-xl border border-border/60 bg-surface-secondary/40 p-1">
          <legend className="sr-only">主题</legend>
          {THEME_OPTIONS.map((option) => {
            const selected = theme === option.value;
            return (
              <button
                key={option.value}
                type="button"
                aria-pressed={selected}
                onClick={() => setTheme(option.value)}
                className={`flex min-h-11 flex-1 items-center justify-center gap-1.5 rounded-lg text-sm font-medium transition-colors active:opacity-80 ${
                  selected ? "bg-accent text-accent-foreground" : "text-muted hover:text-foreground"
                }`}
              >
                {option.value === "system" && <ComputerDesktopIcon className="size-4" aria-hidden="true" />}
                {option.value === "light" && <SunIcon className="size-4" aria-hidden="true" />}
                {option.value === "dark" && <MoonIcon className="size-4" aria-hidden="true" />}
                {option.label}
              </button>
            );
          })}
        </fieldset>
        <span className="text-xs text-muted">
          {theme === "system"
            ? `跟随系统：当前为${resolvedTheme === "light" ? "浅色" : "深色"}（系统切换时自动跟随）`
            : "切换后立即应用于全部页面"}
        </span>
      </Card.Content>
    </Card>
  );
}
