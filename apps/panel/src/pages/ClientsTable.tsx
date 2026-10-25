import { useTranslation } from "react-i18next";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { StatusBadge } from "../components/ui";
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

  const groupNames = (ids: string[]) => {
    return ids.map((id) => groups.find((g) => g.id === id)?.name || id).join(", ") || "-";
  };

  return (
    <Card>
      <CardContent>
        {isLoading ? (
          <div className="flex h-32 items-center justify-center">
            <Spinner />
          </div>
        ) : (
          <Table aria-label="clients">
            <TableHeader>
              <TableRow>
                <TableHead>{t("common.name")}</TableHead>
                <TableHead>{t("clients.email")}</TableHead>
                <TableHead>{t("common.status")}</TableHead>
                <TableHead>
                  {t("clients.trafficUsed")} / {t("clients.trafficLimit")}
                </TableHead>
                <TableHead>{t("clients.allTimeUsed")}</TableHead>
                <TableHead>{t("clients.onHold")}</TableHead>
                <TableHead>{t("clients.expiryDate")}</TableHead>
                <TableHead>{t("clients.resetStrategy")}</TableHead>
                <TableHead>{t("clients.maxDevices")}</TableHead>
                <TableHead>{t("clients.groups")}</TableHead>
                <TableHead>{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {clients.length === 0 && (
                <TableRow>
                  <TableCell colSpan={11} className="p-4 text-center text-muted-foreground">
                    {t("common.empty")}
                  </TableCell>
                </TableRow>
              )}
              {clients.map((client) => (
                <TableRow key={client.id}>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      {client.name}
                      {client.is_exceeded && (
                        <Badge variant="destructive">{t("clients.exceeded")}</Badge>
                      )}
                    </div>
                  </TableCell>
                  <TableCell>{client.email || "-"}</TableCell>
                  <TableCell>
                    <StatusBadge status={client.status} />
                  </TableCell>
                  <TableCell>
                    {formatBytes(client.traffic_used_bytes)} /{" "}
                    {formatBytes(client.traffic_limit_bytes)}
                  </TableCell>
                  <TableCell>{formatBytes(client.all_time_used_bytes)}</TableCell>
                  <TableCell>
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
                  <TableCell>{formatDateTime(client.expiry_date)}</TableCell>
                  <TableCell>{resetStrategyLabel(client.data_limit_reset_strategy)}</TableCell>
                  <TableCell>{client.max_devices ?? "-"}</TableCell>
                  <TableCell className="max-w-xs truncate">
                    {groupNames(client.group_ids || [])}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-wrap gap-2">
                      <Button size="sm" variant="ghost" onClick={() => onEdit(client)}>
                        {t("common.edit")}
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => onResetTraffic(client)}>
                        {t("clients.resetTraffic")}
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => onTrafficDetail(client)}>
                        {t("clients.trafficDetail")}
                      </Button>
                      <Button size="sm" variant="destructive" onClick={() => onDelete(client.id)}>
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
  );
}
