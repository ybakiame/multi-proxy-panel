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
import { PageHeader, ConfirmDialog, Pagination, FormInput, FormCheckbox } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getHosts, createHost, updateHost, deleteHost } from "../api/hosts";
import type { CreateHostPayload } from "../api/hosts";
import { InboundHost } from "../api/types";

interface HostForm {
  protocol_config_id: string;
  node_id: string;
  remark: string;
  address: string;
  port: string;
  sni: string;
  host: string;
  path: string;
  is_active: boolean;
}

const hostsQueryKey = "hosts";

export function Hosts() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [editHost, setEditHost] = useState<InboundHost | null>(null);
  const [deleteHostId, setDeleteHostId] = useState<string | null>(null);
  const [form, setForm] = useState<HostForm>({
    protocol_config_id: "",
    node_id: "",
    remark: "",
    address: "",
    port: "",
    sni: "",
    host: "",
    path: "",
    is_active: true,
  });

  const { data: hostsData, isLoading: loading } = useQuery({
    queryKey: [hostsQueryKey, { page, perPage }],
    queryFn: () => getHosts(page, perPage),
  });

  const hosts = hostsData?.data ?? [];
  const total = hostsData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const createMutation = useMutation({
    mutationFn: createHost,
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [hostsQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; data: Partial<CreateHostPayload> }) =>
      updateHost(payload.id, payload.data),
    onSuccess: () => {
      setEditHost(null);
      queryClient.invalidateQueries({ queryKey: [hostsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteHost,
    onSuccess: () => {
      setDeleteHostId(null);
      queryClient.invalidateQueries({ queryKey: [hostsQueryKey] });
    },
  });

  const resetForm = (host?: InboundHost) => {
    if (host) {
      setForm({
        protocol_config_id: host.protocol_config_id,
        node_id: host.node_id,
        remark: host.remark,
        address: host.address,
        port: host.port.toString(),
        sni: host.sni || "",
        host: host.host || "",
        path: host.path || "",
        is_active: host.is_active,
      });
    } else {
      setForm({
        protocol_config_id: "",
        node_id: "",
        remark: "",
        address: "",
        port: "",
        sni: "",
        host: "",
        path: "",
        is_active: true,
      });
    }
  };

  const buildPayload = (): Partial<CreateHostPayload> => {
    return {
      protocol_config_id: form.protocol_config_id || undefined,
      node_id: form.node_id || undefined,
      remark: form.remark || undefined,
      address: form.address || undefined,
      port: form.port ? Number(form.port) : undefined,
      sni: form.sni || undefined,
      host: form.host || undefined,
      path: form.path || undefined,
      is_active: form.is_active,
    };
  };

  const handleCreate = () => {
    createMutation.mutate({
      protocol_config_id: form.protocol_config_id,
      node_id: form.node_id,
      remark: form.remark,
      address: form.address,
      port: Number(form.port),
      sni: form.sni || undefined,
      host: form.host || undefined,
      path: form.path || undefined,
      is_active: form.is_active,
    });
  };

  const handleUpdate = () => {
    if (!editHost) return;
    updateMutation.mutate({ id: editHost.id, data: buildPayload() });
  };

  const handleDelete = () => {
    if (!deleteHostId) return;
    deleteMutation.mutate(deleteHostId);
  };

  const openEdit = (host: InboundHost) => {
    resetForm(host);
    setEditHost(host);
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("hosts.title")}
        action={{
          label: t("hosts.create"),
          onClick: () => {
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      <Card>
        <CardContent>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="hosts">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("hosts.remark")}</TableHead>
                  <TableHead>{t("hosts.address")}</TableHead>
                  <TableHead>{t("hosts.port")}</TableHead>
                  <TableHead>{t("hosts.sni")}</TableHead>
                  <TableHead>{t("hosts.host")}</TableHead>
                  <TableHead>{t("hosts.path")}</TableHead>
                  <TableHead>{t("hosts.isActive")}</TableHead>
                  <TableHead>{t("hosts.protocolConfig")}</TableHead>
                  <TableHead>{t("hosts.node")}</TableHead>
                  <TableHead>{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {hosts.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={10} className="p-4 text-center text-muted-foreground">
                      {t("common.empty")}
                    </TableCell>
                  </TableRow>
                )}
                {hosts.map((host) => (
                  <TableRow key={host.id}>
                    <TableCell>{host.remark}</TableCell>
                    <TableCell>{host.address}</TableCell>
                    <TableCell>{host.port}</TableCell>
                    <TableCell>{host.sni || "-"}</TableCell>
                    <TableCell>{host.host || "-"}</TableCell>
                    <TableCell>{host.path || "-"}</TableCell>
                    <TableCell>
                      {host.is_active ? t("common.enabled") : t("common.disabled")}
                    </TableCell>
                    <TableCell className="max-w-xs truncate">{host.protocol_config_id}</TableCell>
                    <TableCell className="max-w-xs truncate">{host.node_id}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="ghost" onClick={() => openEdit(host)}>
                          {t("common.edit")}
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => setDeleteHostId(host.id)}
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

      <Pagination
        page={page}
        totalPages={totalPages}
        perPage={perPage}
        total={total}
        onPageChange={setPage}
        onPerPageChange={setPerPage}
      />

      <ConfirmDialog
        title={t("hosts.deleteTitle")}
        isOpen={!!deleteHostId}
        onClose={() => setDeleteHostId(null)}
        onConfirm={handleDelete}
      >
        {t("hosts.deleteConfirm")}
      </ConfirmDialog>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("hosts.createTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <FormInput
              label={t("hosts.protocolConfig")}
              value={form.protocol_config_id}
              onChange={(value) => setForm({ ...form, protocol_config_id: value })}
              placeholder="UUID"
              isRequired
            />
            <FormInput
              label={t("hosts.node")}
              value={form.node_id}
              onChange={(value) => setForm({ ...form, node_id: value })}
              placeholder="UUID"
              isRequired
            />
            <FormInput
              label={t("hosts.remark")}
              value={form.remark}
              onChange={(value) => setForm({ ...form, remark: value })}
              isRequired
            />
            <FormInput
              label={t("hosts.address")}
              value={form.address}
              onChange={(value) => setForm({ ...form, address: value })}
              isRequired
            />
            <FormInput
              type="number"
              label={t("hosts.port")}
              value={form.port}
              onChange={(value) => setForm({ ...form, port: value })}
              isRequired
            />
            <FormInput
              label={t("hosts.sni")}
              value={form.sni}
              onChange={(value) => setForm({ ...form, sni: value })}
            />
            <FormInput
              label={t("hosts.host")}
              value={form.host}
              onChange={(value) => setForm({ ...form, host: value })}
            />
            <FormInput
              label={t("hosts.path")}
              value={form.path}
              onChange={(value) => setForm({ ...form, path: value })}
            />
            <FormCheckbox
              isSelected={form.is_active}
              onChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("hosts.isActive")}
            </FormCheckbox>
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreate}>{t("common.create")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={!!editHost}
        onOpenChange={(open) => {
          if (!open) setEditHost(null);
        }}
      >
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("hosts.editTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <FormInput
              label={t("hosts.protocolConfig")}
              value={form.protocol_config_id}
              onChange={(value) => setForm({ ...form, protocol_config_id: value })}
              placeholder="UUID"
            />
            <FormInput
              label={t("hosts.node")}
              value={form.node_id}
              onChange={(value) => setForm({ ...form, node_id: value })}
              placeholder="UUID"
            />
            <FormInput
              label={t("hosts.remark")}
              value={form.remark}
              onChange={(value) => setForm({ ...form, remark: value })}
            />
            <FormInput
              label={t("hosts.address")}
              value={form.address}
              onChange={(value) => setForm({ ...form, address: value })}
            />
            <FormInput
              type="number"
              label={t("hosts.port")}
              value={form.port}
              onChange={(value) => setForm({ ...form, port: value })}
            />
            <FormInput
              label={t("hosts.sni")}
              value={form.sni}
              onChange={(value) => setForm({ ...form, sni: value })}
            />
            <FormInput
              label={t("hosts.host")}
              value={form.host}
              onChange={(value) => setForm({ ...form, host: value })}
            />
            <FormInput
              label={t("hosts.path")}
              value={form.path}
              onChange={(value) => setForm({ ...form, path: value })}
            />
            <FormCheckbox
              isSelected={form.is_active}
              onChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("hosts.isActive")}
            </FormCheckbox>
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setEditHost(null)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleUpdate}>{t("common.update")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
