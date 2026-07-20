import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import { PageHeader, ConfirmDialog, FormCheckbox } from "../components/ui";
import {
  getCoreVersions,
  getUpstreamCoreVersions,
  saveCoreVersions,
  deleteCoreVersion,
  UpstreamCore,
  SaveVersionItem,
} from "../api/coreVersions";
import { CoreVersion } from "../api/types";

export function CoreVersions() {
  const { t } = useTranslation();
  const [versions, setVersions] = useState<CoreVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [upstreamOpen, setUpstreamOpen] = useState(false);
  const [upstreamLoading, setUpstreamLoading] = useState(false);
  const [upstream, setUpstream] = useState<UpstreamCore[]>([]);
  const [selected, setSelected] = useState<Map<string, SaveVersionItem>>(new Map());
  const [saving, setSaving] = useState(false);

  const fetch = async () => {
    setLoading(true);
    try {
      setVersions(await getCoreVersions());
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, []);

  const openUpstream = async () => {
    setUpstreamOpen(true);
    setSelected(new Map());
    setUpstreamLoading(true);
    try {
      setUpstream(await getUpstreamCoreVersions());
    } catch {
      // error handled by axios interceptor
    } finally {
      setUpstreamLoading(false);
    }
  };

  const toggleVersion = (item: SaveVersionItem, checked: boolean) => {
    const key = `${item.core_type}|${item.version}`;
    const next = new Map(selected);
    if (checked) next.set(key, item);
    else next.delete(key);
    setSelected(next);
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveCoreVersions(Array.from(selected.values()));
      setUpstreamOpen(false);
      fetch();
    } catch {
      // error handled by axios interceptor
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!deleteId) return;
    try {
      await deleteCoreVersion(deleteId);
      setDeleteId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
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
          {loading ? (
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
                        <Table.Cell>{new Date(v.created_at).toLocaleString()}</Table.Cell>
                        <Table.Cell>
                          <Button size="sm" variant="danger" onPress={() => setDeleteId(v.id)}>
                            {t("common.delete")}
                          </Button>
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
                            isSelected={v.saved || selected.has(key)}
                            isDisabled={v.saved}
                            onChange={(checked) =>
                              toggleVersion(
                                {
                                  core_type: core.core_type,
                                  version: v.version,
                                  channel: v.channel,
                                },
                                checked,
                              )
                            }
                          >
                            <span className="inline-flex items-center gap-1">
                              <span className="font-mono text-sm">{v.version}</span>
                              {channelBadge(v.channel)}
                              {v.saved && (
                                <span className="text-xs text-muted-foreground">
                                  {t("coreVersions.savedBadge")}
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
