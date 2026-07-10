import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getOnlines } from "../api/onlines";
import { OnlineSession } from "../api/types";
import { formatDateTime } from "../utils/format";

export function Onlines() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination(1, 20);
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
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="onlines">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>
                        {t("onlines.client")}
                      </Table.Column>
                      <Table.Column>{t("onlines.node")}</Table.Column>
                      <Table.Column>{t("onlines.ip")}</Table.Column>
                      <Table.Column>{t("onlines.inbound")}</Table.Column>
                      <Table.Column>{t("onlines.connectedAt")}</Table.Column>
                      <Table.Column>{t("onlines.lastActive")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("onlines.empty")}
                        </div>
                      )}
                    >
                      {display.map((s) => (
                        <Table.Row key={s.id}>
                          <Table.Cell>{s.client_id}</Table.Cell>
                          <Table.Cell>{s.node_id}</Table.Cell>
                          <Table.Cell>{s.ip_address}</Table.Cell>
                          <Table.Cell>{s.inbound_tag || "-"}</Table.Cell>
                          <Table.Cell>
                            {formatDateTime(s.connected_at)}
                          </Table.Cell>
                          <Table.Cell>
                            {formatDateTime(s.last_active_at)}
                          </Table.Cell>
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
