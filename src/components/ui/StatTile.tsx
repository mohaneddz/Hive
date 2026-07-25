import type { ReactNode } from "react";
import { Link } from "react-router-dom";

import { cn } from "@/utils/cn";

interface StatTileProps {
  icon: ReactNode;
  label: string;
  value: string;
  hint?: string;
  to?: string;
  className?: string;
}

/** A single number with a label. Becomes a link when `to` is given. */
export function StatTile({ icon, label, value, hint, to, className }: StatTileProps) {
  const body = (
    <>
      <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
        {icon}
      </div>
      <div className="min-w-0">
        <p className="truncate text-[22px] font-extrabold leading-none tracking-[-.03em] text-ink">
          {value}
        </p>
        <p className="mt-1 truncate text-[11px] font-bold text-ink-muted">{label}</p>
        {hint && <p className="mt-0.5 truncate text-[10px] text-ink-muted">{hint}</p>}
      </div>
    </>
  );

  const classes = cn(
    "flex items-center gap-3.5 rounded-[18px] border border-ink/[.07] bg-panel p-4 shadow-[0_12px_40px_rgba(75,52,10,.055)]",
    to && "transition hover:-translate-y-px hover:border-honey/40",
    className,
  );

  if (to) {
    return (
      <Link to={to} className={classes}>
        {body}
      </Link>
    );
  }
  return <div className={classes}>{body}</div>;
}
