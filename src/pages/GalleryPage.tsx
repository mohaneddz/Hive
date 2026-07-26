import { confirm, open } from "@tauri-apps/plugin-dialog";
import { Download, FolderPlus, ImagePlus, Layers, Loader2, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { TimelineView } from "@/components/collections/AutoGroups";
import { AddToAlbumDialog } from "@/components/media/AddToAlbumDialog";
import { GalleryToolbar, type GalleryFilters } from "@/components/media/GalleryToolbar";
import { MediaGrid } from "@/components/media/MediaGrid";
import { YearScrubber } from "@/components/media/YearScrubber";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import { exportMedia, getTimeline, setTrashed } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaSort, TimelineBucket, TimelineGranularity } from "@/types/media";
import { groupByDate } from "@/utils/dateGroups";
import { formatCount } from "@/utils/format";

const DATE_SORTS: MediaSort[] = ["taken", "oldest"];
const YEAR_SCRUB_ATTEMPTS = 25;

export function GalleryPage() {
  const { items, total, loading, jobs, folders, addFolder, cancelJob, loadPage, setItems } = useMediaLibrary();
  const [searchParams, setSearchParams] = useSearchParams();
  const folderId = searchParams.get("folder") ?? undefined;
  const activeFolder = useMemo(() => folders.find((f) => f.id === folderId), [folders, folderId]);

  const [view, setView] = useState<"grid" | "timeline">("grid");
  const [granularity, setGranularity] = useState<TimelineGranularity>("day");
  const [sort, setSort] = useState<MediaSort>("taken");
  const [filters, setFilters] = useState<GalleryFilters>({ mediaType: "all", favoritesOnly: false });
  const [columns, setColumns] = useState(4);
  const [autoLoad, setAutoLoad] = useState(false);

  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [addingToAlbum, setAddingToAlbum] = useState(false);

  const [yearBuckets, setYearBuckets] = useState<TimelineBucket[]>([]);
  const [pendingYear, setPendingYear] = useState<string | null>(null);

  // scrollToYear runs a loop across several loadPage calls; it reads this ref
  // instead of `items` so it sees each page landing, not the stale array closed
  // over when the loop started.
  const itemsRef = useRef(items);
  useEffect(() => {
    itemsRef.current = items;
  }, [items]);

  const requestOptions = useMemo(
    () => ({
      folderId,
      mediaType: filters.mediaType === "all" ? undefined : filters.mediaType,
      favoritesOnly: filters.favoritesOnly || undefined,
      sort,
    }),
    [folderId, filters, sort],
  );

  useEffect(() => {
    void loadPage(0, requestOptions);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folderId, filters.mediaType, filters.favoritesOnly, sort]);

  useEffect(() => {
    void getTimeline("year").then(setYearBuckets);
  }, []);

  const chooseFolder = async () => {
    try {
      const selectedPath = await open({ directory: true, multiple: false, title: "Add a media folder" });
      if (typeof selectedPath === "string") await addFolder(selectedPath);
    } catch {
      /* No native folder picker in the web preview. */
    }
  };

  const activeJob = jobs.find((job) => job.status === "running");
  const showDateHeaders = view === "grid" && DATE_SORTS.includes(sort);
  const groups = useMemo(
    () => (showDateHeaders ? groupByDate(items, granularity) : null),
    [showDateHeaders, items, granularity],
  );

  const loadMore = useCallback(() => {
    if (!loading && items.length < total) void loadPage(items.length, requestOptions);
  }, [loading, items.length, total, loadPage, requestOptions]);

  const sentinelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!autoLoad || view !== "grid") return;
    const node = sentinelRef.current;
    if (!node) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) loadMore();
      },
      { rootMargin: "600px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [autoLoad, view, loadMore]);

  const scrollToYear = async (year: string) => {
    if (pendingYear) return;
    if (view !== "grid") setView("grid");
    if (!DATE_SORTS.includes(sort)) return;

    const findAndScroll = () => {
      const target = document.querySelector(`[data-year="${year}"]`);
      if (!target) return false;
      target.scrollIntoView({ behavior: "smooth", block: "start" });
      return true;
    };
    if (findAndScroll()) return;

    setPendingYear(year);
    try {
      for (let attempt = 0; attempt < YEAR_SCRUB_ATTEMPTS; attempt++) {
        if (itemsRef.current.length >= total) break;
        await loadPage(itemsRef.current.length, requestOptions);
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        if (findAndScroll()) break;
      }
    } finally {
      setPendingYear(null);
    }
  };

  const toggleSelect = (mediaId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(mediaId)) next.delete(mediaId);
      else next.add(mediaId);
      return next;
    });
  };

  const applyFavorite = useCallback(
    (mediaId: string, favorite: boolean) => {
      setItems((prev) => prev.map((item) => (item.id === mediaId ? { ...item, isFavorite: favorite } : item)));
    },
    [setItems],
  );

  const exportSelected = async () => {
    if (selected.size === 0) return;
    const destination = await open({ directory: true, multiple: false, title: "Export selected items to…" });
    if (typeof destination !== "string") return;
    const report = await exportMedia([...selected], destination);
    await confirm(`${report.exported} exported, ${report.skipped} skipped.`, { title: "Export finished", kind: "info" });
  };

  const trashSelected = async () => {
    if (selected.size === 0) return;
    const confirmed = await confirm(`Move ${formatCount(selected.size, "item")} to trash?`, {
      title: "Move to trash",
      kind: "warning",
    });
    if (!confirmed) return;
    for (const mediaId of selected) {
      await setTrashed(mediaId, true);
    }
    setItems((prev) => prev.filter((item) => !selected.has(item.id)));
    setSelected(new Set());
    setSelecting(false);
  };

  const exitSelection = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  return (
    <div>
      <GalleryPageHeader
        eyebrow={activeFolder ? "Folder" : "Your library"}
        title={activeFolder ? activeFolder.name : "Every photo and video, indexed locally."}
        description={
          folders.length === 0
            ? "Add a folder to start building your local library."
            : `${total} item${total === 1 ? "" : "s"}${activeFolder ? "" : ` across ${folders.length} folder${folders.length === 1 ? "" : "s"}`}.`
        }
        action={
          selecting ? (
            <div className="flex items-center gap-2">
              <Button variant="secondary" icon={<Layers size={15} />} onClick={() => setAddingToAlbum(true)} disabled={selected.size === 0}>
                Add to album
              </Button>
              <Button variant="secondary" icon={<Download size={15} />} onClick={exportSelected} disabled={selected.size === 0}>
                Export
              </Button>
              <Button variant="secondary" icon={<Trash2 size={15} />} onClick={trashSelected} disabled={selected.size === 0}>
                Trash
              </Button>
              <Button variant="ghost" icon={<X size={15} />} onClick={exitSelection}>
                Done
              </Button>
            </div>
          ) : activeFolder ? (
            <div className="flex items-center gap-3">
              <Link
                to="/gallery"
                onClick={() => setSearchParams({})}
                className="inline-flex items-center gap-1.5 text-xs font-bold text-honey-deep hover:underline"
              >
                <X size={13} /> Clear filter
              </Link>
              {items.length > 0 && (
                <Button variant="secondary" onClick={() => setSelecting(true)}>
                  Select
                </Button>
              )}
            </div>
          ) : (
            <div className="flex items-center gap-2">
              {items.length > 0 && (
                <Button variant="secondary" onClick={() => setSelecting(true)}>
                  Select
                </Button>
              )}
              <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
                Add folder
              </Button>
            </div>
          )
        }
      />

      {selecting && (
        <p className="mt-5 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
          {selected.size === 0 ? "Tap items to select them." : `${formatCount(selected.size, "item")} selected.`}
        </p>
      )}

      {activeJob && (
        <div className="mt-5 flex items-center justify-between gap-3 rounded-2xl border border-honey/30 bg-honey/10 px-4 py-3 text-xs font-semibold text-honey-deep">
          <div className="flex items-center gap-3">
            <Loader2 size={15} className="animate-spin shrink-0" />
            <span>
              Indexing {activeJob.current}/{activeJob.total}
              {activeJob.message ? ` — ${activeJob.message}` : ""}
            </span>
          </div>
          <button
            onClick={() => cancelJob(activeJob.id)}
            className="flex items-center gap-1.5 rounded-lg bg-honey/20 px-3 py-1.5 text-xs font-bold text-honey-deep transition hover:bg-honey/30"
          >
            <X size={14} />
            Cancel
          </button>
        </div>
      )}

      {items.length === 0 && !loading && (
        <EmptyState
          icon={<ImagePlus size={22} />}
          title="No media yet"
          description="Add a folder full of photos or videos to build your library."
          action={
            <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
              Add folder
            </Button>
          }
        />
      )}

      {items.length > 0 && (
        <>
          <GalleryToolbar
            view={view}
            onViewChange={setView}
            granularity={granularity}
            onGranularityChange={setGranularity}
            sort={sort}
            onSortChange={setSort}
            filters={filters}
            onFiltersChange={setFilters}
            columns={columns}
            onColumnsChange={setColumns}
            autoLoad={autoLoad}
            onAutoLoadChange={setAutoLoad}
          />

          {view === "timeline" ? (
            <div className="mt-2">
              <TimelineView />
            </div>
          ) : (
            <div className="mt-2 flex items-start gap-3">
              <div className="min-w-0 flex-1">
                {groups
                  ? groups.map((group) => (
                      <section key={group.key} className="mt-6" data-year={group.year ?? undefined}>
                        <h2 className="mb-3 text-sm font-extrabold text-ink">{group.label}</h2>
                        <MediaGrid
                          items={group.items}
                          columns={columns as 3 | 4 | 5 | 6 | 7 | 8}
                          onFavoriteChange={applyFavorite}
                          selected={selecting ? selected : undefined}
                          onToggleSelect={selecting ? toggleSelect : undefined}
                        />
                      </section>
                    ))
                  : (
                      <MediaGrid
                        items={items}
                        className="mt-6"
                        columns={columns as 3 | 4 | 5 | 6 | 7 | 8}
                        onFavoriteChange={applyFavorite}
                        selected={selecting ? selected : undefined}
                        onToggleSelect={selecting ? toggleSelect : undefined}
                      />
                    )}

                <div ref={sentinelRef} />

                {items.length < total && (
                  <div className="mt-8 flex justify-center">
                    <Button variant="secondary" disabled={loading} onClick={loadMore}>
                      {loading ? "Loading…" : "Load more"}
                    </Button>
                  </div>
                )}
              </div>

              {DATE_SORTS.includes(sort) && (
                <YearScrubber buckets={yearBuckets} pending={pendingYear} onSelect={scrollToYear} />
              )}
            </div>
          )}
        </>
      )}

      {addingToAlbum && (
        <AddToAlbumDialog
          mediaIds={[...selected]}
          onClose={() => setAddingToAlbum(false)}
        />
      )}
    </div>
  );
}
