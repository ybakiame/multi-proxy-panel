import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import { PageHeader, ConfirmDialog, FormInput, FormSelect } from "../components/ui";
import {
  getCertificates,
  createCertificate,
  renewCertificate,
  deleteCertificate,
} from "../api/certificates";
import { getNodes } from "../api/nodes";

const certificatesQueryKey = "certificates";
const nodesQueryKey = "nodes";

export function Certificates() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [form, setForm] = useState({ domain: "", node_id: "" });

  const { data: certs = [], isLoading: loading } = useQuery({
    queryKey: [certificatesQueryKey],
    queryFn: () => getCertificates(),
  });

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const createMutation = useMutation({
    mutationFn: createCertificate,
    onSuccess: () => {
      setCreateOpen(false);
      setForm({ domain: "", node_id: "" });
      queryClient.invalidateQueries({ queryKey: [certificatesQueryKey] });
    },
  });

  const renewMutation = useMutation({
    mutationFn: renewCertificate,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [certificatesQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteCertificate,
    onSuccess: () => {
      setDeleteId(null);
      queryClient.invalidateQueries({ queryKey: [certificatesQueryKey] });
    },
  });

  const handleCreate = () => {
    createMutation.mutate({ domain: form.domain.trim(), node_id: form.node_id });
  };

  const handleRenew = (id: string) => {
    renewMutation.mutate(id);
  };

  const handleDelete = () => {
    if (!deleteId) return;
    deleteMutation.mutate(deleteId);
  };

  const statusBadge = (status: string) => {
    const color =
      status === "active"
        ? "bg-success-soft text-success-soft-foreground"
        : status === "failed"
          ? "bg-danger-soft text-danger-soft-foreground"
          : "bg-warning-soft text-warning-soft-foreground";
    return (
      <span
        className={`inline-flex items-center whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium ${color}`}
      >
        {t(`certificates.status_${status}`, { defaultValue: status })}
      </span>
    );
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("certificates.title")}
        action={{
          label: t("certificates.create"),
          onClick: () => setCreateOpen(true),
        }}
      />

      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="certificates">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("certificates.domain")}</Table.Column>
                    <Table.Column>{t("certificates.node")}</Table.Column>
                    <Table.Column>{t("common.status")}</Table.Column>
                    <Table.Column>{t("common.expiry")}</Table.Column>
                    <Table.Column>{t("certificates.lastError")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("certificates.emptyHint")}
                      </div>
                    )}
                  >
                    {certs.map((cert) => (
                      <Table.Row key={cert.id}>
                        <Table.Cell className="font-mono text-sm">{cert.domain}</Table.Cell>
                        <Table.Cell>{cert.node_name || cert.node_id}</Table.Cell>
                        <Table.Cell>{statusBadge(cert.status)}</Table.Cell>
                        <Table.Cell>
                          {cert.expires_at ? new Date(cert.expires_at).toLocaleDateString() : "-"}
                        </Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {cert.last_error || "-"}
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => handleRenew(cert.id)}>
                              {t("certificates.renew")}
                            </Button>
                            <Button size="sm" variant="danger" onPress={() => setDeleteId(cert.id)}>
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
        title={t("certificates.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("certificates.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("certificates.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("certificates.domain")}
                value={form.domain}
                onChange={(value) => setForm({ ...form, domain: value })}
                placeholder="hy2.example.com"
                description={t("certificates.domainDescription")}
                isRequired
              />
              <FormSelect
                label={t("certificates.node")}
                value={form.node_id}
                onChange={(value) => setForm({ ...form, node_id: value })}
                options={nodes.map((n) => ({ id: n.id, label: n.name }))}
                isRequired
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setCreateOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button isDisabled={!form.domain.trim() || !form.node_id} onPress={handleCreate}>
                {t("common.create")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
