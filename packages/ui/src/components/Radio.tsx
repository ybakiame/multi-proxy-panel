import { Radio as BaseRadio } from "@base-ui/react/radio";
import { RadioGroup as BaseRadioGroup } from "@base-ui/react/radio-group";
import { createContext, useContext, type ReactNode } from "react";
import { cx } from "../utils";

interface RadioProps {
  value: string;
  isDisabled?: boolean;
  className?: string;
  children?: ReactNode;
}
const Context = createContext<RadioProps>({ value: "" });
function Root({ className, children, ...props }: RadioProps) {
  return (
    <Context.Provider value={props}>
      <label className={cx("inline-flex cursor-pointer items-center gap-2", className)}>{children}</label>
    </Context.Provider>
  );
}
function Control({ children }: { children?: ReactNode }) {
  const { value, isDisabled } = useContext(Context);
  return (
    <BaseRadio.Root
      value={value}
      disabled={isDisabled}
      className="inline-flex size-5 shrink-0 items-center justify-center rounded-full border border-border outline-none data-checked:border-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
    >
      {children}
    </BaseRadio.Root>
  );
}
function Indicator() {
  return <BaseRadio.Indicator className="size-2.5 rounded-full bg-accent" />;
}
function Content({ className, children }: { className?: string; children?: ReactNode }) {
  return <span className={cx("inline-flex items-center gap-2", className)}>{children}</span>;
}
export const Radio = Object.assign(Root, { Control, Indicator, Content });
export function RadioGroup({
  value,
  onChange,
  isDisabled,
  children,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  isDisabled?: boolean;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <BaseRadioGroup
      value={value}
      onValueChange={onChange}
      disabled={isDisabled}
      className={cx("flex flex-col gap-2", className)}
    >
      {children}
    </BaseRadioGroup>
  );
}
