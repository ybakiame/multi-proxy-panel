import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { navItems, navSections } from "./nav";
import { BrandMark } from "./BrandMark";
import { VersionBadge } from "./VersionBadge";
import { useAuth } from "../context/AuthContext";
import { useSettingsStore } from "../stores/settings";
import {
  SunIcon,
  MoonIcon,
  Bars3Icon,
  XMarkIcon,
  ArrowRightEndOnRectangleIcon,
} from "@heroicons/react/24/outline";
import { Button } from "./ui/button";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Sheet, SheetClose, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "./ui/sheet";

function LanguageSelect({ id, className }: { id: string; className?: string }) {
  const { t, i18n } = useTranslation();
  const { setLanguage } = useSettingsStore();

  return (
    <div className={className}>
      <Label htmlFor={id} className="sr-only">
        {t("common.language")}
      </Label>
      <Select
        value={i18n.language}
        onValueChange={(lang) => {
          if (lang) setLanguage(lang as "zh-CN" | "en-US");
        }}
      >
        <SelectTrigger id={id} className="h-8 w-full text-xs" aria-label={t("common.language")}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="zh-CN">中文</SelectItem>
          <SelectItem value="en-US">English</SelectItem>
        </SelectContent>
      </Select>
    </div>
  );
}

function ThemeToggle({ className }: { className?: string }) {
  const { t } = useTranslation();
  const { theme, toggleTheme } = useSettingsStore();
  const isDark = theme === "dark";

  return (
    <Button
      variant="ghost"
      size="icon-sm"
      onClick={toggleTheme}
      className={className}
      aria-label={t("common.theme")}
    >
      {isDark ? <MoonIcon className="h-4 w-4" /> : <SunIcon className="h-4 w-4" />}
    </Button>
  );
}

function LogoutButton({ variant }: { variant: "icon" | "text" }) {
  const { t } = useTranslation();
  const { logout } = useAuth();

  if (variant === "icon") {
    return (
      <Button variant="ghost" size="icon-sm" onClick={logout} aria-label={t("common.logout")}>
        <ArrowRightEndOnRectangleIcon className="h-4 w-4" />
      </Button>
    );
  }
  return (
    <Button variant="outline" size="sm" onClick={logout}>
      <ArrowRightEndOnRectangleIcon className="h-3.5 w-3.5" />
      {t("common.logout")}
    </Button>
  );
}

function NavigationGroups({ onNavigate }: { onNavigate?: () => void }) {
  const { t } = useTranslation();
  const location = useLocation();

  return (
    <nav className="flex-1 overflow-y-auto px-3 pb-4" aria-label={t("nav.dashboard")}>
      <div className="space-y-5">
        {navSections.map((section) => {
          const items = navItems.filter((item) => item.section === section.key);
          if (items.length === 0) return null;
          return (
            <div key={section.key}>
              <p className="px-2 pb-1.5 text-[11px] font-medium tracking-wider text-muted-foreground uppercase">
                {t(section.labelKey)}
              </p>
              <ul className="space-y-0.5">
                {items.map((item) => {
                  const Icon = item.icon;
                  const isActive =
                    item.path === "/"
                      ? location.pathname === "/"
                      : location.pathname.startsWith(item.path);
                  return (
                    <li key={item.path}>
                      <Link
                        to={item.path}
                        onClick={onNavigate}
                        aria-current={isActive ? "page" : undefined}
                        className={`flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] font-medium transition-colors ${
                          isActive
                            ? "bg-primary text-primary-foreground shadow-sm"
                            : "text-muted-foreground hover:bg-muted hover:text-foreground"
                        }`}
                      >
                        <Icon className="h-4 w-4 shrink-0" />
                        {t(item.labelKey)}
                      </Link>
                    </li>
                  );
                })}
              </ul>
            </div>
          );
        })}
      </div>
    </nav>
  );
}

function BrandRow() {
  return (
    <div className="flex items-center gap-2.5 px-4 py-4">
      <BrandMark className="h-8 w-8" />
      <span className="text-[15px] font-semibold tracking-tight">ProxyPanel</span>
    </div>
  );
}

export function Layout({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const location = useLocation();
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  const currentNav = navItems
    .filter((item) =>
      item.path === "/" ? location.pathname === "/" : location.pathname.startsWith(item.path),
    )
    .sort((a, b) => b.path.length - a.path.length)[0];
  const pageTitle = currentNav ? t(currentNav.labelKey) : "ProxyPanel";

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-background md:flex-row">
      {/* 移动端顶栏 */}
      <div className="flex items-center justify-between border-b border-border bg-card px-4 py-2.5 md:hidden">
        <div className="flex items-center gap-2.5">
          <BrandMark />
          <span className="text-[15px] font-semibold tracking-tight">ProxyPanel</span>
        </div>
        <Sheet open={isMenuOpen} onOpenChange={setIsMenuOpen}>
          <SheetTrigger
            render={<Button variant="ghost" size="icon" aria-label={t("common.openMenu")} />}
          >
            <Bars3Icon className="h-5 w-5" />
          </SheetTrigger>
          <SheetContent side="left" showCloseButton={false} className="w-72 gap-0 p-0 md:hidden">
            <SheetHeader className="flex-row items-center justify-between gap-2 border-b border-border px-4 py-3">
              <SheetTitle className="sr-only">ProxyPanel</SheetTitle>
              <div className="flex items-center gap-2.5">
                <BrandMark />
                <span className="text-[15px] font-semibold tracking-tight">ProxyPanel</span>
              </div>
              <SheetClose
                render={
                  <Button variant="ghost" size="icon-sm" aria-label={t("common.closeMenu")} />
                }
              >
                <XMarkIcon className="h-4 w-4" />
              </SheetClose>
            </SheetHeader>
            <NavigationGroups onNavigate={() => setIsMenuOpen(false)} />
            <div className="space-y-3 border-t border-border p-4">
              <VersionBadge withStatus />
              <div className="flex items-center gap-2">
                <LanguageSelect id="language-select-mobile" className="flex-1" />
                <ThemeToggle />
              </div>
              <LogoutButton variant="text" />
            </div>
          </SheetContent>
        </Sheet>
      </div>

      {/* 桌面侧栏 */}
      <aside className="hidden w-60 flex-col border-r border-border bg-card md:flex lg:w-64">
        <BrandRow />
        <div className="px-4 pb-1">
          <VersionBadge withStatus />
        </div>
        <NavigationGroups />
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {/* 桌面顶部栏 */}
        <header className="hidden h-14 shrink-0 items-center justify-between gap-4 border-b border-border bg-card px-6 md:flex">
          <h1 className="truncate text-sm font-semibold">{pageTitle}</h1>
          <div className="flex items-center gap-2">
            <VersionBadge withStatus className="mr-1" />
            <LanguageSelect id="language-select" className="w-[104px]" />
            <ThemeToggle />
            <LogoutButton variant="text" />
          </div>
        </header>

        <main className="flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-[1440px] px-4 py-5 md:px-6 md:py-6">{children}</div>
        </main>
      </div>
    </div>
  );
}
