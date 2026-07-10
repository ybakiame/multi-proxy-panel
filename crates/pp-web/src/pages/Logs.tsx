import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, SearchInput, FormSelect } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getLogs } from "../api/logs";
import { Log } from "../api/types";
import { formatDateTime } from "../utils/format";

const LEVELS = ["all", "info", "warn", "error", "debug"];

export function Logs() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages, reset } = usePagination(
    1,
    20,
  );
  const [logs, setLogs] = useState<Log[]>([]);
  const [level, setLevel] = useState("all");
  const [source, setSource] = useState("");
  const [appliedLevel, setAppliedLevel] = useState("all");
  const [appliedSource, setAppliedSource] = useState("");
  const [loading, setLoading] = useState(false);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getLogs(page, perPage, appliedLevel, appliedSource);
      setLogs(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage, appliedLevel, appliedSource]);

  const handleFilter = () => {
    setAppliedLevel(level);
    setAppliedSource(source);
    reset();
  };

  return (
    <div className="space-y-4">
      <PageHeader title={t("logs.title")} />
      <div className="flex flex-wrap items-end gap-4">
        <FormSelect
          label={t("logs.level")}
          value={level}
          onChange={setLevel}
          options={LEVELS.map((l) => ({ id: l, label: l.toUpperCase() }))}
          className="min-w-[160px]"
        />
        <SearchInput value={source} onChange={setSource} placeholder={t("logs.source")} />
        <Button onPress={handleFilter}>{t("common.filter")}</Button>
      </div>
      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="logs">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("logs.level")}</Table.Column>
                      <Table.Column>{t("logs.source")}</Table.Column>
                      <Table.Column>{t("logs.message")}</Table.Column>
                      <Table.Column>{t("logs.time")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("logs.empty")}
                        </div>
                      )}
                    >
                      {logs.map((log) => (
                        <Table.Row key={log.id}>
                          <Table.Cell>{log.level}</Table.Cell>
                          <Table.Cell>{log.source}</Table.Cell>
                          <Table.Cell className="max-w-xs truncate">{log.message}</Table.Cell>
                          <Table.Cell>{formatDateTime(log.created_at)}</Table.Cell>
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
