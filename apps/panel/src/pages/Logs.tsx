import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, SearchInput, FormSelect } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getLogs } from "../api/logs";
import { formatDateTime } from "../utils/format";

const LEVELS = ["all", "info", "warn", "error", "debug"];
const logsQueryKey = "logs";

export function Logs() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage } = usePagination(1, 20);
  const [kind, setKind] = useState<"system" | "agent">("system");
  const [level, setLevel] = useState("all");
  const [source, setSource] = useState("");
  const [appliedLevel, setAppliedLevel] = useState("all");
  const [appliedSource, setAppliedSource] = useState("");

  const { data, isLoading, isError, isFetching, refetch } = useQuery({
    queryKey: [logsQueryKey, { kind, page, perPage, level: appliedLevel, source: appliedSource }],
    queryFn: () => getLogs(page, perPage, appliedLevel, appliedSource, kind),
    refetchInterval: page === 1 ? 15000 : false,
  });

  const logs = data?.data ?? [];
  const total = data?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const handleFilter = () => {
    setAppliedLevel(level);
    setAppliedSource(source);
    setPage(1);
  };

  return (
    <div className="space-y-4">
      <PageHeader title={t("logs.title")} />
      <form
        className="flex flex-wrap items-end gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          handleFilter();
        }}
      >
        <FormSelect
          label={t("logs.kind")}
          value={kind}
          onChange={(value) => {
            setKind(value === "agent" ? "agent" : "system");
            setPage(1);
          }}
          options={[
            { id: "system", label: t("logs.system") },
            { id: "agent", label: t("logs.agent") },
          ]}
          className="min-w-[160px]"
        />
        <FormSelect
          label={t("logs.level")}
          value={level}
          onChange={setLevel}
          options={LEVELS.map((l) => ({ id: l, label: l.toUpperCase() }))}
          className="min-w-[160px]"
        />
        <SearchInput value={source} onChange={setSource} placeholder={t("logs.source")} />
        <Button type="submit">{t("common.filter")}</Button>
        <Button
          type="button"
          variant="secondary"
          isDisabled={isFetching}
          onPress={() => {
            void refetch();
          }}
        >
          {t("common.refresh")}
        </Button>
      </form>
      {isError && (
        <div role="alert" className="rounded-lg border border-danger p-4 text-danger">
          {t("logs.loadError")}
        </div>
      )}
      <Card>
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : isError && !data ? null : (
            <>
              <Table aria-label={t("logs.title")}>
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
                          <Table.Cell>
                            <div className="max-w-xs break-all">{log.source}</div>
                            {log.node_id && (
                              <div className="text-sm text-muted-foreground">
                                {log.node_name ?? log.node_id}
                              </div>
                            )}
                          </Table.Cell>
                          <Table.Cell className="max-w-xs">
                            <details>
                              <summary className="cursor-pointer truncate" title={t("logs.expand")}>
                                {log.message}
                              </summary>
                              <pre className="mt-2 whitespace-pre-wrap break-all font-mono text-sm">
                                {log.message}
                              </pre>
                            </details>
                          </Table.Cell>
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
