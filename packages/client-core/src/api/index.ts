/**
 * 客户端 Tauri API 层（客户端核心库 api 目录入口）。
 *
 * 聚合导出各领域 API 模块（types + functions），供包根统一导出
 * （`packages/client-core/src/index.ts`）后由消费方经 `@pp/client-core` 引用。
 */

// ---------------------------------------------------------------------------
// Error helper
// ---------------------------------------------------------------------------

/** Normalize Tauri command rejection value to readable error message. */
export function toErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

// ---------------------------------------------------------------------------
// Core types & config
// ---------------------------------------------------------------------------

export type { MitmScriptDialect, ClientConfig, ClientStatus, SaveConfigView } from "./types";
export { getConfig, saveConfig, startProxy, stopProxy, proxyStatus, setRuleMode, listTraffic, getMitmCa } from "./core";
export type { TrafficRecord, MitmCaView } from "./core";

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

export type { SubscriptionUserInfo, SubscriptionFormat, SubscriptionView, AddSubscriptionInput } from "./subscriptions";
export {
  listSubscriptions,
  addSubscription,
  removeSubscription,
  setSubscriptionEnabled,
  setActiveSubscription,
  refreshSubscription,
  updateSubscription,
} from "./subscriptions";

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

export type { ProfileView, ProfileDetailView, CreateProfileInput, UpdateProfileInput } from "./profiles";
export { listProfiles, createProfile, getProfile, updateProfile, deleteProfile, previewCoreConfig } from "./profiles";

// ---------------------------------------------------------------------------
// Remotes
// ---------------------------------------------------------------------------

export type {
  RemoteKind,
  RemoteResource,
  FetchReport,
  ArgSpecView,
  ConfigMetaView,
  DetectRemoteView,
  ImportSummary,
} from "./remotes";
export {
  listRemotes,
  addRemote,
  updateRemote,
  detectRemote,
  getRemoteIcon,
  removeRemote,
  fetchRemotes,
  importConfig,
} from "./remotes";

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

export type { TaskScriptView } from "./tasks";
export { listTasks, runTask } from "./tasks";

// ---------------------------------------------------------------------------
// Cores
// ---------------------------------------------------------------------------

export type { CoreSource, LocalCoreView } from "./cores";
export {
  listCores,
  listRemoteCoreVersions,
  listDownloadedVersions,
  downloadCore,
  setActiveCore,
  deleteCore,
  detectSystemCores,
} from "./cores";

// ---------------------------------------------------------------------------
// Proxies
// ---------------------------------------------------------------------------

export type { GroupView, NodeView, ProxyList, DelayResult } from "./proxies";
export { proxiesList, proxiesSelect, proxiesTestDelay, proxiesTestGroup } from "./proxies";

// ---------------------------------------------------------------------------
// Connections
// ---------------------------------------------------------------------------

export type { ConnectionView, ActiveConnections } from "./connections";
export { connectionsActive, connectionsClosed, connectionsClose } from "./connections";

// ---------------------------------------------------------------------------
// Local Override
// ---------------------------------------------------------------------------

export type {
  LocalRuleView,
  LocalRuleSetRefView,
  CoreLocalOverrideView,
  AppliedTemplateView,
  CustomRuleSetView,
  CustomRuleSetInput,
  CustomRuleSetSource,
  RuleSetUpdateOutcome,
  CustomTemplateView,
  CustomTemplateInput,
  LocalOverrideView,
  SaveLocalOverrideInput,
  CoreLocalOverrideInput,
  LocalRuleInput,
  LocalRuleSetRefInput,
  AppliedTemplateInput,
} from "./localOverride";
export {
  localOverrideGet,
  localOverrideSave,
  localOverrideApplyTemplate,
  localOverrideRevertTemplate,
  localOverrideUpdateRulesetsNow,
  localOverrideUpdateRuleSet,
} from "./localOverride";

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

export type { LogEntry } from "./logs";
export { getLogs, exportLogs, clearLogs, logFrontend, openExportDir, listLogFiles, readLogFileTail } from "./logs";

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

export type { Capabilities, PlatformInfo } from "./system";
export {
  getCapabilities,
  platformInfo,
  requestVpnPermission,
  vpnLastError,
  coreVersion,
  notifyPrefsChanged,
  tunAuthStatus,
  authorizeTun,
  gpuAcceleration,
  toastModeOverride,
  testGithubProxy,
} from "./system";
