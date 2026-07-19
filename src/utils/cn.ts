import { twMerge } from "tailwind-merge";

type ClassValue = string | false | null | undefined;

export function cn(...values: ClassValue[]) {
  return twMerge(values.filter(Boolean).join(" "));
}
