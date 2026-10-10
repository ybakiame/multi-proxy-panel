import { Card } from "@pp/ui";
import { useThemePreference } from "@pp/client-core";
import { ComputerDesktopIcon, MoonIcon, SunIcon } from "@heroicons/react/24/outline";

/** 主题选项（值域与 `@pp/client-core` theme 模块的 `pp-ui-theme` 存储键一致）。 */
const THEME_OPTIONS = [
  { value: "system", label: "跟随系统", icon: ComputerDesktopIcon },
  { value: "light", label: "浅色", icon: SunIcon },
  { value: "dark", label: "深色", icon: MoonIcon },
] as const;

/**
 * 外观设置卡：浅色 / 深色 / 跟随系统三选（默认跟随系统）。
 *
 * 经 `@pp/client-core` 的 `useThemePreference` 切换并持久化（localStorage
 * `pp-ui-theme`，UI 无关键）；首帧由 index.html 预置脚本就位，`system` 下持续
 * 跟随系统深浅色变化。
 */
export default function AppearanceSettings() {
  const { theme, setTheme } = useThemePreference();

  return (
    <Card>
      <Card.Header>
        <Card.Title>外观</Card.Title>
        <Card.Description>界面主题，「跟随系统」随系统深浅色自动切换</Card.Description>
      </Card.Header>
      <Card.Content>
        <div className="flex gap-2" role="radiogroup" aria-label="主题">
          {THEME_OPTIONS.map((option) => {
            const active = theme === option.value;
            const Icon = option.icon;
            return (
              <label
                key={option.value}
                className={`flex flex-1 cursor-pointer items-center justify-center gap-2 rounded-xl border px-3 py-3 text-sm font-medium transition-colors ${
                  active
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-border/60 bg-surface text-muted hover:text-foreground"
                }`}
              >
                <input
                  type="radio"
                  name="theme"
                  value={option.value}
                  checked={active}
                  onChange={() => setTheme(option.value)}
                  className="sr-only"
                />
                <Icon className="size-5" aria-hidden="true" />
                {option.label}
              </label>
            );
          })}
        </div>
      </Card.Content>
    </Card>
  );
}
