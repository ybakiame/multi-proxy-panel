import { useEffect, useRef } from "react";
import { HashRouter, useLocation } from "react-router-dom";
import { isTauriEnv } from "@pp/client-core";
import { IS_MOBILE, Shell } from "@pp/ui";
import { ThemeProvider } from "./theme";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { DesktopSidebar } from "./layout/desktop/DesktopSidebar";
import { TABS, TabBar } from "./components/mobile/TabBar";
import { Toaster } from "./components/Toaster";
import { RestartPrompt as DesktopRestartPrompt } from "./components/desktop/RestartPrompt";
import { RestartPrompt as MobileRestartPrompt } from "./components/mobile/RestartPrompt";
import { AppRoutes } from "./routes";

function AppContent() {
  const { pathname } = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const showNavigation = TABS.some((tab) => tab.to === pathname);
  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [pathname]);
  return (
    <>
      <Toaster />
      <Shell
        mainRef={mainRef}
        sidebar={IS_MOBILE ? null : <DesktopSidebar />}
        navigation={IS_MOBILE ? <TabBar /> : null}
        showNavigation={showNavigation}
      >
        <AppRoutes />
      </Shell>
      {IS_MOBILE ? <MobileRestartPrompt /> : <DesktopRestartPrompt />}
    </>
  );
}
export default function App() {
  if (!isTauriEnv)
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4 bg-background p-6 text-foreground">
        <h1 className="text-xl font-semibold">请在客户端内运行</h1>
        <p className="max-w-md text-center text-sm text-muted">
          当前页面缺少 Tauri 运行环境，请使用 {IS_MOBILE ? "bun run tauri android dev" : "bun run tauri dev"}{" "}
          启动客户端。
        </p>
      </div>
    );
  return (
    <HashRouter>
      <ThemeProvider>
        <ErrorBoundary>
          <AppContent />
        </ErrorBoundary>
      </ThemeProvider>
    </HashRouter>
  );
}
