import { ComputerDesktopIcon, MoonIcon, SunIcon } from "@heroicons/react/24/outline";
import { Segmented, SegmentedButton } from "konsta/react";
import { Card } from "../../components/ui";
import { useThemePreference } from "../../theme";
import type { ThemePreference, UiStylePreference } from "../../theme";

/** 主题选项（顺序即展示顺序；跟随系统为无记录时的默认值）。 */
const THEME_OPTIONS: { value: ThemePreference; label: string; icon: typeof SunIcon }[] = [
  { value: "system", label: "跟随系统", icon: ComputerDesktopIcon },
  { value: "light", label: "浅色", icon: SunIcon },
  { value: "dark", label: "深色", icon: MoonIcon },
];

/** UI 风格选项（Konsta UI 双主题；iOS 为无记录时的默认值）。 */
const UI_STYLE_OPTIONS: { value: UiStylePreference; label: string }[] = [
  { value: "ios", label: "iOS" },
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
  const { theme, setTheme, uiStyle, setUiStyle } = useThemePreference();

  return (
    <Card>
      <Card.Header>
        <Card.Title>外观</Card.Title>
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
                    <text className="text-md">{option.label}</text>
                  </span>
                </SegmentedButton>
              );
            })}
          </Segmented>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">界面风格</span>
          <Segmented strong>
            {UI_STYLE_OPTIONS.map((option) => (
              <SegmentedButton
                key={option.value}
                active={uiStyle === option.value}
                onClick={() => setUiStyle(option.value)}
                className="min-h-8"
                aria-pressed={uiStyle === option.value}
              >
                {option.label}
              </SegmentedButton>
            ))}
          </Segmented>
        </div>
      </Card.Content>
    </Card>
  );
}
