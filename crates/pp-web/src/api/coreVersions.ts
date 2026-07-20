import { get, post, del } from "./client";
import type { CoreVersion } from "./types";

export interface UpstreamVersion {
  version: string;
  channel: string;
  saved: boolean;
}

export interface UpstreamCore {
  core_type: string;
  versions: UpstreamVersion[];
}

export interface SaveVersionItem {
  core_type: string;
  version: string;
  channel: string;
}

export const getCoreVersions = (coreType?: string) =>
  get<{ versions: CoreVersion[] }>(
    `/api/v1/core-versions${coreType ? `?core_type=${encodeURIComponent(coreType)}` : ""}`,
  ).then((res) => res.versions);

export const getUpstreamCoreVersions = () =>
  get<{ cores: UpstreamCore[] }>("/api/v1/core-versions/upstream").then((res) => res.cores);

export const saveCoreVersions = (versions: SaveVersionItem[]) =>
  post<{ added: number }>("/api/v1/core-versions", { versions });

export const deleteCoreVersion = (id: string) => del(`/api/v1/core-versions/${id}`);
