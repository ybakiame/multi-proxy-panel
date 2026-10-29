import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { baseUrl } from "../api/config";
import { getGroups } from "../api/groups";
import type { Client } from "../api/types";
import {
  createSubscription,
  deleteSubscription,
  getSubscriptions,
  updateSubscription,
} from "../api/subscriptions";
import {
  ConfirmDialog,
  CopyableSecret,
  FormCheckbox,
  FormInput,
  FormSelect,
} from "../components/ui";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { Spinner } from "../components/ui/spinner";
import { formatDateTime } from "../utils/format";

const FORMATS = ["base64", "clash", "sing-box", "v2rayng"];

interface ClientSubscriptionDialogProps {
  client: Client | null;
  onClose: () => void;
}

export function ClientSubscriptionDialog({ client, onClose }: ClientSubscriptionDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data: groups = [] } = useQuery({ queryKey: ["groups"], queryFn: getGroups });
  const {
    data: subscription = null,
    isLoading: initialLoading,
    error: queryError,
    refetch,
  } = useQuery({
    queryKey: ["client-subscription", client?.id],
    queryFn: async () => {
      const result = await getSubscriptions(1, 1000);
      return result.data.find((item) => item.client_id === client?.id) ?? null;
    },
    enabled: !!client,
  });
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(new Set());
  const [format, setFormat] = useState("base64");
  const [expireAt, setExpireAt] = useState("");
  const [isActive, setIsActive] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [rotateOpen, setRotateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const current = (await refetch()).data ?? null;
      setExpireAt(current?.expire_at?.slice(0, 16) ?? "");
      setIsActive(current?.is_active ?? true);
      queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    } catch {
      setError(t("clients.subscriptionError"));
    } finally {
      setLoading(false);
    }
  }, [queryClient, refetch, t]);

  const enableSubscription = () => {
    if (!client || selectedGroups.size === 0) {
      setError(t("clients.subscriptionGroupRequired"));
      return;
    }
    setLoading(true);
    setError("");
    createSubscription({ client_id: client.id, group_ids: Array.from(selectedGroups) })
      .then(() => load())
      .catch(() => setError(t("clients.subscriptionError")))
      .finally(() => setLoading(false));
  };

  const save = () => {
    if (!subscription) return;
    setLoading(true);
    setError("");
    updateSubscription(subscription.id, {
      is_active: isActive,
      expire_at: expireAt ? new Date(expireAt).toISOString() : "",
    })
      .then(() => load())
      .catch(() => setError(t("clients.subscriptionError")))
      .finally(() => setLoading(false));
  };

  const rotate = () => {
    if (!subscription) return;
    setLoading(true);
    deleteSubscription(subscription.id)
      .then(() => load())
      .then(() => setRotateOpen(false))
      .catch(() => setError(t("clients.subscriptionError")))
      .finally(() => setLoading(false));
  };

  const link = subscription
    ? `${baseUrl()}${subscription.url_path}?format=${encodeURIComponent(format)}`
    : "";

  return (
    <>
      <Dialog open={!!client} onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {client
                ? `${t("clients.subscriptionTitle")}: ${client.name}`
                : t("clients.subscriptionTitle")}
            </DialogTitle>
          </DialogHeader>
          {(loading || initialLoading) && !subscription ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : error || queryError ? (
            <div className="space-y-3">
              <p className="text-sm text-destructive">{error || t("clients.subscriptionError")}</p>
              <Button variant="outline" onClick={() => client && load()}>
                {t("common.retry")}
              </Button>
            </div>
          ) : !subscription ? (
            <div className="space-y-4">
              <p className="text-sm text-muted-foreground">{t("clients.enableSubscriptionHint")}</p>
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("clients.subscriptionGroups")}</p>
                {groups.map((group) => (
                  <FormCheckbox
                    key={group.id}
                    isSelected={selectedGroups.has(group.id)}
                    onChange={(selected) => {
                      const next = new Set(selectedGroups);
                      if (selected) next.add(group.id);
                      else next.delete(group.id);
                      setSelectedGroups(next);
                    }}
                  >
                    {group.name}
                  </FormCheckbox>
                ))}
              </div>
              <Button disabled={loading || selectedGroups.size === 0} onClick={enableSubscription}>
                {loading ? <Spinner /> : t("clients.enableSubscription")}
              </Button>
            </div>
          ) : subscription ? (
            <div className="space-y-4">
              <FormSelect
                label={t("subscriptions.format")}
                value={format}
                onChange={setFormat}
                options={FORMATS.map((item) => ({ id: item, label: item }))}
              />
              <CopyableSecret secret={link} label={t("clients.subscriptionLink")} />
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("subscriptions.qrCode")}</p>
                <img
                  src={`${baseUrl()}${subscription.url_path}/qr?format=${encodeURIComponent(format)}`}
                  alt={t("subscriptions.qrCode")}
                  className="size-48 max-w-full rounded-lg bg-white p-2"
                />
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <FormInput
                  type="datetime-local"
                  label={t("subscriptions.expiresAt")}
                  value={expireAt}
                  onChange={setExpireAt}
                />
                <div className="space-y-2 text-sm">
                  <p>{isActive ? t("common.enabled") : t("common.disabled")}</p>
                  <p className="text-muted-foreground">
                    {t("subscriptions.lastAccessed")}:{" "}
                    {formatDateTime(subscription.last_accessed_at)}
                  </p>
                </div>
              </div>
              <FormCheckbox isSelected={isActive} onChange={setIsActive}>
                {t("subscriptions.isActive")}
              </FormCheckbox>
              <div className="flex flex-wrap gap-2">
                <Button variant="outline" onClick={() => setRotateOpen(true)}>
                  {t("clients.rotateSubscription")}
                </Button>
              </div>
            </div>
          ) : null}
          <DialogFooter>
            <Button variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
            {subscription && (
              <Button disabled={loading} onClick={save}>
                {loading ? <Spinner /> : t("common.save")}
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        title={t("clients.rotateSubscription")}
        isOpen={rotateOpen}
        onClose={() => setRotateOpen(false)}
        onConfirm={rotate}
      >
        {t("clients.rotateSubscriptionConfirm")}
      </ConfirmDialog>
    </>
  );
}
