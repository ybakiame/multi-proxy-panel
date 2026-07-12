import { useState } from "react";
import { Button, Select, ListBox, Label } from "@heroui/react";
import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { navItems } from "./nav";
import { useAuth } from "../context/AuthContext";
import { useSettingsStore } from "../stores/settings";
import { SunIcon, MoonIcon, Bars3Icon, XMarkIcon } from "@heroicons/react/24/outline";

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

  return (
    <div className="flex h-screen w-screen flex-col md:flex-row overflow-hidden bg-background">
      <div className="md:hidden flex items-center justify-between px-4 py-3 border-b border-border bg-card">
        <span className="font-bold text-xl">ProxyPanel</span>
        <button
          type="button"
          className="p-2 rounded-md hover:bg-muted"
          onClick={() => setIsMenuOpen(!isMenuOpen)}
          aria-label={isMenuOpen ? t("common.closeMenu") : t("common.openMenu")}
        >
          {isMenuOpen ? <XMarkIcon className="h-6 w-6" /> : <Bars3Icon className="h-6 w-6" />}
        </button>
      </div>

      <aside
        className={`${
          isMenuOpen ? "block" : "hidden"
        } md:flex w-full md:w-64 flex-col border-r border-border bg-card`}
      >
        <div className="p-6">
          <h1 className="text-2xl font-bold">ProxyPanel</h1>
        </div>
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
        <div className="border-t border-border p-4 space-y-3">
          <Select value={i18n.language} onChange={(value) => changeLanguage(value as string)}>
            <Label>{t("common.language")}</Label>
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                <ListBox.Item id="zh-CN" textValue="中文">
                  中文
                </ListBox.Item>
                <ListBox.Item id="en-US" textValue="English">
                  English
                </ListBox.Item>
              </ListBox>
            </Select.Popover>
          </Select>
          <div className="flex gap-2">
            <Button
              isIconOnly
              variant="ghost"
              onPress={toggleTheme}
              className="flex-1"
              aria-label={t("common.theme")}
            >
              {isDark ? <MoonIcon className="h-4 w-4" /> : <SunIcon className="h-4 w-4" />}
            </Button>
            <Button variant="danger" onPress={logout} className="flex-[2]">
              {t("common.logout")}
            </Button>
          </div>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto p-6">{children}</main>
    </div>
  );
}
