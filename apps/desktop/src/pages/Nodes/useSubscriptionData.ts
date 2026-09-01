import { useCallback, useEffect, useState } from "react";
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

export type OpResult = { sub: SubscriptionView; kind: "add" | "refresh" };

export function useSubscriptionData() {
  const [subs, setSubs] = useState<SubscriptionView[]>([]);
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<OpResult | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  const refreshSubs = useCallback(async () => {
    try {
      setSubs(await listSubscriptions());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      setProfiles(await listProfiles());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshSubs();
    void loadProfiles();
  }, [refreshSubs, loadProfiles]);

  const handleAdd = async (name: string, url: string, ua: string, profileId: string | null) => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const sub = await addSubscription({
        name: name.trim(),
        url: url.trim(),
        user_agent: ua.trim() || undefined,
        profile_id: profileId,
      });
      setResult({ sub, kind: "add" });
      setError(null);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await removeSubscription(id);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (sub: SubscriptionView, onSuccess?: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await setSubscriptionEnabled(sub.id, !sub.enabled);
      await refreshSubs();
      await onSuccess?.();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRefresh = async (id: string) => {
    setRefreshingId(id);
    setError(null);
    setResult(null);
    try {
      const sub = await refreshSubscription(id);
      setResult({ sub, kind: "refresh" });
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setRefreshingId(null);
    }
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
    try {
      await updateSubscription(sub.id, name.trim(), url.trim(), profileId, userAgent);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return {
    subs,
    profiles,
    busy,
    error,
    result,
    refreshingId,
    refreshSubs,
    loadProfiles,
    handleAdd,
    handleRemove,
    handleToggle,
    handleRefresh,
    handleEditSave,
    setError,
    setResult,
  };
}
