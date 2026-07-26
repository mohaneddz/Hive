import type { TimelineBucket } from "@/types/media";
import { cn } from "@/utils/cn";

/** A slim clickable year rail, letting the grid be jumped to a given year at a glance. */
export function YearScrubber({
  buckets,
  pending,
  onSelect,
}: {
  buckets: TimelineBucket[];
  /** The year currently being scrolled/loaded to, for a lightweight busy affordance. */
  pending: string | null;
  onSelect: (year: string) => void;
}) {
  if (buckets.length === 0) return null;

  return (
    <nav
      aria-label="Jump to year"
      className="sticky top-0 flex max-h-[calc(100vh-160px)] shrink-0 flex-col items-end gap-1 overflow-y-auto py-1 pl-2"
    >
      {buckets.map((bucket) => (
        <button
          key={bucket.key}
          onClick={() => onSelect(bucket.key)}
          className={cn(
            "rounded-md px-1.5 py-0.5 text-[11px] font-bold text-ink-muted transition hover:text-honey-deep",
            pending === bucket.key && "animate-pulse text-honey-deep",
          )}
          title={`${bucket.count} items in ${bucket.label}`}
        >
          {bucket.label}
        </button>
      ))}
    </nav>
  );
}
