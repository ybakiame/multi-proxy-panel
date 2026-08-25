import { useState } from "react";
import { Button } from "@heroui/react";

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
  const [selectedPerPage, setSelectedPerPage] = useState(perPage.toString());

  const handlePerPageChange = (value: string) => {
    setSelectedPerPage(value);
    onPerPageChange(Number(value));
  };

  return (
    <div className="flex items-center justify-between gap-4 py-4">
      <div className="text-sm text-muted-foreground">
        Page {page} of {totalPages} ({total} total)
      </div>
      <div className="flex items-center gap-2">
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          isDisabled={page <= 1}
          onPress={() => onPageChange(1)}
        >
          «
        </Button>
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          isDisabled={page <= 1}
          onPress={() => onPageChange(page - 1)}
        >
          ‹
        </Button>
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          isDisabled={page >= totalPages}
          onPress={() => onPageChange(page + 1)}
        >
          ›
        </Button>
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          isDisabled={page >= totalPages}
          onPress={() => onPageChange(totalPages)}
        >
          »
        </Button>
      </div>
      <select
        className="rounded-md border border-border bg-surface px-2 py-1 text-sm text-foreground"
        value={selectedPerPage}
        onChange={(e) => handlePerPageChange(e.target.value)}
      >
        <option value="10">10 / page</option>
        <option value="20">20 / page</option>
        <option value="50">50 / page</option>
        <option value="100">100 / page</option>
      </select>
    </div>
  );
}
