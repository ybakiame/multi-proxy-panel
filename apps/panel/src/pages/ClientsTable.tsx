import { useTranslation } from "react-i18next";
import { Badge } from "../components/ui/badge";
import { Card, CardContent } from "../components/ui/card";
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import { DataTable, MobileCard, MobileCardList, RowActions, StatusBadge } from "../components/ui";
import type { Client, Group } from "../api/types";
import { formatBytes, formatDateTime } from "../utils/format";

interface ClientsTableProps {
  clients: Client[];
  groups: Group[];
  isLoading: boolean;
  resetStrategyLabel: (strategy: string) => string;
  onEdit: (client: Client) => void;
  onResetTraffic: (client: Client) => void;
  onTrafficDetail: (client: Client) => void;
  onDelete: (clientId: string) => void;
}

function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds === undefined) return "-";
  const parts: string[] = [];
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0) parts.push(`${hours}h`);
  if (mins > 0) parts.push(`${mins}m`);
  if (secs > 0) parts.push(`${secs}s`);
  return parts.join(" ") || "0s";
}

export function ClientsTable({
  clients,
  groups,
  isLoading,
  resetStrategyLabel,
  onEdit,
  onResetTraffic,
  onTrafficDetail,
  onDelete,
}: ClientsTableProps) {
  const { t } = useTranslation();

  const groupNames = (ids: string[]) =>
    ids.map((id) => groups.find((g) => g.id === id)?.name || id).join(", ") || "-";

  const actions = (client: Client) => (
    <RowActions
      label={client.name}
      primaryAction={{ label: t("common.edit"), onSelect: () => onEdit(client) }}
      actions={[
        { label: t("clients.resetTraffic"), onSelect: () => onResetTraffic(client) },
        { label: t("clients.trafficDetail"), onSelect: () => onTrafficDetail(client) },
        { label: t("common.delete"), onSelect: () => onDelete(client.id), destructive: true },
      ]}
    />
  );

  return (
    <Card>
      <CardContent className="space-y-4 md:space-y-0">
        <div className="hidden md:block">
          <DataTable
            aria-label="clients"
            columns={11}
            loading={isLoading}
            empty={clients.length === 0 ? t("common.empty") : undefined}
            header={
              <TableRow>
                <TableHead>{t("common.name")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("clients.email")}</TableHead>
                <TableHead>{t("common.status")}</TableHead>
                <TableHead>
                  {t("clients.trafficUsed")} / {t("clients.trafficLimit")}
                </TableHead>
                <TableHead className="hidden xl:table-cell">{t("clients.allTimeUsed")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("clients.onHold")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("clients.expiryDate")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("clients.resetStrategy")}</TableHead>
                <TableHead className="hidden xl:table-cell">{t("clients.maxDevices")}</TableHead>
                <TableHead className="hidden lg:table-cell">{t("clients.groups")}</TableHead>
                <TableHead className="text-right">{t("common.actions")}</TableHead>
              </TableRow>
            }
          >
            {clients.map((client) => (
              <TableRow key={client.id}>
                <TableCell>
                  <div className="flex max-w-48 items-center gap-2 break-all">
                    {client.name}
                    {client.is_exceeded && (
                      <Badge variant="destructive">{t("clients.exceeded")}</Badge>
                    )}
                  </div>
                </TableCell>
                <TableCell className="hidden lg:table-cell">{client.email || "-"}</TableCell>
                <TableCell>
                  <StatusBadge status={client.status} />
                </TableCell>
                <TableCell className="font-mono text-xs">
                  {formatBytes(client.traffic_used_bytes)} /{" "}
                  {formatBytes(client.traffic_limit_bytes)}
                </TableCell>
                <TableCell className="hidden font-mono text-xs xl:table-cell">
                  {formatBytes(client.all_time_used_bytes)}
                </TableCell>
                <TableCell className="hidden xl:table-cell">
                  <div className="space-y-1">
                    {client.status === "on_hold" && (
                      <Badge variant="secondary" className="bg-warning/10 text-warning">
                        {t("clients.onHold")}
                      </Badge>
                    )}
                    {client.on_hold_timeout && (
                      <p className="text-xs text-muted-foreground">
                        {t("clients.onHoldTimeout")}: {formatDateTime(client.on_hold_timeout)}
                      </p>
                    )}
                    {client.on_hold_expire_duration_secs !== null && (
                      <p className="text-xs text-muted-foreground">
                        {t("clients.onHoldDuration")}:{" "}
                        {formatDuration(client.on_hold_expire_duration_secs)}
                      </p>
                    )}
                  </div>
                </TableCell>
                <TableCell className="hidden lg:table-cell">
                  {formatDateTime(client.expiry_date)}
                </TableCell>
                <TableCell className="hidden xl:table-cell">
                  {resetStrategyLabel(client.data_limit_reset_strategy)}
                </TableCell>
                <TableCell className="hidden xl:table-cell">{client.max_devices ?? "-"}</TableCell>
                <TableCell className="hidden max-w-xs truncate lg:table-cell">
                  {groupNames(client.group_ids || [])}
                </TableCell>
                <TableCell>{actions(client)}</TableCell>
              </TableRow>
            ))}
          </DataTable>
        </div>
        {!isLoading && clients.length > 0 && (
          <MobileCardList>
            {clients.map((client) => (
              <MobileCard
                key={client.id}
                title={client.name}
                status={<StatusBadge status={client.status} />}
                fields={[
                  { label: t("clients.email"), value: client.email || "-" },
                  {
                    label: t("clients.trafficUsed"),
                    value: `${formatBytes(client.traffic_used_bytes)} / ${formatBytes(client.traffic_limit_bytes)}`,
                  },
                  { label: t("clients.groups"), value: groupNames(client.group_ids || []) },
                  { label: t("clients.expiryDate"), value: formatDateTime(client.expiry_date) },
                ]}
                actions={actions(client)}
              />
            ))}
          </MobileCardList>
        )}
      </CardContent>
    </Card>
  );
}
