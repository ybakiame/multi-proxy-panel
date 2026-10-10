import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** className 合并助手：clsx 条件拼接 + tailwind-merge 冲突去重（后者覆盖前者）。 */
export function cx(...inputs: ClassValue[]): string {
  return twMerge(clsx(...inputs));
}
export type { ClassValue };
