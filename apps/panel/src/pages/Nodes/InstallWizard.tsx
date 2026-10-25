import { Link } from "react-router-dom";
import { CopyableSecret, FormInput, FormTextArea } from "../../components/ui";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Spinner } from "../../components/ui/spinner";
import type { Node } from "../../api/types";
import type { InstallCommand } from "../../api/nodes";
import type { NodeFormState } from "./useNodeActions";
import { useTranslation } from "react-i18next";

interface InstallWizardProps {
  isOpen: boolean;
  wizardStep: 1 | 2;
  newNodeId: string | null;
  form: NodeFormState;
  showAdvanced: boolean;
  installCmd: InstallCommand | null;
  installLoading: boolean;
  createPending: boolean;
  pollingNode: Node | null;
  onClose: () => void;
  onChangeForm: (form: NodeFormState) => void;
  onToggleAdvanced: () => void;
  onCreate: () => void;
}

export function InstallWizard({
  isOpen,
  wizardStep,
  form,
  showAdvanced,
  installCmd,
  installLoading,
  createPending,
  pollingNode,
  onClose,
  onChangeForm,
  onToggleAdvanced,
  onCreate,
}: InstallWizardProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("nodes.wizardTitle")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          {wizardStep === 1 ? (
            <>
              <FormInput
                label={t("nodes.name")}
                value={form.name}
                onChange={(value) => onChangeForm({ ...form, name: value })}
                isRequired
              />
              <FormInput
                label={t("nodes.domain")}
                value={form.domain}
                onChange={(value) => onChangeForm({ ...form, domain: value })}
                placeholder={t("common.optional")}
              />
              <p className="text-sm text-muted-foreground">{t("nodes.step1Hint")}</p>
              <Button variant="ghost" size="sm" onClick={onToggleAdvanced}>
                {showAdvanced ? t("common.collapse") : t("common.expand")}
              </Button>
              {showAdvanced && (
                <div className="space-y-4">
                  <FormInput
                    type="number"
                    label={t("nodes.usageCoefficient")}
                    value={form.usage_coefficient.toString()}
                    onChange={(value) =>
                      onChangeForm({ ...form, usage_coefficient: Number(value) })
                    }
                  />
                  <FormInput
                    label={t("nodes.parentId")}
                    value={form.parent_id}
                    onChange={(value) => onChangeForm({ ...form, parent_id: value })}
                    placeholder="UUID (optional)"
                  />
                  <FormTextArea
                    label={t("nodes.labels")}
                    value={form.labels}
                    onChange={(value) => onChangeForm({ ...form, labels: value })}
                    className="font-mono"
                  />
                </div>
              )}
            </>
          ) : (
            <>
              <h3 className="text-sm font-medium">{t("nodes.step2Title")}</h3>
              {installLoading ? (
                <div className="flex h-32 items-center justify-center">
                  <Spinner />
                </div>
              ) : installCmd ? (
                <>
                  {installCmd.was_connected && (
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-sm text-warning">
                      {t("nodes.installWarning")}
                    </div>
                  )}
                  <p className="text-sm text-muted-foreground">{t("nodes.installHint")}</p>
                  <CopyableSecret secret={installCmd.command} label={t("nodes.installCommand")} />
                  <div className="space-y-1 text-sm text-muted-foreground">
                    <p>
                      <span className="font-medium">{t("nodes.scriptUrl")}:</span>{" "}
                      {installCmd.script_url}
                    </p>
                    <p>
                      <span className="font-medium">{t("nodes.hubUrl")}:</span> {installCmd.hub_url}
                    </p>
                    <p>
                      <span className="font-medium">{t("nodes.version")}:</span>{" "}
                      {installCmd.version}
                    </p>
                  </div>
                  <div className="flex items-center gap-3 rounded-lg bg-muted p-3">
                    {pollingNode?.status === "online" ? (
                      <>
                        <span className="inline-flex h-6 w-6 items-center justify-center rounded-full bg-success text-success-foreground">
                          ✓
                        </span>
                        <span className="text-sm font-medium">{t("nodes.connected")}</span>
                        <Link to="/bindings">
                          <Button size="sm">{t("nodes.gotoBindings")}</Button>
                        </Link>
                      </>
                    ) : (
                      <>
                        <Spinner />
                        <span className="text-sm text-muted-foreground">
                          {t("nodes.waitingConnect")}
                        </span>
                      </>
                    )}
                  </div>
                </>
              ) : null}
            </>
          )}
        </div>
        <DialogFooter>
          {wizardStep === 1 ? (
            <>
              <Button variant="ghost" onClick={onClose}>
                {t("common.cancel")}
              </Button>
              <Button onClick={onCreate} disabled={!form.name.trim() || createPending}>
                {createPending ? <Spinner /> : t("common.create")}
              </Button>
            </>
          ) : (
            <Button variant="ghost" onClick={onClose}>
              {t("common.close")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
