import { useEffect, useRef, useState } from "react";
import { CalendarRange, Filter, Infinity as InfinityIcon, LayoutGrid, Minus, Plus } from "lucide-react";

import type { MediaSort, MediaType, TimelineGranularity } from "@/types/media";
import { cn } from "@/utils/cn";

const GRANULARITIES: { value: TimelineGranularity; label: string }[] = [
  { value: "day", label: "Day" },
  { value: "month", label: "Month" },
  { value: "year", label: "Year" },
];

const SORTS: { value: MediaSort; label: string }[] = [
  { value: "taken", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "added", label: "Recently added" },
  { value: "viewed", label: "Recently viewed" },
  { value: "name", label: "Name" },
  { value: "size", label: "File size" },
];

const MIN_COLUMNS = 3;
const MAX_COLUMNS = 8;

export interface GalleryFilters {
  mediaType: MediaType | "all";
  favoritesOnly: boolean;
}

function useClickOutside(onOutside: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const handler = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) onOutside();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onOutside]);
  return ref;
}

export function GalleryToolbar({
  view,
  onViewChange,
  granularity,
  onGranularityChange,
  sort,
  onSortChange,
  filters,
  onFiltersChange,
  columns,
  onColumnsChange,
  autoLoad,
  onAutoLoadChange,
}: {
  view: "grid" | "timeline";
  onViewChange: (view: "grid" | "timeline") => void;
  granularity: TimelineGranularity;
  onGranularityChange: (value: TimelineGranularity) => void;
  sort: MediaSort;
  onSortChange: (value: MediaSort) => void;
  filters: GalleryFilters;
  onFiltersChange: (filters: GalleryFilters) => void;
  columns: number;
  onColumnsChange: (columns: number) => void;
  autoLoad: boolean;
  onAutoLoadChange: (value: boolean) => void;
}) {
  const [filterOpen, setFilterOpen] = useState(false);
  const filterRef = useClickOutside(() => setFilterOpen(false));
  const filterActive = filters.mediaType !== "all" || filters.favoritesOnly;

  return (
    <div className="mt-7 flex flex-wrap items-center gap-3">
      <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
        <button
          onClick={() => onViewChange("grid")}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
            view === "grid" && "bg-honey/15 text-honey-deep",
          )}
        >
          <LayoutGrid size={13} /> Grid
        </button>
        <button
          onClick={() => onViewChange("timeline")}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
            view === "timeline" && "bg-honey/15 text-honey-deep",
          )}
        >
          <CalendarRange size={13} /> Timeline
        </button>
      </div>

      {view === "grid" && (
        <>
          <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
            {GRANULARITIES.map((entry) => (
              <button
                key={entry.value}
                onClick={() => onGranularityChange(entry.value)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
                  granularity === entry.value && "bg-honey/15 text-honey-deep",
                )}
              >
                {entry.label}
              </button>
            ))}
          </div>

          <select
            value={sort}
            onChange={(event) => onSortChange(event.target.value as MediaSort)}
            className="h-9 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink outline-none"
            aria-label="Sort by"
          >
            {SORTS.map((entry) => (
              <option key={entry.value} value={entry.value}>
                Sort: {entry.label}
              </option>
            ))}
          </select>

          <div ref={filterRef} className="relative">
            <button
              onClick={() => setFilterOpen((v) => !v)}
              className={cn(
                "inline-flex h-9 items-center gap-2 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink",
                filterActive && "border-honey/50 bg-honey/12 text-honey-deep",
              )}
            >
              <Filter size={13} /> Filter
            </button>

            {filterOpen && (
              <div className="absolute left-0 top-[calc(100%+6px)] z-20 w-56 rounded-2xl border border-ink/[.1] bg-panel p-3 shadow-xl">
                <p className="px-1 text-[10px] font-extrabold uppercase tracking-wide text-ink-muted">Media type</p>
                <div className="mt-1.5 flex items-center gap-1 rounded-lg border border-ink/[.1] bg-canvas p-1">
                  {(["all", "image", "video"] as const).map((value) => (
                    <button
                      key={value}
                      onClick={() => onFiltersChange({ ...filters, mediaType: value })}
                      className={cn(
                        "flex-1 rounded-md px-2 py-1 text-[11px] font-bold text-ink-muted transition",
                        filters.mediaType === value && "bg-honey/15 text-honey-deep",
                      )}
                    >
                      {value === "all" ? "All" : value === "image" ? "Photos" : "Videos"}
                    </button>
                  ))}
                </div>

                <button
                  onClick={() => onFiltersChange({ ...filters, favoritesOnly: !filters.favoritesOnly })}
                  className={cn(
                    "mt-3 flex w-full items-center justify-between rounded-lg border border-ink/[.1] px-2.5 py-1.5 text-[11px] font-bold text-ink transition",
                    filters.favoritesOnly && "border-honey/50 bg-honey/12 text-honey-deep",
                  )}
                >
                  Favorites only
                  <span
                    className={cn(
                      "grid size-4 place-items-center rounded-full border-2",
                      filters.favoritesOnly ? "border-honey bg-honey" : "border-ink/20",
                    )}
                  />
                </button>
              </div>
            )}
          </div>

          <div className="ml-auto flex items-center gap-2 rounded-xl border border-ink/[.12] bg-panel px-2 py-1">
            <button
              onClick={() => onColumnsChange(Math.min(MAX_COLUMNS, columns + 1))}
              disabled={columns >= MAX_COLUMNS}
              className="icon-button size-7 border-0 bg-transparent disabled:opacity-30"
              aria-label="Zoom out"
              title="Zoom out"
            >
              <Minus size={13} />
            </button>
            <input
              type="range"
              min={MIN_COLUMNS}
              max={MAX_COLUMNS}
              // A smaller column count means bigger tiles, so the slider is inverted to read as
              // "zoom" left-to-right: dragging right shrinks the column count, growing the tiles.
              value={MAX_COLUMNS + MIN_COLUMNS - columns}
              onChange={(event) => onColumnsChange(MAX_COLUMNS + MIN_COLUMNS - Number(event.target.value))}
              className="w-24 accent-honey"
              aria-label="Zoom"
            />
            <button
              onClick={() => onColumnsChange(Math.max(MIN_COLUMNS, columns - 1))}
              disabled={columns <= MIN_COLUMNS}
              className="icon-button size-7 border-0 bg-transparent disabled:opacity-30"
              aria-label="Zoom in"
              title="Zoom in"
            >
              <Plus size={13} />
            </button>
            <button
              onClick={() => onAutoLoadChange(!autoLoad)}
              className={cn("icon-button size-7 border-0 bg-transparent", autoLoad && "text-honey-deep")}
              aria-pressed={autoLoad}
              aria-label={autoLoad ? "Disable infinite scroll" : "Enable infinite scroll"}
              title={autoLoad ? "Infinite scroll on" : "Infinite scroll off"}
            >
              <InfinityIcon size={14} />
            </button>
          </div>
        </>
      )}
    </div>
  );
}
