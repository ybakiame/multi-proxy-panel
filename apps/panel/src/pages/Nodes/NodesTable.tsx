import { useTranslation } from "react-i18next";
import { Card, CardContent } from "../../components/ui/card";
import { TableCell, TableHead, TableRow } from "../../components/ui/table";
import {
  DataTable,
  MobileCard,
  MobileCardList,
  RowActions,
  StatusBadge,
} from "../../components/ui";
import type { Node } from "../../api/types";
import type { PendingUpdate } from "../../api/nodes";

interface NodesTableProps {
  nodes: Node[];
  pending: PendingUpdate[];
  isLoading: boolean;
  onEdit: (node: Node) => void;
  onLogs: (node: Node) => void;
  onPush: (node: Node) => void;
  onInstall: (node: Node) => void;
  onBinaries: (node: Node) => void;
  onCertificate: (node: Node) => void;
  onDelete: (node: Node) => void;
}

export function NodesTable({
  nodes,
  pending,
  isLoading,
  onEdit,
  onLogs,
  onPush,
  onInstall,
  onBinaries,
  onCertificate,
  onDelete,
}: NodesTableProps) {
  const { t } = useTranslation();

  const actions = (node: Node) => (
    <RowActions
      label={node.name}
      primaryAction={{ label: t("common.edit"), onSelect: () => onEdit(node) }}
      actions={[
        { label: t("nodes.logs"), onSelect: () => onLogs(node) },
        { label: t("nodes.pushConfig"), onSelect: () => onPush(node) },
        { label: t("nodes.installCommand"), onSelect: () => onInstall(node) },
        { label: t("nodes.binaries"), onSelect: () => onBinaries(node) },
        { label: t("certificates.create"), onSelect: () => onCertificate(node) },
        { label: t("common.delete"), onSelect: () => onDelete(node), destructive: true },
      ]}
    />
  );

  const coreStatus = (node: Node) => (
    <div className="flex max-w-xs flex-wrap gap-1">
      {(node.core_statuses || []).map((cs) => {
        const shortVersion = cs.version.match(/\d+\.\d+[\d.]*/)?.[0] ?? "";
        return (
          <span
            key={cs.core_type}
            className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs font-medium whitespace-nowrap ${
              cs.running ? "bg-success/10 text-success" : "bg-destructive/10 text-destructive"
            }`}
            title={cs.version}
          >
            {cs.core_type}
            {shortVersion && <span className="opacity-70">{shortVersion}</span>}
            <span>{cs.running ? t("common.running") : t("common.stopped")}</span>
          </span>
        );
      })}
      {(node.core_statuses || []).length === 0 && "-"}
    </div>
  );

  return (
    <Card>
      <CardContent className="space-y-4 md:space-y-0">
        <div className="hidden md:block">
          <DataTable
            aria-label="nodes"
            columns={10}
            loading={isLoading}
            empty={nodes.length === 0 ? t("common.empty") : undefined}
            header={
              <TableRow>
                <TableHead>{t("nodes.name")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("nodes.hostname")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("nodes.address")}</TableHead>
                <TableHead>{t("common.status")}</TableHead>
                <TableHead>{t("nodes.pending")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("nodes.coreStatus")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("nodes.parentId")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("nodes.coresAvailable")}</TableHead>
                <TableHead className="hidden lg:table-cell">
                  {t("nodes.usageCoefficient")}
                </TableHead>
                <TableHead className="text-right">{t("common.actions")}</TableHead>
              </TableRow>
            }
          >
            {nodes.map((node) => (
              <TableRow key={node.id}>
                <TableCell className="max-w-48 font-medium break-all">{node.name}</TableCell>
                <TableCell className="hidden lg:table-cell">{node.hostname || "-"}</TableCell>
                <TableCell className="hidden lg:table-cell">{node.address || "-"}</TableCell>
                <TableCell>
                  <StatusBadge status={node.status} />
                </TableCell>
                <TableCell>
                  <div className="flex max-w-40 flex-wrap gap-1">
                    {pending
                      .filter((p) => p.node_id === node.id)
                      .map((p, i) => (
                        <span
                          key={i}
                          className="inline-flex items-center rounded bg-warning/10 px-2 py-0.5 text-xs font-medium whitespace-nowrap text-warning"
                        >
                          {p.update_type === "core"
                            ? `${t("nodes.pendingCore")} ${p.core_type}`
                            : `${t("nodes.pendingConfig")} ${p.core_type}`}
                        </span>
                      ))}
                    {pending.filter((p) => p.node_id === node.id).length === 0 && "-"}
                  </div>
                </TableCell>
                <TableCell className="hidden xl:table-cell">{coreStatus(node)}</TableCell>
                <TableCell className="hidden xl:table-cell">
                  {node.parent_id ? `${t("nodes.childOf")} ${node.parent_id}` : "-"}
                </TableCell>
                <TableCell className="hidden xl:table-cell">
                  {(node.cores_available || []).join(", ") || "-"}
                </TableCell>
                <TableCell className="hidden lg:table-cell">{node.usage_coefficient}</TableCell>
                <TableCell>{actions(node)}</TableCell>
              </TableRow>
            ))}
          </DataTable>
        </div>
        {!isLoading && nodes.length > 0 && (
          <MobileCardList>
            {nodes.map((node) => (
              <MobileCard
                key={node.id}
                title={node.name}
                status={<StatusBadge status={node.status} />}
                fields={[
                  { label: t("nodes.address"), value: node.address || node.hostname || "-" },
                  { label: t("nodes.usageCoefficient"), value: node.usage_coefficient },
                  { label: t("nodes.coreStatus"), value: coreStatus(node) },
                ]}
                actions={actions(node)}
              />
            ))}
          </MobileCardList>
        )}
      </CardContent>
    </Card>
  );
}
