import { useEffect } from "react";
import { useTheme } from "@heroui/react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { Sidebar } from "./layout/Sidebar";
import Dashboard from "./pages/Dashboard";
import Mitm from "./pages/Mitm";
import Nodes from "./pages/Nodes";
import Scripts from "./pages/Scripts";
import Settings from "./pages/Settings";

/**
 * 固定为深色主题：HeroUI v3 深色模式由 `<html>` 上的 `dark` / `data-theme="dark"`
 * 驱动（index.html 已静态设置），此处用 `useTheme` 钩子保证持久化后仍为深色。
 */
function ThemeBootstrap() {
  const { setTheme } = useTheme("dark");

  useEffect(() => {
    setTheme("dark");
  }, [setTheme]);

  return null;
}

export default function App() {
  return (
    <HashRouter>
      <ThemeBootstrap />
      <div className="flex h-full min-h-screen bg-background text-foreground">
        <Sidebar />
        <main className="flex-1 overflow-y-auto p-6">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/nodes" element={<Nodes />} />
            <Route path="/mitm" element={<Mitm />} />
            <Route path="/scripts" element={<Scripts />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
