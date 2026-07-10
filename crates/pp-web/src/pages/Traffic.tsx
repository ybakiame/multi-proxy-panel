import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, Spinner, Table } from "@heroui/react";
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
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="traffic">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("traffic.hour")}</Table.Column>
                      <Table.Column>{t("traffic.node")}</Table.Column>
                      <Table.Column>{t("traffic.client")}</Table.Column>
                      <Table.Column>{t("traffic.upload")}</Table.Column>
                      <Table.Column>{t("traffic.download")}</Table.Column>
                      <Table.Column>{t("traffic.total")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("traffic.empty")}
                        </div>
                      )}
                    >
                      {display.map((r) => (
                        <Table.Row key={r.id}>
                          <Table.Cell>{r.hour_bucket}</Table.Cell>
                          <Table.Cell>{r.node_id || "-"}</Table.Cell>
                          <Table.Cell>{r.client_id || "-"}</Table.Cell>
                          <Table.Cell>{formatBytes(r.upload_bytes)}</Table.Cell>
                          <Table.Cell>{formatBytes(r.download_bytes)}</Table.Cell>
                          <Table.Cell>{formatBytes(r.upload_bytes + r.download_bytes)}</Table.Cell>
                        </Table.Row>
                      ))}
                    </Table.Body>
                  </Table.Content>
                </Table.ScrollContainer>
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
        </Card.Content>
      </Card>
    </div>
  );
}
