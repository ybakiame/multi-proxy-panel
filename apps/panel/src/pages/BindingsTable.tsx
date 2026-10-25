import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { Binding, Node, ProtocolConfig } from "../api/types";

interface BindingsTableProps {
  bindings: Binding[];
  nodes: Node[];
  protocols: ProtocolConfig[];
  onEdit: (binding: Binding) => void;
  onDelete: (bindingId: string) => void;
}

export function BindingsTable({
  bindings,
  nodes,
  protocols,
  onEdit,
  onDelete,
}: BindingsTableProps) {
  const { t } = useTranslation();

  const getNodeName = (nodeId: string) => {
    const node = nodes.find((n) => n.id === nodeId);
    return node ? node.name : nodeId;
  };

  const getProtocolName = (protocolId: string) => {
    const protocol = protocols.find((p) => p.id === protocolId);
    return protocol ? protocol.name : protocolId;
  };

  return (
    <Table aria-label="bindings">
      <TableHeader>
        <TableRow>
          <TableHead>{t("bindings.node")}</TableHead>
          <TableHead>{t("bindings.protocol")}</TableHead>
          <TableHead>{t("common.active")}</TableHead>
          <TableHead>{t("bindings.overrideSettings")}</TableHead>
          <TableHead>{t("common.actions")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {bindings.length === 0 && (
          <TableRow>
            <TableCell colSpan={5} className="p-4 text-center text-muted-foreground">
              {t("common.empty")}
            </TableCell>
          </TableRow>
        )}
        {bindings.map((binding) => (
          <TableRow key={binding.id}>
            <TableCell>{getNodeName(binding.node_id)}</TableCell>
            <TableCell>{getProtocolName(binding.protocol_config_id)}</TableCell>
            <TableCell>{binding.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
            <TableCell className="max-w-xs truncate font-mono">
              {binding.override_settings ? JSON.stringify(binding.override_settings) : "-"}
            </TableCell>
            <TableCell>
              <div className="flex gap-2">
                <Button size="sm" variant="ghost" onClick={() => onEdit(binding)}>
                  {t("common.edit")}
                </Button>
                <Button size="sm" variant="destructive" onClick={() => onDelete(binding.id)}>
                  {t("common.delete")}
                </Button>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
