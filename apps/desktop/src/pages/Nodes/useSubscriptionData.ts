import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  addSubscription,
  listProfiles,
  listSubscriptions,
  refreshSubscription,
  removeSubscription,
  setSubscriptionEnabled,
  toErrorMessage,
  updateSubscription,
} from "../../api";
import type { ProfileView, SubscriptionView } from "../../api";
import { PROFILES_KEY, SUBSCRIPTIONS_KEY } from "../../api/keys";

export type OpResult = { sub: SubscriptionView; kind: "add" | "refresh" };

export function useSubscriptionData() {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<OpResult | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  const { data: subs = [] } = useQuery<SubscriptionView[]>({
    queryKey: SUBSCRIPTIONS_KEY,
    queryFn: listSubscriptions,
    refetchInterval: 5000,
  });

  const { data: profiles = [] } = useQuery<ProfileView[]>({
    queryKey: PROFILES_KEY,
    queryFn: listProfiles,
  });

  const refreshSubs = async () => {
    await queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
  };

  const addMutation = useMutation({
    mutationFn: async (input: { name: string; url: string; ua: string; profileId: string | null }) => {
      return addSubscription({
        name: input.name.trim(),
        url: input.url.trim(),
        user_agent: input.ua.trim() || undefined,
        profile_id: input.profileId,
      });
    },
    onSuccess: (sub) => {
      setResult({ sub, kind: "add" });
      setError(null);
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setError(toErrorMessage(err));
    },
    onSettled: () => setBusy(false),
  });

  const removeMutation = useMutation({
    mutationFn: async (id: string) => {
      await removeSubscription(id);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setError(toErrorMessage(err));
    },
    onSettled: () => setBusy(false),
  });

  const toggleMutation = useMutation({
    mutationFn: async ({ id, enabled }: { id: string; enabled: boolean }) => {
      await setSubscriptionEnabled(id, enabled);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setError(toErrorMessage(err));
    },
    onSettled: () => setBusy(false),
  });

  const refreshMutation = useMutation({
    mutationFn: async (id: string) => {
      return refreshSubscription(id);
    },
    onSuccess: (sub) => {
      setResult({ sub, kind: "refresh" });
      setError(null);
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setError(toErrorMessage(err));
    },
    onSettled: () => setRefreshingId(null),
  });

  const editMutation = useMutation({
    mutationFn: async ({
      id,
      name,
      url,
      profileId,
      userAgent,
    }: {
      id: string;
      name: string;
      url: string;
      profileId: string | null;
      userAgent?: string;
    }) => {
      return updateSubscription(id, name.trim(), url.trim(), profileId, userAgent);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setError(toErrorMessage(err));
    },
    onSettled: () => setBusy(false),
  });

  const handleAdd = async (name: string, url: string, ua: string, profileId: string | null) => {
    setBusy(true);
    setError(null);
    setResult(null);
    addMutation.mutate({ name, url, ua, profileId });
  };

  const handleRemove = async (id: string) => {
    setBusy(true);
    setError(null);
    removeMutation.mutate(id);
  };

  const handleToggle = async (sub: SubscriptionView, onSuccess?: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    toggleMutation.mutate(
      { id: sub.id, enabled: !sub.enabled },
      {
        onSuccess: () => {
          void onSuccess?.();
        },
      },
    );
  };

  const handleRefresh = async (id: string) => {
    setRefreshingId(id);
    setError(null);
    setResult(null);
    refreshMutation.mutate(id);
  };

  const handleEditSave = async (
    sub: SubscriptionView,
    name: string,
    url: string,
    profileId: string | null,
    userAgent?: string,
  ) => {
    setBusy(true);
    setError(null);
    editMutation.mutate({ id: sub.id, name, url, profileId, userAgent });
  };

  return {
    subs,
    profiles,
    busy,
    error,
    result,
    refreshingId,
    refreshSubs,
    handleAdd,
    handleRemove,
    handleToggle,
    handleRefresh,
    handleEditSave,
    setError,
    setResult,
  };
}
