/**
 * Shared types for the RemotesTab split.
 */

import type { ArgSpecView } from "../../../api";

/** Form state shared between add and edit modals. */
export interface RemoteFormState {
  name: string;
  description: string;
  url: string;
  kind: string;
  dialect: string;
  interval: string;
}

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

/** Result produced by the remote-sniff hook. */
export interface SniffResult {
  kind: string | null;
  dialect: string | null;
  name: string | null;
  description: string | null;
  icon: string | null;
  arguments: ArgSpecView[];
  info: string | null;
}
