import { ComputerDesktopIcon, MoonIcon, SunIcon } from "@heroicons/react/24/outline";
import { Segmented, SegmentedButton } from "konsta/react";
import { Card } from "../../components/ui";
import { DEFAULT_THEME, DEFAULT_UI_STYLE, useThemePreference } from "../../theme";
import type { ThemePreference, UiStylePreference } from "../../theme";

/** 主题选项（顺序即展示顺序；跟随系统为无记录时的默认值）。 */
const THEME_OPTIONS: { value: ThemePreference; label: string; icon: typeof SunIcon }[] = [
  { value: "system", label: `跟随系统${DEFAULT_THEME === "system" ? "（默认）" : ""}`, icon: ComputerDesktopIcon },
  { value: "light", label: "浅色", icon: SunIcon },
  { value: "dark", label: "深色", icon: MoonIcon },
];

/** UI 风格选项（Konsta UI 双主题；iOS 为无记录时的默认值）。 */
const UI_STYLE_OPTIONS: { value: UiStylePreference; label: string }[] = [
  { value: "ios", label: `iOS${DEFAULT_UI_STYLE === "ios" ? "（默认）" : ""}` },
  { value: "material", label: "Material" },
];

/**
 * 外观设置卡（设置主页直挂，非二级页）。
 *
 * 两组三选/二选分段控件（Konsta `Segmented`）：
 * - 主题：跟随系统 / 浅色 / 深色，选择持久化到 localStorage 并即时同步 `<html>`
 *   （index.html 预置脚本在首帧前读取同一键，冷启动无闪变）；
 * - UI 风格：iOS / Material（Konsta UI 双主题，经 `KonstaProvider` 响应式切换，
 *   无需重启）。
 */
export function AppearanceCard() {
  const { theme, resolvedTheme, setTheme, uiStyle, setUiStyle } = useThemePreference();

  return (
    <Card>
      <Card.Header>
        <Card.Title>外观</Card.Title>
        <Card.Description>主题与界面风格 · 更改即时生效，重启应用后保持</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">主题</span>
          <Segmented strong>
            {THEME_OPTIONS.map((option) => {
              const Icon = option.icon;
              return (
                <SegmentedButton
                  key={option.value}
                  active={theme === option.value}
                  onClick={() => setTheme(option.value)}
                  className="min-h-11"
                  aria-pressed={theme === option.value}
                >
                  <span className="inline-flex items-center gap-1.5">
                    <Icon className="size-4" aria-hidden="true" />
                    {option.label}
                  </span>
                </SegmentedButton>
              );
            })}
          </Segmented>
          <span className="text-xs text-zinc-500 dark:text-zinc-400">
            {theme === "system"
              ? `跟随系统：当前为${resolvedTheme === "light" ? "浅色" : "深色"}（系统切换时自动跟随）`
              : "切换后立即应用于全部页面"}
          </span>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">界面风格</span>
          <Segmented strong>
            {UI_STYLE_OPTIONS.map((option) => (
              <SegmentedButton
                key={option.value}
                active={uiStyle === option.value}
                onClick={() => setUiStyle(option.value)}
                className="min-h-11"
                aria-pressed={uiStyle === option.value}
              >
                {option.label}
              </SegmentedButton>
            ))}
          </Segmented>
          <span className="text-xs text-zinc-500 dark:text-zinc-400">
            iOS 与 Material（Android）两套组件视觉风格，切换即时生效
          </span>
        </div>
      </Card.Content>
    </Card>
  );
}
