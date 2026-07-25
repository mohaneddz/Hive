import type { ReactNode } from "react";

import { cn } from "@/utils/cn";

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "mt-12 flex flex-col items-center gap-4 rounded-3xl border border-dashed border-ink/[.15] p-16 text-center",
        className,
      )}
    >
      <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
        {icon}
      </div>
      <div>
        <p className="text-sm font-extrabold text-ink">{title}</p>
        <p className="mt-1 max-w-md text-xs text-ink-muted">{description}</p>
      </div>
      {action}
    </div>
  );
}
