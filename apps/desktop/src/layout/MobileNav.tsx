import { useState } from "react";
import { Bars3Icon } from "@heroicons/react/24/outline";
import { Button, Drawer } from "@heroui/react";
import clsx from "clsx";
import { NavLink } from "react-router-dom";
import { useCapabilities } from "../hooks/useCapabilities";
import { NAV_ITEMS } from "./nav";

function visibleNavItems(capabilities: ReturnType<typeof useCapabilities>["data"]) {
  if (!capabilities) return NAV_ITEMS;
  const caps = capabilities.capabilities;
  return NAV_ITEMS.filter((item) => {
    if (item.requiresMitm && !caps.mitm) return false;
    if (item.requiresScriptsRemote && !caps.scripts_remote) return false;
    return true;
  });
}

/**
 * 移动端导航：`lg`（1024px）以下显示的顶栏（汉堡按钮 + 应用名）与
 * 左侧滑出的导航抽屉。抽屉为受控状态，点击导航项后自动关闭；
 * 路由高亮与桌面侧栏保持一致。
 */
export function MobileNav() {
  const [isOpen, setIsOpen] = useState(false);
  const { data: capabilities } = useCapabilities();
  const items = visibleNavItems(capabilities);

  return (
    <>
      <div
        className="sticky top-0 z-40 flex min-h-14 shrink-0 items-center gap-3 border-b border-border/60 bg-surface px-4 lg:hidden"
        style={{ paddingTop: "env(safe-area-inset-top)" }}
      >
        <Button variant="ghost" isIconOnly aria-label="打开导航菜单" className="-ml-1" onPress={() => setIsOpen(true)}>
          <Bars3Icon className="size-6" />
        </Button>
        <span className="text-sm font-semibold">ProxyPanel</span>
      </div>

      <Drawer.Backdrop isOpen={isOpen} onOpenChange={setIsOpen}>
        <Drawer.Content placement="left">
          <Drawer.Dialog
            style={{
              paddingTop: "env(safe-area-inset-top)",
              paddingBottom: "env(safe-area-inset-bottom)",
            }}
          >
            <Drawer.CloseTrigger />
            <Drawer.Header>
              <Drawer.Heading>ProxyPanel</Drawer.Heading>
            </Drawer.Header>
            <Drawer.Body>
              <nav className="flex flex-col gap-1" aria-label="主导航">
                {items.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    end={item.to === "/"}
                    onClick={() => setIsOpen(false)}
                    className={({ isActive }) =>
                      clsx(
                        "flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors",
                        isActive
                          ? "bg-primary/10 text-primary"
                          : "text-muted hover:bg-surface-secondary hover:text-foreground",
                      )
                    }
                  >
                    <item.icon className="size-4" aria-hidden="true" />
                    {item.label}
                  </NavLink>
                ))}
              </nav>
            </Drawer.Body>
          </Drawer.Dialog>
        </Drawer.Content>
      </Drawer.Backdrop>
    </>
  );
}
