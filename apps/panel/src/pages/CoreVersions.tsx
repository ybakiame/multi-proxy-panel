import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
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
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="core versions">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("coreVersions.core")}</Table.Column>
                    <Table.Column>{t("coreVersions.version")}</Table.Column>
                    <Table.Column>{t("coreVersions.channel")}</Table.Column>
                    <Table.Column>{t("coreVersions.inUse")}</Table.Column>
                    <Table.Column>{t("coreVersions.publishedAt")}</Table.Column>
                    <Table.Column>{t("common.createdAt")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("coreVersions.emptyHint")}
                      </div>
                    )}
                  >
                    {versions.map((v) => (
                      <Table.Row key={v.id}>
                        <Table.Cell>{v.core_type}</Table.Cell>
                        <Table.Cell className="font-mono text-sm">{v.version}</Table.Cell>
                        <Table.Cell>{channelBadge(v.channel)}</Table.Cell>
                        <Table.Cell>
                          {v.is_active ? (
                            <span className="inline-flex items-center whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium bg-success-soft text-success-soft-foreground">
                              {t("coreVersions.inUse")}
                            </span>
                          ) : null}
                        </Table.Cell>
                        <Table.Cell>
                          {v.published_at ? new Date(v.published_at).toLocaleString() : "-"}
                        </Table.Cell>
                        <Table.Cell>{new Date(v.created_at).toLocaleString()}</Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            {!v.is_active && (
                              <Button
                                size="sm"
                                variant="ghost"
                                onPress={() => handleActivate(v.id)}
                              >
                                {t("coreVersions.setActive")}
                              </Button>
                            )}
                            <Button
                              size="sm"
                              variant="danger"
                              isDisabled={v.is_active}
                              onPress={() => setDeleteId(v.id)}
                            >
                              {t("common.delete")}
                            </Button>
                          </div>
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
      </Card>

      <ConfirmDialog
        title={t("coreVersions.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("coreVersions.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={upstreamOpen} onOpenChange={(open) => setUpstreamOpen(open)}>
        <Modal.Container className="max-w-2xl">
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("coreVersions.selectVersions")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="max-h-[60vh] overflow-auto space-y-4">
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
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setUpstreamOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button isDisabled={selected.size === 0 || saving} onPress={handleSave}>
                {saving ? t("common.submitting") : t("coreVersions.saveSelected")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
