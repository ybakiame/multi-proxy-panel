import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Spinner } from "./spinner";
import { Table, TableBody, TableCell, TableHeader, TableRow } from "./table";

interface DataTableProps extends React.ComponentProps<"table"> {
  columns: number;
  header: ReactNode;
  children: ReactNode;
  loading?: boolean;
  error?: string;
  empty?: ReactNode;
}

export function DataTable({
  columns,
  header,
  children,
  loading = false,
  error,
  empty,
  ...props
}: DataTableProps) {
  const { t } = useTranslation();

  return (
    <Table {...props}>
      <TableHeader>{header}</TableHeader>
      <TableBody>
        {loading ? (
          <TableRow>
            <TableCell colSpan={columns} className="h-32 text-center">
              <span className="inline-flex items-center gap-2 text-muted-foreground">
                <Spinner /> {t("common.loading")}
              </span>
            </TableCell>
          </TableRow>
        ) : error ? (
          <TableRow>
            <TableCell colSpan={columns} className="h-24 text-center text-destructive">
              <span role="alert">{error}</span>
            </TableCell>
          </TableRow>
        ) : empty ? (
          <TableRow>
            <TableCell colSpan={columns} className="h-24 text-center text-muted-foreground">
              {empty}
            </TableCell>
          </TableRow>
        ) : (
          children
        )}
      </TableBody>
    </Table>
  );
}
