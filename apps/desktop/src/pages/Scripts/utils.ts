/**
 * Shared types, constants, and utility functions for the Scripts page.
 */

import type { ArgSpecView } from "../../api";

/** Dialect options for remote resource add form (Surge / Loon only). */
export const REMOTE_DIALECT_OPTIONS = [
  { id: "Surge", label: "Surge" },
  { id: "Loon", label: "Loon" },
] as const;

/** Dialect options for config import (lowercase, matching command values). */
export const IMPORT_DIALECT_OPTIONS = [
  { id: "surge", label: "Surge" },
  { id: "loon", label: "Loon" },
] as const;

/** Argument edit state for the add form. */
export interface ArgEdit {
  key: string;
  default_value: string;
  description: string | null;
  kind: "Input" | "Select";
  options: string[];
  tag: string | null;
  value: string;
}

/** Argument value edit state for the edit modal. */
export interface ArgValueEdit extends ArgSpecView {
  value: string;
}

/** Normalize detect_remote kind string to option value. */
export function normalizeKind(kind: string | null): string | null {
  if (!kind) return null;
  const lower = kind.trim().toLowerCase();
  if (lower === "script") return "Script";
  if (lower === "snippet") return "Snippet";
  return kind.trim();
}

/** Normalize dialect string: QuantumultX legacy data -> Loon. */
export function normalizeDialect(dialect: string | null | undefined): string | null {
  if (!dialect) return null;
  const trimmed = dialect.trim();
  if (trimmed === "QuantumultX" || trimmed.toLowerCase() === "quantumultx") return "Loon";
  return trimmed;
}

/** Derive resource name from URL (strip common suffixes). */
export function deriveNameFromUrl(url: string): string {
  try {
    const parsed = new URL(url);
    const segments = parsed.pathname.split("/").filter(Boolean);
    const last = segments[segments.length - 1];
    if (last) {
      const stem = last.replace(/\.(js|conf|sgmodule|plugin|loon)$/i, "");
      if (stem.trim() !== "") return stem;
    }
  } catch {
    // fallback to raw url
  }
  return url;
}

/** Format RFC3339 timestamp to local readable string. */
export function formatTime(iso: string | null): string {
  if (!iso) return "-";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/** Format interval seconds to human-readable text. */
export function formatInterval(secs: number): string {
  if (secs % 86400 === 0) return `${secs / 86400} 天`;
  if (secs % 3600 === 0) return `${secs / 3600} 小时`;
  return `${secs} 秒`;
}

/** Group arguments by tag. */
export function groupArgsByTag<T extends { key: string; tag: string | null }>(
  args: T[],
): { tag: string | null; args: T[] }[] {
  const groups = new Map<string | null, T[]>();
  for (const arg of args) {
    const tag = arg.tag ?? null;
    const group = groups.get(tag) ?? [];
    group.push(arg);
    groups.set(tag, group);
  }
  return Array.from(groups.entries()).map(([tag, items]) => ({ tag, args: items }));
}
