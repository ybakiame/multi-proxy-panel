import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { PageHeader, ConfirmDialog, FormCheckbox } from "../components/ui";
import {
  getCoreVersions,
  getUpstreamCoreVersions,
  saveCoreVersions,
  deleteCoreVersion,
  activateCoreVersion,
  SaveVersionItem,
} from "../api/coreVersions";

const coreVersionsQueryKey = "core-versions";
const upstreamQueryKey = "core-versions-upstream";

export function CoreVersions() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [upstreamOpen, setUpstreamOpen] = useState(false);
  const [selected, setSelected] = useState<Map<string, SaveVersionItem>>(new Map());
  const [saving, setSaving] = useState(false);

  const { data: versions = [], isLoading } = useQuery({
    queryKey: [coreVersionsQueryKey],
    queryFn: () => getCoreVersions(),
  });

  const { data: upstream = [], isLoading: upstreamLoading } = useQuery({
    queryKey: [upstreamQueryKey],
    queryFn: getUpstreamCoreVersions,
    enabled: upstreamOpen,
  });

  const saveMutation = useMutation({
    mutationFn: saveCoreVersions,
    onSuccess: () => {
      setUpstreamOpen(false);
      setSelected(new Map());
      queryClient.invalidateQueries({ queryKey: [coreVersionsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteCoreVersion,
    onSuccess: () => {
      setDeleteId(null);
      queryClient.invalidateQueries({ queryKey: [coreVersionsQueryKey] });
    },
  });

  const activateMutation = useMutation({
    mutationFn: activateCoreVersion,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [coreVersionsQueryKey] });
    },
  });

  const openUpstream = () => {
    setUpstreamOpen(true);
    setSelected(new Map());
  };

  const toggleVersion = (item: SaveVersionItem, checked: boolean) => {
    const key = `${item.core_type}|${item.version}`;
    const next = new Map(selected);
    if (checked) next.set(key, item);
    else next.delete(key);
    setSelected(next);
  };

  const handleSave = () => {
    setSaving(true);
    saveMutation.mutate(Array.from(selected.values()), {
      onSettled: () => setSaving(false),
    });
  };

  const handleDelete = () => {
    if (!deleteId) return;
    deleteMutation.mutate(deleteId);
  };

  const handleActivate = (id: string) => {
    activateMutation.mutate(id);
  };

  const channelBadge = (channel: string) => (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium ${
        channel === "prerelease"
          ? "bg-warning-soft text-warning-soft-foreground"
          : "bg-success-soft text-success-soft-foreground"
      }`}
    >
      {channel === "prerelease" ? t("coreVersions.prerelease") : t("coreVersions.release")}
    </span>
  );

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("coreVersions.title")}
        action={{
          label: t("coreVersions.addFromUpstream"),
          onClick: openUpstream,
        }}
      />

      <Card>
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="core versions">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("coreVersions.core")}</TableHead>
                  <TableHead>{t("coreVersions.version")}</TableHead>
                  <TableHead>{t("coreVersions.channel")}</TableHead>
                  <TableHead>{t("coreVersions.inUse")}</TableHead>
                  <TableHead>{t("coreVersions.publishedAt")}</TableHead>
                  <TableHead>{t("common.createdAt")}</TableHead>
                  <TableHead>{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {versions.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={7} className="p-4 text-center text-muted-foreground">
                      {t("coreVersions.emptyHint")}
                    </TableCell>
                  </TableRow>
                )}
                {versions.map((v) => (
                  <TableRow key={v.id}>
                    <TableCell>{v.core_type}</TableCell>
                    <TableCell className="font-mono text-sm">{v.version}</TableCell>
                    <TableCell>{channelBadge(v.channel)}</TableCell>
                    <TableCell>
                      {v.is_active ? (
                        <span className="inline-flex items-center rounded bg-success-soft px-2 py-0.5 text-xs font-medium whitespace-nowrap text-success-soft-foreground">
                          {t("coreVersions.inUse")}
                        </span>
                      ) : null}
                    </TableCell>
                    <TableCell>
                      {v.published_at ? new Date(v.published_at).toLocaleString() : "-"}
                    </TableCell>
                    <TableCell>{new Date(v.created_at).toLocaleString()}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        {!v.is_active && (
                          <Button size="sm" variant="ghost" onClick={() => handleActivate(v.id)}>
                            {t("coreVersions.setActive")}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={v.is_active}
                          onClick={() => setDeleteId(v.id)}
                        >
                          {t("common.delete")}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("coreVersions.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("coreVersions.deleteConfirm")}
      </ConfirmDialog>

      <Dialog open={upstreamOpen} onOpenChange={setUpstreamOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{t("coreVersions.selectVersions")}</DialogTitle>
          </DialogHeader>
          <div className="max-h-[60vh] space-y-4 overflow-auto">
            {upstreamLoading ? (
              <div className="flex h-32 items-center justify-center">
                <Spinner />
              </div>
            ) : (
              upstream.map((core) => (
                <div key={core.core_type} className="space-y-2">
                  <p className="text-sm font-medium">{core.core_type}</p>
                  <div className="flex flex-wrap gap-2">
                    {core.versions.map((v) => {
                      const key = `${core.core_type}|${v.version}`;
                      return (
                        <FormCheckbox
                          key={key}
                          isSelected={(v.saved && !v.update_available) || selected.has(key)}
                          isDisabled={v.saved && !v.update_available}
                          onChange={(checked) =>
                            toggleVersion(
                              {
                                core_type: core.core_type,
                                version: v.version,
                                channel: v.channel,
                                published_at: v.published_at,
                                commit_sha: v.commit_sha,
                              },
                              checked,
                            )
                          }
                        >
                          <span className="inline-flex items-center gap-1">
                            <span className="font-mono text-sm">{v.version}</span>
                            {channelBadge(v.channel)}
                            {v.saved && !v.update_available && (
                              <span className="text-xs text-muted-foreground">
                                {t("coreVersions.savedBadge")}
                              </span>
                            )}
                            {v.saved && v.update_available && (
                              <span className="rounded bg-warning-soft px-1.5 py-0.5 text-xs font-medium text-warning-soft-foreground">
                                {t("coreVersions.updateAvailable")}
                              </span>
                            )}
                          </span>
                        </FormCheckbox>
                      );
                    })}
                  </div>
                </div>
              ))
            )}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setUpstreamOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button disabled={selected.size === 0 || saving} onClick={handleSave}>
              {saving ? t("common.submitting") : t("coreVersions.saveSelected")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
