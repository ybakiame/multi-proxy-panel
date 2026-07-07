import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Select,
  SelectItem,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
} from "@heroui/react";
import { PageHeader, Pagination, SearchInput } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getLogs } from "../api/logs";
import { Log } from "../api/types";
import { formatDateTime } from "../utils/format";

const LEVELS = ["all", "info", "warn", "error", "debug"];

export function Logs() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages, reset } = usePagination(1, 20);
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
        <Select
          label={t("logs.level")}
          selectedKeys={[level]}
          onSelectionChange={(keys) => {
            setLevel((Array.from(keys)[0] as string) || "all");
          }}
          className="min-w-[160px]"
        >
          {LEVELS.map((l) => (
            <SelectItem key={l}>{l.toUpperCase()}</SelectItem>
          ))}
        </Select>
        <SearchInput
          value={source}
          onChange={setSource}
          placeholder={t("logs.source")}
        />
        <Button onPress={handleFilter}>{t("common.filter")}</Button>
      </div>
      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table removeWrapper aria-label="logs">
                <TableHeader>
                  <TableColumn>{t("logs.level")}</TableColumn>
                  <TableColumn>{t("logs.source")}</TableColumn>
                  <TableColumn>{t("logs.message")}</TableColumn>
                  <TableColumn>{t("logs.time")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("logs.empty")}>
                  {logs.map((log) => (
                    <TableRow key={log.id}>
                      <TableCell>{log.level}</TableCell>
                      <TableCell>{log.source}</TableCell>
                      <TableCell className="max-w-xs truncate">{log.message}</TableCell>
                      <TableCell>{formatDateTime(log.created_at)}</TableCell>
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
