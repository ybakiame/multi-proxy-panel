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
import { getTraffic } from "../api/traffic";
import { TrafficRecord } from "../api/types";
import { formatBytes } from "../utils/format";

export function Traffic() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination(1, 20);
  const [records, setRecords] = useState<TrafficRecord[]>([]);
  const [loading, setLoading] = useState(false);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getTraffic();
      setRecords(res.data);
      setTotal(res.data.length);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, []);

  const display = records.slice((page - 1) * perPage, page * perPage);

  return (
    <div className="space-y-4">
      <PageHeader title={t("traffic.title")} />
      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table removeWrapper aria-label="traffic">
                <TableHeader>
                  <TableColumn>{t("traffic.hour")}</TableColumn>
                  <TableColumn>{t("traffic.node")}</TableColumn>
                  <TableColumn>{t("traffic.client")}</TableColumn>
                  <TableColumn>{t("traffic.upload")}</TableColumn>
                  <TableColumn>{t("traffic.download")}</TableColumn>
                  <TableColumn>{t("traffic.total")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("traffic.empty")}>
                  {display.map((r) => (
                    <TableRow key={r.id}>
                      <TableCell>{r.hour_bucket}</TableCell>
                      <TableCell>{r.node_id || "-"}</TableCell>
                      <TableCell>{r.client_id || "-"}</TableCell>
                      <TableCell>{formatBytes(r.upload_bytes)}</TableCell>
                      <TableCell>{formatBytes(r.download_bytes)}</TableCell>
                      <TableCell>{formatBytes(r.upload_bytes + r.download_bytes)}</TableCell>
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
