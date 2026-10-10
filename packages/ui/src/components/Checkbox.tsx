import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { createContext, useContext, type ReactNode } from "react";
import { CheckIcon } from "../icons";
import { cx } from "../utils";

interface CheckboxProps {
  isSelected?: boolean;
  isDisabled?: boolean;
  onChange?: (checked: boolean) => void;
  className?: string;
  "aria-label"?: string;
  children?: ReactNode;
}
const Context = createContext<CheckboxProps>({});
function Root({ children, className, ...props }: CheckboxProps) {
  return (
    <Context.Provider value={props}>
      <label
        className={cx(
          "inline-flex cursor-pointer items-center gap-2 text-sm text-foreground",
          props.isDisabled && "opacity-50",
          className,
        )}
      >
        {children}
      </label>
    </Context.Provider>
  );
}
function Control({ children, className }: { children?: ReactNode; className?: string }) {
  const props = useContext(Context);
  return (
    <BaseCheckbox.Root
      checked={props.isSelected}
      disabled={props.isDisabled}
      onCheckedChange={props.onChange}
      aria-label={props["aria-label"]}
      className={cx(
        "inline-flex size-5 shrink-0 items-center justify-center rounded border border-border bg-field-background outline-none data-checked:border-accent data-checked:bg-accent data-checked:text-accent-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
        className,
      )}
    >
      {children}
    </BaseCheckbox.Root>
  );
}
function Indicator() {
  return (
    <BaseCheckbox.Indicator>
      <CheckIcon className="size-4" />
    </BaseCheckbox.Indicator>
  );
}
function Content({ children, className }: { children?: ReactNode; className?: string }) {
  return <span className={cx("inline-flex items-center gap-2", className)}>{children}</span>;
}
export const Checkbox = Object.assign(Root, { Control, Indicator, Content });
