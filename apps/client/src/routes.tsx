import { Navigate, useRoutes, type RouteObject } from "react-router-dom";
import { IS_MOBILE } from "@pp/ui";
import Dashboard from "./pages/Dashboard";
import Subscriptions from "./pages/Subscriptions";
import Config from "./pages/Config";
import Dns from "./pages/Config/Dns";
import Outbounds from "./pages/Config/Outbounds";
import Route from "./pages/Config/Route";
import Rules from "./pages/Config/Rules";
import RuleSets from "./pages/Config/RuleSetsPage";
import RuleSetMarket from "./pages/Config/RuleSetMarket";
import Inbounds from "./pages/Config/Inbounds";
import Experimental from "./pages/Config/ExperimentalPage";
import Settings from "./pages/Settings";
import Logs from "./pages/Logs";
import Stats from "./pages/Stats";
import Proxies from "./pages/Proxies";
import Connections from "./pages/Connections";
import Panel from "./pages/Panel";
import Mitm from "./pages/Mitm";
import Scripts from "./pages/Scripts";
import Tools from "./pages/Tools";
import Override from "./pages/Override";
import VpnNotify from "./pages/Settings/VpnNotifyPage";
import Github from "./pages/Settings/GithubPage";
import DevTools from "./pages/Settings/DevToolsPage";
import Diagnose from "./pages/Settings/DiagnosePage";
import About from "./pages/Settings/AboutPage";

const redirect = (path: string, to: string): RouteObject => ({ path, element: <Navigate to={to} replace /> });
const sharedRoutes: RouteObject[] = [
  { path: "/", element: <Dashboard /> },
  { path: "/subscriptions", element: <Subscriptions /> },
  { path: "/config", element: <Config /> },
  { path: "/config/dns", element: <Dns /> },
  { path: "/config/outbounds", element: <Outbounds /> },
  { path: "/config/route", element: <Route /> },
  { path: "/config/route/rules", element: <Rules /> },
  { path: "/config/route/rulesets", element: <RuleSets /> },
  { path: "/config/route/rulesets/market", element: <RuleSetMarket /> },
  { path: "/config/inbounds", element: <Inbounds /> },
  { path: "/config/experimental", element: <Experimental /> },
  { path: "/settings", element: <Settings /> },
  { path: "/stats", element: <Stats /> },
  { path: "/logs", element: <Logs /> },
  redirect("/nodes", "/subscriptions"),
  redirect("/config/network", "/config/inbounds"),
  redirect("/config/clash-api", "/config/experimental"),
  redirect("/config/rules", "/config/route/rules"),
  redirect("/config/rulesets", "/config/route/rulesets"),
  redirect("/config/rulesets/market", "/config/route/rulesets/market"),
  redirect("/rules", "/config"),
  redirect("/rules/custom", "/config/route/rules"),
  redirect("/rules/rulesets", "/config/route/rulesets"),
  redirect("/rules/rulesets/market", "/config/route/rulesets/market"),
  redirect("/settings/network", "/config/inbounds"),
  redirect("/settings/clash-api", "/config/experimental"),
];
const platformRoutes: RouteObject[] = IS_MOBILE
  ? [
      { path: "/panel", element: <Panel /> },
      redirect("/proxies", "/panel"),
      redirect("/connections", "/panel"),
      { path: "/settings/vpn-notify", element: <VpnNotify /> },
      { path: "/settings/github", element: <Github /> },
      { path: "/settings/dev-tools", element: <DevTools /> },
      { path: "/settings/dev-tools/diagnose", element: <Diagnose /> },
      { path: "/settings/about", element: <About /> },
    ]
  : [
      { path: "/proxies", element: <Proxies /> },
      { path: "/connections", element: <Connections /> },
      { path: "/mitm", element: <Mitm /> },
      { path: "/scripts", element: <Scripts /> },
      { path: "/tools", element: <Tools /> },
      { path: "/override", element: <Override /> },
    ];
export function AppRoutes() {
  return useRoutes([...sharedRoutes, ...platformRoutes, redirect("*", "/")]);
}
