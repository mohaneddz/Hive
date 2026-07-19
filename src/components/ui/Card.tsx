import type { HTMLAttributes } from "react";

import { cn } from "@/utils/cn";

export function Card({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "rounded-[22px] border border-ink/[.07] bg-panel shadow-[0_12px_40px_rgba(75,52,10,.055)]",
        className,
      )}
      {...props}
    />
  );
}
