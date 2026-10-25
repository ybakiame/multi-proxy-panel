import { useTranslation } from "react-i18next";
import { Button } from "./button";

interface PaginationProps {
  page: number;
  totalPages: number;
  perPage: number;
  total: number;
  onPageChange: (page: number) => void;
  onPerPageChange: (perPage: number) => void;
}

export function Pagination({
  page,
  totalPages,
  perPage,
  total,
  onPageChange,
  onPerPageChange,
}: PaginationProps) {
  const { t } = useTranslation();

  const handlePerPageChange = (value: string) => {
    onPageChange(1);
    onPerPageChange(Number(value));
  };

  return (
    <div className="flex items-center justify-between gap-4 py-4">
      <div className="text-sm text-muted-foreground">
        {t("pagination.pageInfo", { current: page, total: totalPages, count: total })}
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={page <= 1}
          onClick={() => onPageChange(1)}
          aria-label={t("pagination.first")}
        >
          «
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
          aria-label={t("pagination.prev")}
        >
          ‹
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={page >= totalPages}
          onClick={() => onPageChange(page + 1)}
          aria-label={t("pagination.next")}
        >
          ›
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={page >= totalPages}
          onClick={() => onPageChange(totalPages)}
          aria-label={t("pagination.last")}
        >
          »
        </Button>
      </div>
      <select
        aria-label={t("pagination.perPage")}
        className="rounded-md border border-input bg-background px-2 py-1 text-sm text-foreground"
        value={perPage.toString()}
        onChange={(event) => handlePerPageChange(event.target.value)}
      >
        <option value="10">10 / page</option>
        <option value="20">20 / page</option>
        <option value="50">50 / page</option>
        <option value="100">100 / page</option>
      </select>
    </div>
  );
}
