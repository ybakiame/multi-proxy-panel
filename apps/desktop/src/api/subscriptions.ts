/**
 * Subscription-related API types and functions.
 *
 * Aligned with Rust-side `SubscriptionView`, `SubscriptionUserInfoView`, etc.
 */

import { invoke } from "@tauri-apps/api/core";

/** Subscription user info (aligned with Rust `SubscriptionUserInfoView`). */
export interface SubscriptionUserInfo {
  /** Used upload bytes. */
  upload: number | null;
  /** Used download bytes. */
  download: number | null;
  /** Total traffic bytes. */
  total: number | null;
  /** Expiration timestamp (seconds). */
  expire: number | null;
}

/** Subscription content format (sniff result, aligned with Rust `SubFormat`). */
export type SubscriptionFormat = "ShareLinks" | "ClashYaml" | "SingBoxJson";

/** A subscription external view (aligned with Rust `SubscriptionView`). */
export interface SubscriptionView {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  /** Associated override template id (`null` = no override). */
  profile_id: string | null;
  userinfo: SubscriptionUserInfo | null;
  /** Node count from last successful fetch. */
  node_count: number;
  /** Last fetch error message (recorded on failure; does not block existing data display). */
  error: string | null;
  /** Last fetch sniffed subscription format; undefined when not fetched. */
  format?: SubscriptionFormat;
  /** Request User-Agent (default/empty = default clash.meta). */
  user_agent?: string;
}

/** Input for adding a subscription. */
export interface AddSubscriptionInput {
  name: string;
  url: string;
  /** Request User-Agent; default/empty uses default `clash.meta`. */
  user_agent?: string;
  /** Associated override template id (`null` / empty = no override). */
  profile_id?: string | null;
}

export function listSubscriptions(): Promise<SubscriptionView[]> {
  return invoke<SubscriptionView[]>("list_subscriptions");
}

export function addSubscription(input: AddSubscriptionInput): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("add_subscription", { input });
}

export function removeSubscription(id: string): Promise<void> {
  return invoke<void>("remove_subscription", { id });
}

export function setSubscriptionEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_subscription_enabled", { id, enabled });
}

/** Set the active subscription selected on home page (`null` = clear selection). */
export function setActiveSubscription(id: string | null): Promise<void> {
  return invoke<void>("set_active_subscription", { id });
}

export function refreshSubscription(id: string): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("refresh_subscription", { id });
}

/**
 * Update subscription name / url / user_agent / associated override template.
 * `profileId`: template id = associate; `null` / empty = disassociate.
 */
export function updateSubscription(
  id: string,
  name: string,
  url: string,
  profileId: string | null,
  userAgent?: string,
): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("update_subscription", { id, name, url, profileId, userAgent });
}
