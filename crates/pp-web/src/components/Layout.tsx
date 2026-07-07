import { useState } from "react";
import {
  Navbar,
  NavbarBrand,
  NavbarContent,
  NavbarMenuToggle,
  Button,
  Select,
  SelectItem,
} from "@heroui/react";
import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";
import { navItems } from "./nav";
import { useAuth } from "../context/AuthContext";
import { SunIcon, MoonIcon } from "@heroicons/react/24/outline";

export function Layout({ children }: { children: React.ReactNode }) {
  const { t, i18n } = useTranslation();
  const { logout } = useAuth();
  const location = useLocation();
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [isDark, setIsDark] = useState(true);

  const changeLanguage = (lang: string) => {
    i18n.changeLanguage(lang);
  };

  const toggleTheme = () => {
    setIsDark(!isDark);
    document.documentElement.classList.toggle("dark", !isDark);
  };

  return (
    <div className="flex h-screen w-screen flex-col md:flex-row overflow-hidden bg-background">
      <div className="md:hidden">
        <Navbar isMenuOpen={isMenuOpen} onMenuOpenChange={setIsMenuOpen}>
          <NavbarBrand>
            <span className="font-bold text-xl">ProxyPanel</span>
          </NavbarBrand>
          <NavbarContent justify="end">
            <NavbarMenuToggle />
          </NavbarContent>
        </Navbar>
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
          <Select
            label={t("common.language")}
            selectedKeys={[i18n.language]}
            onSelectionChange={(keys) => changeLanguage(Array.from(keys)[0] as string)}
            size="sm"
          >
            <SelectItem key="zh-CN">中文</SelectItem>
            <SelectItem key="en-US">English</SelectItem>
          </Select>
          <div className="flex gap-2">
            <Button isIconOnly variant="flat" onPress={toggleTheme} className="flex-1">
              {isDark ? <MoonIcon className="h-4 w-4" /> : <SunIcon className="h-4 w-4" />}
            </Button>
            <Button color="danger" variant="flat" onPress={logout} className="flex-[2]">
              {t("common.logout")}
            </Button>
          </div>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto p-6">
        {children}
      </main>
    </div>
  );
}
