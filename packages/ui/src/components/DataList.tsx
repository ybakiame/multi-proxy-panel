import type { ReactNode } from "react";
import { IS_MOBILE } from "../platform";

export interface DataListColumn<T> {
  id: string;
  header: ReactNode;
  accessorKey?: keyof T;
  cell?: (row: T, index: number) => ReactNode;
  className?: string;
}
export interface DataListProps<T> {
  "aria-label": string;
  rows: T[];
  columns: DataListColumn<T>[];
  getRowId: (row: T) => string;
  renderCard: (row: T, index: number) => ReactNode;
  emptyContent?: ReactNode;
}
/** 原生 table 语义与移动卡片列表；排序、分页状态留在调用方。 */
export function DataList<T>({
  rows,
  columns,
  getRowId,
  renderCard,
  emptyContent,
  "aria-label": label,
}: DataListProps<T>) {
  if (rows.length === 0)
    return (
      <div className="rounded-xl border border-border bg-surface p-6 text-center text-sm text-muted">
        {emptyContent ?? "暂无数据"}
      </div>
    );
  if (IS_MOBILE)
    return (
      <ul aria-label={label} className="flex flex-col gap-2">
        {rows.map((row, index) => (
          <li key={getRowId(row)}>{renderCard(row, index)}</li>
        ))}
      </ul>
    );
  return (
    <div className="overflow-x-auto rounded-xl border border-border bg-surface">
      <table aria-label={label} className="w-full text-left text-sm">
        <thead className="border-b border-border bg-surface-secondary text-muted">
          <tr>
            {columns.map((column) => (
              <th key={column.id} scope="col" className="px-4 py-3 font-medium">
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={getRowId(row)} className="border-b border-separator last:border-0">
              {columns.map((column) => (
                <td key={column.id} className={column.className ?? "px-4 py-3"}>
                  {column.cell
                    ? column.cell(row, index)
                    : column.accessorKey
                      ? String(row[column.accessorKey] ?? "")
                      : null}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
