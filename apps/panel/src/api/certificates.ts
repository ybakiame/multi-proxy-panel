import { get, post, del } from "./client";
import type { ManagedCertificate } from "./types";

export const getCertificates = (nodeId?: string) =>
  get<{ certificates: ManagedCertificate[] }>(
    `/api/v1/certificates${nodeId ? `?node_id=${encodeURIComponent(nodeId)}` : ""}`,
  ).then((res) => res.certificates);

export const createCertificate = (payload: { domain: string; node_id: string }) =>
  post<ManagedCertificate>("/api/v1/certificates", payload);

export const renewCertificate = (id: string) =>
  post<ManagedCertificate>(`/api/v1/certificates/${id}/renew`, {});

export const deleteCertificate = (id: string) => del(`/api/v1/certificates/${id}`);
