import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  createNode,
  updateNode,
  deleteNode,
  pushConfig,
  deleteCoreBinary,
  getCoreBinaries,
  pushPendingUpdates,
  type CreateNodePayload,
  type UpdateNodePayload,
} from "../../api/nodes";

const nodesQueryKey = "nodes";
const pendingQueryKey = "pending-updates";

export interface NodeFormState {
  name: string;
  domain: string;
  usage_coefficient: number;
  labels: string;
  parent_id: string;
}

export const defaultFormState: NodeFormState = {
  name: "",
  domain: "",
  usage_coefficient: 1,
  labels: "{}",
  parent_id: "",
};

export function useNodeActions() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const createMutation = useMutation({
    mutationFn: createNode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; data: UpdateNodePayload }) =>
      updateNode(payload.id, payload.data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteNode,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const pushMutation = useMutation({
    mutationFn: (payload: { id: string; config: Record<string, unknown> }) =>
      pushConfig(payload.id, payload.config),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const pushAllMutation = useMutation({
    mutationFn: pushPendingUpdates,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const deleteBinaryMutation = useMutation({
    mutationFn: (payload: { nodeId: string; fileName: string }) =>
      deleteCoreBinary(payload.nodeId, payload.fileName),
    onSuccess: (_, payload) => {
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey, payload.nodeId, "binaries"] });
      return getCoreBinaries(payload.nodeId);
    },
  });

  const buildCreatePayload = (form: NodeFormState): CreateNodePayload => {
    let labels: Record<string, string> = {};
    try {
      labels = JSON.parse(form.labels);
    } catch {
      // ignore parse error
    }
    return {
      name: form.name,
      domain: form.domain || undefined,
      usage_coefficient: form.usage_coefficient,
      labels,
      parent_id: form.parent_id || undefined,
    };
  };

  const buildUpdatePayload = (form: NodeFormState): UpdateNodePayload => {
    let labels: Record<string, string> = {};
    try {
      labels = JSON.parse(form.labels);
    } catch {
      // ignore parse error
    }
    return {
      name: form.name,
      domain: form.domain || undefined,
      usage_coefficient: form.usage_coefficient,
      labels,
      parent_id: form.parent_id || undefined,
    };
  };

  return {
    createMutation,
    updateMutation,
    deleteMutation,
    pushMutation,
    pushAllMutation,
    deleteBinaryMutation,
    buildCreatePayload,
    buildUpdatePayload,
    t,
    nodesQueryKey,
    pendingQueryKey,
  };
}
