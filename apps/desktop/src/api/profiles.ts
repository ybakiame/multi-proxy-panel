/**
 * Profile (override template) API types and functions.
 *
 * Aligned with Rust-side `ProfileView`, `ProfileDetailView`, etc.
 */

import { invoke } from "@tauri-apps/api/core";
import type { CoreType } from "./types";

/** Profile list view (aligned with Rust `ProfileView`). */
export interface ProfileView {
  id: string;
  name: string;
  /** Core type: `singbox` / `mihomo`. */
  core_type: CoreType;
  /** YAML override byte count (for list display). */
  yaml_bytes: number;
  /** JS override byte count (for list display). */
  js_bytes: number;
  /** Remote YAML override URL (`null` = not configured). */
  yaml_url: string | null;
  /** Remote JS override URL (`null` = not configured). */
  js_url: string | null;
}

/** Profile detail view (with override content, aligned with Rust `ProfileDetailView`). */
export interface ProfileDetailView {
  id: string;
  name: string;
  core_type: CoreType;
  /** YAML deep-merge override (RFC 7386 style; empty = not enabled). */
  yaml_override: string;
  /** JS override (sync pure function `function main(config){...; return config}`; empty = not enabled). */
  js_override: string;
  /** Remote YAML override URL (`null` = not configured). */
  yaml_url: string | null;
  /** Remote JS override URL (`null` = not configured). */
  js_url: string | null;
}

/** Input for creating a new profile. */
export interface CreateProfileInput {
  name: string;
  core_type: CoreType;
}

/** Input for updating a profile (rejected by command layer on YAML/JS override and remote URL validation failure). */
export interface UpdateProfileInput {
  id: string;
  name: string;
  yaml_override: string;
  js_override: string;
  /** Remote YAML override URL (empty = not configured). */
  yaml_url: string;
  /** Remote JS override URL (empty = not configured). */
  js_url: string;
}

/** List all override templates. */
export function listProfiles(): Promise<ProfileView[]> {
  return invoke<ProfileView[]>("list_profiles");
}

/** Create a new override template (duplicate name errors are propagated from command layer). */
export function createProfile(input: CreateProfileInput): Promise<ProfileView> {
  return invoke<ProfileView>("create_profile", { input });
}

/** Read a single profile detail (including YAML / JS override content). */
export function getProfile(id: string): Promise<ProfileDetailView> {
  return invoke<ProfileDetailView>("get_profile", { id });
}

/** Update editable fields of a profile (name / yaml_override / js_override). */
export function updateProfile(input: UpdateProfileInput): Promise<void> {
  return invoke<void>("update_profile", { input });
}

/** Delete an override template. */
export function deleteProfile(id: string): Promise<void> {
  return invoke<void>("delete_profile", { id });
}

/**
 * Generate core config preview (according to current client core type; sing-box = JSON, mihomo = YAML text).
 * Pass `subscriptionId` to preview by specified subscription (ignores enabled state);
 * omit / pass `null` to generate by current effective subscription
 * (falls back to legacy Hub subscription path when no subscription selected, no override).
 */
export function previewCoreConfig(subscriptionId?: string | null): Promise<string> {
  return invoke<string>("preview_core_config", { subscriptionId: subscriptionId ?? null });
}
