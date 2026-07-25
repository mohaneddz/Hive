import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, CalendarRange, Loader2, MapPin, Plane } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaGrid } from "@/components/media/MediaGrid";
import { MediaThumb } from "@/components/media/MediaThumb";
import {
  detectEvents,
  detectTrips,
  getMediaPage,
  getTimeline,
  isTauri,
  listMediaInBucket,
} from "@/lib/tauri";
import type { EventGroup, MediaItem, TimelineBucket, TimelineGranularity } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCount, formatDate } from "@/utils/format";

const GRANULARITIES: { value: TimelineGranularity; label: string }[] = [
  { value: "year", label: "Year" },
  { value: "month", label: "Month" },
  { value: "day", label: "Day" },
];

/** "12 – 18 June 2024", or a single date when the run fits in one day. */
function spanLabel(start: string, end: string): string {
  const from = formatDate(start);
  const to = formatDate(end);
  return from === to ? from : `${from} → ${to}`;
}

/* ------------------------------------------------------------- timeline -- */

export function TimelineView() {
  const [granularity, setGranularity] = useState<TimelineGranularity>("year");
  const [buckets, setBuckets] = useState<TimelineBucket[] | null>(null);
  const [open, setOpen] = useState<TimelineBucket | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);

  useEffect(() => {
    if (!isTauri()) return;
    setBuckets(null);
    setOpen(null);
    void getTimeline(granularity).then(setBuckets);
  }, [granularity]);

  const openBucket = async (bucket: TimelineBucket) => {
    setOpen(bucket);
    setItems(await listMediaInBucket(granularity, bucket.key));
  };

  if (open) {
    return (
      <div>
        <button
          onClick={() => setOpen(null)}
          className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <ArrowLeft size={14} /> Back to timeline
        </button>
        <h2 className="text-lg font-extrabold text-ink">{open.label}</h2>
        <p className="text-xs text-ink-muted">{formatCount(open.count, "photo")}</p>
        <MediaGrid items={items} className="mt-6" />
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
        {GRANULARITIES.map((entry) => (
          <button
            key={entry.value}
            onClick={() => setGranularity(entry.value)}
            className={cn(
              "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
              granularity === entry.value && "bg-cream text-honey-deep",
            )}
          >
            {entry.label}
          </button>
        ))}
      </div>

      {buckets === null && <div className="mt-10 text-center text-sm text-ink-muted">Loading…</div>}

      {buckets?.length === 0 && (
        <EmptyState
          icon={<CalendarRange size={22} />}
          title="Nothing to place on a timeline yet"
          description="Photos need a capture date. Add a folder, or correct a date from the editor's Details tab."
        />
      )}

      {buckets && buckets.length > 0 && (
        <div className="mt-6 grid grid-cols-4 gap-4">
          {buckets.map((bucket) => (
            <button
              key={bucket.key}
              onClick={() => openBucket(bucket)}
              className="group overflow-hidden rounded-[22px] border border-ink/[.07] bg-panel text-left shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
            >
              <div className="relative aspect-[4/3] bg-shell">
                {bucket.coverMediaId ? (
                  <MediaThumb
                    mediaId={bucket.coverMediaId}
                    alt={bucket.label}
                    className="size-full object-cover transition duration-500 group-hover:scale-[1.04]"
                  />
                ) : (
                  <div className="grid size-full place-items-center text-ink-muted">
                    <CalendarRange size={24} />
                  </div>
                )}
                <span className="absolute left-3 top-3 rounded-full bg-black/55 px-2.5 py-1 text-[11px] font-extrabold text-white">
                  {bucket.label}
                </span>
              </div>
              <div className="p-4">
                <p className="text-xs font-extrabold text-ink">{formatCount(bucket.count, "photo")}</p>
                <p className="mt-0.5 truncate text-[11px] text-ink-muted">
                  {spanLabel(bucket.start, bucket.end)}
                </p>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/* ------------------------------------------------------ events & trips -- */

export function EventsView({ trips = false }: { trips?: boolean }) {
  const [groups, setGroups] = useState<EventGroup[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState<EventGroup | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);

  const run = useCallback(async () => {
    if (!isTauri()) return;
    setBusy(true);
    setOpen(null);
    try {
      setGroups(trips ? await detectTrips() : await detectEvents());
    } finally {
      setBusy(false);
    }
  }, [trips]);

  useEffect(() => {
    void run();
  }, [run]);

  const openGroup = async (group: EventGroup) => {
    setOpen(group);
    // The ids are already known; fetching one page and filtering keeps this to a
    // single round trip.
    const page = await getMediaPage({ limit: 10_000, offset: 0 });
    const wanted = new Set(group.mediaIds);
    setItems(page.items.filter((item) => wanted.has(item.id)));
  };

  if (open) {
    return (
      <div>
        <button
          onClick={() => setOpen(null)}
          className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <ArrowLeft size={14} /> Back to {trips ? "trips" : "events"}
        </button>
        <h2 className="text-lg font-extrabold text-ink">{spanLabel(open.start, open.end)}</h2>
        <p className="text-xs text-ink-muted">
          {formatCount(open.count, "photo")}
          {open.isTrip && ` · about ${Math.round(open.distanceKm)} km from home`}
        </p>
        <MediaGrid items={items} className="mt-6" />
      </div>
    );
  }

  return (
    <div>
      <Card className="flex items-start justify-between gap-4 p-6">
        <div>
          <h2 className="text-base font-extrabold text-ink">
            {trips ? "Detected trips" : "Detected events"}
          </h2>
          <p className="mt-0.5 max-w-2xl text-xs text-ink-muted">
            {trips
              ? "A trip is a run of photos lasting at least a night whose centre of gravity sits far from where most of your photos are taken. Distance is measured from your coordinates — no model, no lookup, nothing sent anywhere."
              : "An event is a burst of photos with a quiet gap on either side. Hive splits your library wherever more than six hours pass with nothing taken."}
          </p>
        </div>
        <Button
          variant="secondary"
          icon={busy ? <Loader2 size={15} className="animate-spin" /> : undefined}
          onClick={run}
          disabled={busy}
        >
          Recompute
        </Button>
      </Card>

      {groups === null && <div className="mt-10 text-center text-sm text-ink-muted">Loading…</div>}

      {groups?.length === 0 && (
        <EmptyState
          icon={trips ? <Plane size={22} /> : <CalendarRange size={22} />}
          title={trips ? "No trips found" : "No events found"}
          description={
            trips
              ? "Trips need geotagged photos taken away from home over at least a night. Photos without GPS can not be placed."
              : "Events need at least four photos taken close together in time."
          }
        />
      )}

      {groups && groups.length > 0 && (
        <div className="mt-5 grid grid-cols-4 gap-4">
          {groups.map((group) => (
            <button
              key={group.id}
              onClick={() => openGroup(group)}
              className="group overflow-hidden rounded-[22px] border border-ink/[.07] bg-panel text-left shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
            >
              <div className="relative aspect-[4/3] bg-shell">
                <MediaThumb
                  mediaId={group.coverMediaId}
                  alt={spanLabel(group.start, group.end)}
                  className="size-full object-cover transition duration-500 group-hover:scale-[1.04]"
                />
                <span className="absolute left-3 top-3 rounded-full bg-black/55 px-2.5 py-1 text-[11px] font-extrabold text-white">
                  {formatCount(group.count, "photo")}
                </span>
                {group.isTrip && (
                  <span className="absolute right-3 top-3 inline-flex items-center gap-1 rounded-full bg-honey/90 px-2.5 py-1 text-[10px] font-extrabold text-[#3b2900]">
                    <MapPin size={10} />
                    {Math.round(group.distanceKm)} km
                  </span>
                )}
              </div>
              <div className="p-4">
                <p className="truncate text-xs font-extrabold text-ink">
                  {spanLabel(group.start, group.end)}
                </p>
                <p className="mt-0.5 truncate text-[11px] text-ink-muted">
                  {group.lat != null && group.lon != null
                    ? `${group.lat.toFixed(2)}, ${group.lon.toFixed(2)}`
                    : "No location"}
                </p>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
