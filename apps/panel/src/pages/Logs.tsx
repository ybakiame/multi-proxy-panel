import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { PageHeader, Pagination, SearchInput, FormSelect } from "../components/ui";
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
          disabled={isFetching}
          onClick={() => {
            void refetch();
          }}
        >
          {t("common.refresh")}
        </Button>
      </form>
      {isError && (
        <div role="alert" className="rounded-lg border border-destructive p-4 text-destructive">
          {t("logs.loadError")}
        </div>
      )}
      <Card>
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : isError && !data ? null : (
            <>
              <Table aria-label={t("logs.title")}>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("logs.level")}</TableHead>
                    <TableHead>{t("logs.source")}</TableHead>
                    <TableHead>{t("logs.message")}</TableHead>
                    <TableHead>{t("logs.time")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {logs.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={4} className="p-4 text-center text-muted-foreground">
                        {t("logs.empty")}
                      </TableCell>
                    </TableRow>
                  ) : (
                    logs.map((log) => (
                      <TableRow key={log.id}>
                        <TableCell>{log.level}</TableCell>
                        <TableCell>
                          <div className="max-w-xs break-all">{log.source}</div>
                          {log.node_id && (
                            <div className="text-sm text-muted-foreground">
                              {log.node_name ?? log.node_id}
                            </div>
                          )}
                        </TableCell>
                        <TableCell className="max-w-xs">
                          <details>
                            <summary className="cursor-pointer truncate" title={t("logs.expand")}>
                              {log.message}
                            </summary>
                            <pre className="mt-2 whitespace-pre-wrap break-all font-mono text-sm">
                              {log.message}
                            </pre>
                          </details>
                        </TableCell>
                        <TableCell>{formatDateTime(log.created_at)}</TableCell>
                      </TableRow>
                    ))
                  )}
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
        </CardContent>
      </Card>
    </div>
  );
}
