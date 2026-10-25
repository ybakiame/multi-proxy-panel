import {
  HomeIcon,
  ServerIcon,
  CogIcon,
  LinkIcon,
  GlobeAltIcon,
  UsersIcon,
  UserGroupIcon,
  NewspaperIcon,
  ChartBarIcon,
  DocumentTextIcon,
  KeyIcon,
  BellIcon,
  SignalIcon,
  ArrowsRightLeftIcon,
  CpuChipIcon,
  LockClosedIcon,
  ArrowPathRoundedSquareIcon,
  WrenchIcon,
} from "@heroicons/react/24/outline";

export interface NavItem {
  path: string;
  labelKey: string;
  icon: typeof HomeIcon;
  section: NavSectionKey;
}

export type NavSectionKey = "overview" | "infrastructure" | "access" | "observability" | "system";

export const navItems: NavItem[] = [
  { path: "/", labelKey: "nav.dashboard", icon: HomeIcon, section: "overview" },
  { path: "/nodes", labelKey: "nav.nodes", icon: ServerIcon, section: "infrastructure" },
  { path: "/protocols", labelKey: "nav.protocols", icon: CogIcon, section: "infrastructure" },
  { path: "/bindings", labelKey: "nav.bindings", icon: LinkIcon, section: "infrastructure" },
  { path: "/hosts", labelKey: "nav.hosts", icon: GlobeAltIcon, section: "infrastructure" },
  {
    path: "/relay-rules",
    labelKey: "nav.relayRules",
    icon: ArrowPathRoundedSquareIcon,
    section: "infrastructure",
  },
  {
    path: "/core-versions",
    labelKey: "nav.coreVersions",
    icon: CpuChipIcon,
    section: "infrastructure",
  },
  {
    path: "/certificates",
    labelKey: "nav.certificates",
    icon: LockClosedIcon,
    section: "infrastructure",
  },
  { path: "/clients", labelKey: "nav.clients", icon: UsersIcon, section: "access" },
  { path: "/groups", labelKey: "nav.groups", icon: UserGroupIcon, section: "access" },
  {
    path: "/subscriptions",
    labelKey: "nav.subscriptions",
    icon: NewspaperIcon,
    section: "access",
  },
  { path: "/metrics", labelKey: "nav.metrics", icon: ChartBarIcon, section: "observability" },
  {
    path: "/traffic",
    labelKey: "nav.traffic",
    icon: ArrowsRightLeftIcon,
    section: "observability",
  },
  { path: "/onlines", labelKey: "nav.onlines", icon: SignalIcon, section: "observability" },
  { path: "/logs", labelKey: "nav.logs", icon: DocumentTextIcon, section: "observability" },
  { path: "/api-keys", labelKey: "nav.apiKeys", icon: KeyIcon, section: "system" },
  { path: "/webhooks", labelKey: "nav.webhooks", icon: BellIcon, section: "system" },
  { path: "/system", labelKey: "nav.system", icon: WrenchIcon, section: "system" },
];

export const navSections: { key: NavSectionKey; labelKey: string }[] = [
  { key: "overview", labelKey: "navSection.overview" },
  { key: "infrastructure", labelKey: "navSection.infrastructure" },
  { key: "access", labelKey: "navSection.access" },
  { key: "observability", labelKey: "navSection.observability" },
  { key: "system", labelKey: "navSection.system" },
];
