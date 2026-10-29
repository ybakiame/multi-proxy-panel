import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createCertificate } from "../api/certificates";
import type { Node } from "../api/types";
import { FormInput, FormSelect } from "../components/ui";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";

interface CertificateFormDialogProps {
  isOpen: boolean;
  nodes: Node[];
  initialNodeId?: string;
  onOpenChange: (open: boolean) => void;
}

export function CertificateFormDialog({
  isOpen,
  nodes,
  initialNodeId,
  onOpenChange,
}: CertificateFormDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [domain, setDomain] = useState("");
  const [nodeId, setNodeId] = useState(initialNodeId ?? "");
  const [error, setError] = useState("");

  const createMutation = useMutation({
    mutationFn: createCertificate,
    onSuccess: () => {
      onOpenChange(false);
      setDomain("");
      setError("");
      queryClient.invalidateQueries({ queryKey: ["certificates"] });
    },
    onError: () => setError(t("certificates.createError")),
  });

  const handleCreate = () => {
    createMutation.mutate({ domain: domain.trim(), node_id: nodeId });
  };

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("certificates.createTitle")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormInput
            label={t("certificates.domain")}
            value={domain}
            onChange={setDomain}
            placeholder="hy2.example.com"
            description={t("certificates.domainDescription")}
            isRequired
          />
          <FormSelect
            label={t("certificates.node")}
            value={nodeId}
            onChange={setNodeId}
            options={nodes.map((node) => ({ id: node.id, label: node.name }))}
            isDisabled={!!initialNodeId}
            isRequired
          />
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button
            disabled={!domain.trim() || !nodeId || createMutation.isPending}
            onClick={handleCreate}
          >
            {createMutation.isPending ? t("common.submitting") : t("common.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
