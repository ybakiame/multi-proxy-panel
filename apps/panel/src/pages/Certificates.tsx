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
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import { DataTable, PageHeader, ConfirmDialog, FormInput, FormSelect } from "../components/ui";
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
        <CardContent>
          <DataTable
            aria-label="certificates"
            columns={6}
            loading={loading}
            empty={certs.length === 0 ? t("certificates.emptyHint") : undefined}
            header={
              <TableRow>
                <TableHead>{t("certificates.domain")}</TableHead>
                <TableHead>{t("certificates.node")}</TableHead>
                <TableHead>{t("common.status")}</TableHead>
                <TableHead>{t("common.expiry")}</TableHead>
                <TableHead>{t("certificates.lastError")}</TableHead>
                <TableHead>{t("common.actions")}</TableHead>
              </TableRow>
            }
          >
            {certs.map((cert) => (
              <TableRow key={cert.id}>
                <TableCell className="font-mono text-sm">{cert.domain}</TableCell>
                <TableCell>{cert.node_name || cert.node_id}</TableCell>
                <TableCell>{statusBadge(cert.status)}</TableCell>
                <TableCell>
                  {cert.expires_at ? new Date(cert.expires_at).toLocaleDateString() : "-"}
                </TableCell>
                <TableCell className="max-w-xs truncate">{cert.last_error || "-"}</TableCell>
                <TableCell>
                  <div className="flex gap-2">
                    <Button size="sm" variant="ghost" onClick={() => handleRenew(cert.id)}>
                      {t("certificates.renew")}
                    </Button>
                    <Button size="sm" variant="destructive" onClick={() => setDeleteId(cert.id)}>
                      {t("common.delete")}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </DataTable>
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("certificates.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("certificates.deleteConfirm")}
      </ConfirmDialog>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("certificates.createTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button disabled={!form.domain.trim() || !form.node_id} onClick={handleCreate}>
              {t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
