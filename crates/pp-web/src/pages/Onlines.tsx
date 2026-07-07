import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Card,
  CardBody,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
} from "@heroui/react";
import { PageHeader, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getOnlines } from "../api/onlines";
import { OnlineSession } from "../api/types";
import { formatDateTime } from "../utils/format";

export function Onlines() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination(1, 20);
  const [sessions, setSessions] = useState<OnlineSession[]>([]);
  const [loading, setLoading] = useState(false);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getOnlines();
      setSessions(res.data);
      setTotal(res.data.length);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, []);

  const display = sessions.slice((page - 1) * perPage, page * perPage);

  return (
    <div className="space-y-4">
      <PageHeader title={t("onlines.title")} />
      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table removeWrapper aria-label="onlines">
                <TableHeader>
                  <TableColumn>{t("onlines.client")}</TableColumn>
                  <TableColumn>{t("onlines.node")}</TableColumn>
                  <TableColumn>{t("onlines.ip")}</TableColumn>
                  <TableColumn>{t("onlines.inbound")}</TableColumn>
                  <TableColumn>{t("onlines.connectedAt")}</TableColumn>
                  <TableColumn>{t("onlines.lastActive")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("onlines.empty")}>
                  {display.map((s) => (
                    <TableRow key={s.id}>
                      <TableCell>{s.client_id}</TableCell>
                      <TableCell>{s.node_id}</TableCell>
                      <TableCell>{s.ip_address}</TableCell>
                      <TableCell>{s.inbound_tag || "-"}</TableCell>
                      <TableCell>{formatDateTime(s.connected_at)}</TableCell>
                      <TableCell>{formatDateTime(s.last_active_at)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
              <Pagination
                page={page}
                totalPages={totalPages}
                perPage={perPage}
                total={total}
                onPageChange={setPage}
                onPerPageChange={setPerPage}
              />
            </>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
