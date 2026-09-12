/**
 * TanStack Query queryKey 集中定义（见 .agents/rules/client-state-management.md）。
 *
 * 同一数据一个 queryKey、一处定义：新增需要缓存的服务端/命令数据时在此登记，
 * 页面与组件只引用这里的常量（含参数化 key 的工厂函数），
 * 禁止在组件内各自写字面量 key。
 */

export const CAPABILITIES_KEY = ["capabilities"] as const;

export const CONFIG_KEY = ["config"] as const;
export const PROXY_STATUS_KEY = ["proxy_status"] as const;
export const TRAFFIC_KEY = ["traffic"] as const;
export const MITM_CA_KEY = ["mitmCa"] as const;

export const REMOTES_KEY = ["remotes"] as const;
export const TASKS_KEY = ["tasks"] as const;

export const LOCAL_OVERRIDE_KEY = ["localOverride"] as const;

export const PROFILES_KEY = ["profiles"] as const;
/** 单个覆写模板详情（Override 页，按选中 id 参数化）。 */
export const profileKey = (id: string | null) => ["profile", id] as const;

export const SUBSCRIPTIONS_KEY = ["subscriptions"] as const;
/** 单个订阅的静态节点 tag 列表（订阅缓存读取，按订阅 id 参数化）。 */
export const subscriptionNodeTagsKey = (id: string) => ["subscription_node_tags", id] as const;
export const CORES_KEY = ["cores"] as const;
export const CORES_LIST_KEY = ["cores_list"] as const;
export const REMOTE_VERSIONS_KEY = ["remote_core_versions"] as const;
export const VPN_ERROR_KEY = ["vpnLastError"] as const;
/** sing-box 核心版本（Android，经 libbox.Version() 读取）。 */
export const CORE_VERSION_KEY = ["core_version"] as const;

export const PLATFORM_KEY = ["platform_info"] as const;
export const TUN_AUTH_KEY = ["tun_auth"] as const;
export const NOTIF_PERM_KEY = ["notification_permission"] as const;

export const PROXIES_KEY = ["proxies_list"] as const;
export const CONNECTIONS_KEY = ["connections_active"] as const;
export const CLOSED_CONNECTIONS_KEY = ["connections_closed"] as const;

export const LOGS_KEY = ["logs"] as const;
export const LOG_FILES_KEY = ["log_files"] as const;

export const CONFIG_PREVIEW_KEY = ["config_preview"] as const;

export const CONFIG_SLICES_KEY = ["config_slices"] as const;

/** 内置 CN 分流基线只读视图（静态数据，staleTime Infinity）。 */
export const BASELINE_VIEW_KEY = ["baseline_view"] as const;
