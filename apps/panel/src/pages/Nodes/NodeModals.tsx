import { useTranslation } from "react-i18next";
import {
  CopyableSecret,
  ConfirmDialog,
  DataTable,
  FormInput,
  FormSelect,
  FormTextArea,
} from "../../components/ui";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Spinner } from "../../components/ui/spinner";
import { TableCell, TableHead, TableRow } from "../../components/ui/table";
import type { Node, AgentLog } from "../../api/types";
import type { CoreBinary, InstallCommand } from "../../api/nodes";
import type { NodeFormState } from "./useNodeActions";

interface EditModalProps {
  isOpen: boolean;
  node: Node | null;
  form: NodeFormState;
  onClose: () => void;
  onChange: (form: NodeFormState) => void;
  onConfirm: () => void;
}

export function EditModal({ isOpen, node, form, onClose, onChange, onConfirm }: EditModalProps) {
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
          <DialogTitle>{t("nodes.editTitle")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormInput
            label={t("nodes.name")}
            value={form.name}
            onChange={(value) => onChange({ ...form, name: value })}
          />
          <FormInput
            label={t("nodes.domain")}
            value={form.domain}
            onChange={(value) => onChange({ ...form, domain: value })}
            placeholder={t("common.optional")}
          />
          <FormInput label={t("nodes.hostname")} value={node?.hostname || ""} isReadOnly />
          <FormInput label={t("nodes.address")} value={node?.address || ""} isReadOnly />
          <FormInput
            type="number"
            label={t("nodes.usageCoefficient")}
            value={form.usage_coefficient.toString()}
            onChange={(value) => onChange({ ...form, usage_coefficient: Number(value) })}
          />
          <FormInput
            label={t("nodes.parentId")}
            value={form.parent_id}
            onChange={(value) => onChange({ ...form, parent_id: value })}
            placeholder="UUID (clear to remove)"
          />
          <FormTextArea
            label={t("nodes.labels")}
            value={form.labels}
            onChange={(value) => onChange({ ...form, labels: value })}
            className="font-mono"
          />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={onConfirm}>{t("common.update")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface PushModalProps {
  isOpen: boolean;
  node: Node | null;
  pushCore: string;
  pushing: boolean;
  onClose: () => void;
  onChangeCore: (core: string) => void;
  onConfirm: () => void;
}

export function PushModal({
  isOpen,
  node,
  pushCore,
  pushing,
  onClose,
  onChangeCore,
  onConfirm,
}: PushModalProps) {
  const { t } = useTranslation();

  const coreOptions =
    (node?.cores_available || []).filter(Boolean).length > 0
      ? (node?.cores_available || []).filter(Boolean)
      : ["sing-box", "mihomo"];

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {node ? `${t("nodes.pushTitle")}: ${node.name}` : t("nodes.pushTitle")}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormSelect
            label={t("nodes.selectCore")}
            value={pushCore}
            onChange={onChangeCore}
            options={coreOptions.map((core) => ({ id: core, label: core }))}
            isRequired
          />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={onConfirm} disabled={pushing || !pushCore}>
            {pushing ? <Spinner /> : t("nodes.pushConfig")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface BinariesModalProps {
  isOpen: boolean;
  node: Node | null;
  binaries: CoreBinary[];
  binLoading: boolean;
  binError: string;
  deleteBinary: string | null;
  onClose: () => void;
  onDelete: (fileName: string) => void;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
}

function formatSize(bytes: number) {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

export function BinariesModal({
  isOpen,
  node,
  binaries,
  binLoading,
  binError,
  deleteBinary,
  onClose,
  onDelete,
  onConfirmDelete,
  onCancelDelete,
}: BinariesModalProps) {
  const { t } = useTranslation();

  return (
    <>
      <Dialog
        open={isOpen}
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
      >
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {node ? `${t("nodes.binariesTitle")}: ${node.name}` : t("nodes.binariesTitle")}
            </DialogTitle>
          </DialogHeader>
          <div className="max-h-[60vh] space-y-3 overflow-auto">
            {binError && (
              <p className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
                {binError}
              </p>
            )}
            <DataTable
              aria-label="core binaries"
              columns={5}
              loading={binLoading}
              empty={binaries.length === 0 ? t("nodes.noCoreBinaries") : undefined}
              header={
                <TableRow>
                  <TableHead>{t("nodes.binaryName")}</TableHead>
                  <TableHead>{t("nodes.binarySize")}</TableHead>
                  <TableHead>{t("nodes.binaryModified")}</TableHead>
                  <TableHead>{t("common.status")}</TableHead>
                  <TableHead>{t("common.actions")}</TableHead>
                </TableRow>
              }
            >
              {binaries.map((b) => (
                <TableRow key={b.file_name}>
                  <TableCell className="font-mono text-sm">{b.file_name}</TableCell>
                  <TableCell>{formatSize(b.size_bytes)}</TableCell>
                  <TableCell>
                    {b.modified_at ? new Date(b.modified_at * 1000).toLocaleString() : "-"}
                  </TableCell>
                  <TableCell>
                    {b.in_use ? (
                      <span className="rounded bg-success-soft px-2 py-0.5 text-xs font-medium text-success-soft-foreground">
                        {t("nodes.binaryInUse")}
                      </span>
                    ) : (
                      "-"
                    )}
                  </TableCell>
                  <TableCell>
                    <Button
                      size="sm"
                      variant="destructive"
                      disabled={b.in_use}
                      onClick={() => onDelete(b.file_name)}
                    >
                      {t("common.delete")}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </DataTable>
          </div>
          <DialogFooter>
            <Button onClick={onClose}>{t("common.close")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        title={t("nodes.deleteBinaryTitle")}
        isOpen={!!deleteBinary}
        onClose={onCancelDelete}
        onConfirm={onConfirmDelete}
      >
        {t("nodes.deleteBinaryConfirm", { file: deleteBinary })}
      </ConfirmDialog>
    </>
  );
}

interface LogsModalProps {
  isOpen: boolean;
  node: Node | null;
  logs: AgentLog[];
  logsLoading: boolean;
  onClose: () => void;
}

export function LogsModal({ isOpen, node, logs, logsLoading, onClose }: LogsModalProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{node ? `${t("nodes.logs")}: ${node.name}` : t("nodes.logs")}</DialogTitle>
        </DialogHeader>
        <div className="max-h-[60vh] space-y-2 overflow-auto">
          {logsLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : logs.length === 0 ? (
            <div className="p-4 text-center text-muted-foreground">{t("common.empty")}</div>
          ) : (
            logs.map((log) => (
              <div key={log.id} className="border-b border-border pb-2 text-sm">
                <div className="flex items-center gap-2">
                  <span
                    className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                      log.level === "error"
                        ? "bg-danger-soft text-danger-soft-foreground"
                        : log.level === "warn"
                          ? "bg-warning-soft text-warning-soft-foreground"
                          : "bg-muted text-muted-foreground"
                    }`}
                  >
                    {log.level}
                  </span>
                  <span className="text-muted-foreground">{log.target}</span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {new Date(log.created_at).toLocaleString()}
                  </span>
                </div>
                <p className="mt-1 whitespace-pre-wrap break-words">{log.message}</p>
              </div>
            ))
          )}
        </div>
        <DialogFooter>
          <Button onClick={onClose}>{t("common.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface InstallModalProps {
  isOpen: boolean;
  installCmd: InstallCommand | null;
  installLoading: boolean;
  onClose: () => void;
}

export function InstallModal({ isOpen, installCmd, installLoading, onClose }: InstallModalProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>
            {installCmd
              ? `${t("nodes.installTitle")}: ${installCmd.name}`
              : t("nodes.installTitle")}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
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
                  <span className="font-medium">{t("nodes.version")}:</span> {installCmd.version}
                </p>
              </div>
            </>
          ) : null}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
