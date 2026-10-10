import type { ReactNode, Ref } from "react";
import { IS_MOBILE } from "../platform";

interface ShellProps {
  children: ReactNode;
  sidebar: ReactNode;
  navigation: ReactNode;
  showNavigation: boolean;
  mainRef?: Ref<HTMLElement>;
}
/** 平台形态只在组件层分叉：桌面侧栏，移动固定底栏与安全区。 */
export function Shell({ children, sidebar, navigation, showNavigation, mainRef }: ShellProps) {
  if (IS_MOBILE)
    return (
      <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
        <main
          ref={mainRef}
          className={`min-h-0 flex-1 overflow-y-auto ${showNavigation ? "pb-[calc(6rem+env(safe-area-inset-bottom))]" : ""}`}
        >
          {children}
        </main>
        {showNavigation && navigation}
      </div>
    );
  return (
    <div className="flex h-full min-h-screen bg-background text-foreground">
      {sidebar}
      <main ref={mainRef} className="min-w-0 flex-1 overflow-y-auto p-4 lg:p-6">
        {children}
      </main>
    </div>
  );
}
