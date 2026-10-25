import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { navItems } from "./nav";
import { useAuth } from "../context/AuthContext";
import { useSettingsStore } from "../stores/settings";
import { SunIcon, MoonIcon, Bars3Icon, XMarkIcon } from "@heroicons/react/24/outline";
import { Button } from "./ui/button";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Sheet, SheetClose, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "./ui/sheet";

export function Layout({ children }: { children: React.ReactNode }) {
  const { t, i18n } = useTranslation();
  const { logout } = useAuth();
  const location = useLocation();
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const { theme, setLanguage, toggleTheme } = useSettingsStore();
  const isDark = theme === "dark";

  const changeLanguage = (lang: string) => {
    setLanguage(lang as "zh-CN" | "en-US");
  };

  const renderNavigation = () => (
    <>
      <nav className="flex-1 overflow-y-auto px-4 pb-4">
        <ul className="space-y-1">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = location.pathname === item.path;
            return (
              <li key={item.path}>
                <Link
                  to={item.path}
                  onClick={() => setIsMenuOpen(false)}
                  aria-current={isActive ? "page" : undefined}
                  className={`flex items-center gap-3 rounded-lg px-4 py-2.5 text-sm transition-colors ${
                    isActive
                      ? "bg-primary text-primary-foreground"
                      : "text-foreground hover:bg-muted"
                  }`}
                >
                  <Icon className="h-5 w-5" />
                  {t(item.labelKey)}
                </Link>
              </li>
            );
          })}
        </ul>
      </nav>
      <div className="space-y-3 border-t border-border p-4">
        <Label htmlFor="language-select">{t("common.language")}</Label>
        <Select
          value={i18n.language}
          onValueChange={(lang) => {
            if (lang) changeLanguage(lang);
          }}
        >
          <SelectTrigger id="language-select" className="w-full" aria-label={t("common.language")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="zh-CN">中文</SelectItem>
            <SelectItem value="en-US">English</SelectItem>
          </SelectContent>
        </Select>
        <div className="flex gap-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={toggleTheme}
            className="flex-1"
            aria-label={t("common.theme")}
          >
            {isDark ? <MoonIcon className="h-4 w-4" /> : <SunIcon className="h-4 w-4" />}
          </Button>
          <Button variant="destructive" onClick={logout} className="flex-[2]">
            {t("common.logout")}
          </Button>
        </div>
      </div>
    </>
  );

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-background md:flex-row">
      <div className="flex items-center justify-between border-b border-border bg-card px-4 py-3 md:hidden">
        <span className="text-xl font-bold">ProxyPanel</span>
        <Sheet open={isMenuOpen} onOpenChange={setIsMenuOpen}>
          <SheetTrigger
            render={<Button variant="ghost" size="icon" aria-label={t("common.openMenu")} />}
          >
            <Bars3Icon className="h-6 w-6" />
          </SheetTrigger>
          <SheetContent side="left" showCloseButton={false} className="w-64 gap-0 p-0 md:hidden">
            <SheetHeader className="flex-row items-center justify-between p-6">
              <SheetTitle className="text-2xl font-bold">ProxyPanel</SheetTitle>
              <SheetClose
                render={
                  <Button variant="ghost" size="icon-sm" aria-label={t("common.closeMenu")} />
                }
              >
                <XMarkIcon className="h-5 w-5" />
              </SheetClose>
            </SheetHeader>
            {renderNavigation()}
          </SheetContent>
        </Sheet>
      </div>

      <aside className="hidden w-64 flex-col border-r border-border bg-card md:flex">
        <div className="p-6">
          <h1 className="text-2xl font-bold">ProxyPanel</h1>
        </div>
        {renderNavigation()}
      </aside>

      <main className="flex-1 overflow-y-auto p-6">{children}</main>
    </div>
  );
}
