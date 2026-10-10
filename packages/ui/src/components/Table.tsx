import {
  Children,
  cloneElement,
  createContext,
  isValidElement,
  useContext,
  type ComponentProps,
  type ReactNode,
} from "react";
import { cx } from "../utils";

const RowHeaderContext = createContext(-1);

function Root({ className, ...props }: ComponentProps<"div">) {
  return <div className={cx("rounded-xl border border-border bg-surface", className)} {...props} />;
}
function ScrollContainer({ className, ...props }: ComponentProps<"div">) {
  return <div className={cx("overflow-x-auto", className)} {...props} />;
}
function Content({ className, children, ...props }: ComponentProps<"table">) {
  const header = Children.toArray(children).find((child) => isValidElement(child) && child.type === Header);
  const columns = isValidElement<{ children?: ReactNode }>(header) ? Children.toArray(header.props.children) : [];
  const rowHeader = columns.findIndex(
    (column) => isValidElement<{ isRowHeader?: boolean }>(column) && column.props.isRowHeader,
  );
  return (
    <RowHeaderContext.Provider value={rowHeader}>
      <table className={cx("w-full border-collapse text-left text-sm", className)} {...props}>
        {children}
      </table>
    </RowHeaderContext.Provider>
  );
}
function Header({ children, ...props }: ComponentProps<"thead">) {
  return (
    <thead {...props}>
      <tr className="border-b border-border bg-surface-secondary text-muted">{children}</tr>
    </thead>
  );
}
function Column({ isRowHeader: _isRowHeader, className, ...props }: ComponentProps<"th"> & { isRowHeader?: boolean }) {
  return <th scope="col" className={cx("px-4 py-3 font-medium whitespace-nowrap", className)} {...props} />;
}
function Body(props: ComponentProps<"tbody">) {
  return <tbody {...props} />;
}
function Row({ className, children, ...props }: ComponentProps<"tr">) {
  const rowHeader = useContext(RowHeaderContext);
  return (
    <tr className={cx("border-b border-separator last:border-0 hover:bg-surface-hover", className)} {...props}>
      {Children.map(children, (child, index) =>
        isValidElement<{ isRowHeader?: boolean }>(child) && child.type === Cell
          ? cloneElement(child, { isRowHeader: index === rowHeader })
          : child,
      )}
    </tr>
  );
}
function Cell({ isRowHeader, className, ...props }: ComponentProps<"td"> & { isRowHeader?: boolean }) {
  const classes = cx("px-4 py-3 text-foreground", className);
  return isRowHeader ? (
    <th scope="row" className={cx("font-normal", classes)} {...props} />
  ) : (
    <td className={classes} {...props} />
  );
}
export const Table = Object.assign(Root, { ScrollContainer, Content, Header, Column, Body, Row, Cell });
