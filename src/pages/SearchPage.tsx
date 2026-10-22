import { Lock, Search, Sparkles, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { GalleryChatPanel } from "@/components/chat/GalleryChatPanel";
import { MediaCard } from "@/components/media/MediaCard";
import { SavedSearchesPanel, type SavedSearch } from "@/components/search/SavedSearches";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useClickOutside } from "@/hooks/useClickOutside";
import { isTauri, listPeople, listSearchCameras, searchLibrary, type SearchLibraryRequest } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem, MediaType, PersonSummary } from "@/types/media";
import { cn } from "@/utils/cn";

const TYPE_FILTERS: { label: string; value: MediaType | "all" }[] = [
  { label: "All", value: "all" },
  { label: "Photos", value: "image" },
  { label: "Videos", value: "video" },
];

type Orientation = "all" | "landscape" | "portrait" | "square";
type SortOption = "relevance" | "newest" | "oldest" | "name";

const SORT_OPTIONS: { value: SortOption; label: string }[] = [
  { value: "relevance", label: "Best match" },
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "name", label: "Name" },
];

export function SearchPage() {
  const [searchParams] = useSearchParams();
  const [query, setQuery] = useState(() => searchParams.get("q") ?? "");
  const [mediaType, setMediaType] = useState<MediaType | "all">("all");
  const [favoritesOnly, setFavoritesOnly] = useState(() => searchParams.get("favorites") === "1");
  const [mode, setMode] = useState<"exact" | "semantic">("exact");
  const [items, setItems] = useState<MediaItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [debouncedQuery, setDebouncedQuery] = useState(query);
  const { status: aiStatus } = useAiStatus();

  const [dateOpen, setDateOpen] = useState(false);
  const dateRef = useClickOutside(() => setDateOpen(false));
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [camera, setCamera] = useState("all");
  const [personId, setPersonId] = useState("all");
  const [orientation, setOrientation] = useState<Orientation>("all");
  const [sortBy, setSortBy] = useState<SortOption>("relevance");

  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [cameraOptions, setCameraOptions] = useState<string[]>([]);
  const searchGeneration = useRef(0);

  const requestFor = (offset: number): SearchLibraryRequest => ({
    query: debouncedQuery.trim(),
    semantic: mode === "semantic" && Boolean(aiStatus?.modelsReady),
    mediaType: mediaType === "all" ? null : mediaType,
    favoritesOnly,
    dateFrom: dateFrom || null,
    dateTo: dateTo || null,
    camera: camera === "all" ? null : camera,
    personId: personId === "all" ? null : personId,
    orientation: orientation === "all" ? null : orientation,
    sort: sortBy,
    limit: 100,
    offset,
  });

  useEffect(() => {
    const fromUrl = searchParams.get("q");
    if (fromUrl && fromUrl !== query) setQuery(fromUrl);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  useEffect(() => {
    const timeout = window.setTimeout(() => setDebouncedQuery(query), 300);
    return () => window.clearTimeout(timeout);
  }, [query]);

  useEffect(() => {
    let cancelled = false;
    searchGeneration.current += 1;
    if (!isTauri()) {
      setItems([]);
      setTotal(0);
      setLoading(false);
      setFailure(null);
      return;
    }
    setLoading(true);
    setFailure(null);
    searchLibrary(requestFor(0))
      .then((page) => {
        if (!cancelled) {
          setItems(page.items);
          setTotal(page.total);
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setItems([]);
          setTotal(0);
          setFailure(String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [debouncedQuery, mediaType, favoritesOnly, mode, aiStatus?.modelsReady, dateFrom, dateTo, camera, personId, orientation, sortBy]);

  useEffect(() => {
    if (isTauri() && aiStatus?.peopleCount) void listPeople().then(setPeople);
  }, [aiStatus?.peopleCount]);

  useEffect(() => {
    if (isTauri()) void listSearchCameras().then(setCameraOptions).catch(() => {});
  }, []);

  const loadMore = async () => {
    if (loadingMore || items.length >= total) return;
    const generation = searchGeneration.current;
    setLoadingMore(true);
    try {
      const page = await searchLibrary(requestFor(items.length));
      if (searchGeneration.current === generation) {
        setItems((current) => [...current, ...page.items]);
        setTotal(page.total);
      }
    } catch (cause) { if (searchGeneration.current === generation) setFailure(String(cause)); }
    finally { setLoadingMore(false); }
  };

  const activeDateRange = dateFrom || dateTo;
  const hasExtraFilters = activeDateRange || camera !== "all" || personId !== "all" || orientation !== "all";

  const clearExtraFilters = () => {
    setDateFrom("");
    setDateTo("");
    setCamera("all");
    setPersonId("all");
    setOrientation("all");
  };

  const applySavedSearch = (search: SavedSearch) => {
    setMode(search.mode);
    setQuery(search.query);
  };

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
      <div className="min-w-0">
      <GalleryPageHeader
        eyebrow="Search"
        title="Find anything in your library."
        description={
          mode === "semantic"
            ? "Semantic search — describe what you're looking for, not just the filename."
            : "Full-text search across filenames and camera metadata."
        }
        action={
          <span className="inline-flex items-center gap-1.5 text-[11px] font-bold text-ink-muted">
            <Lock size={11} /> Processed locally
          </span>
        }
      />

      <div className="mt-7 flex flex-wrap items-center gap-3">
        <label className="relative min-w-[240px] flex-1">
          <Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-muted" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            className="search-input"
            placeholder={mode === "semantic" ? "Describe what you're looking for…" : "Search filename or camera"}
          />
        </label>

        <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
          <button
            onClick={() => setMode("semantic")}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
              mode === "semantic" && "bg-honey/15 text-honey-deep",
            )}
          >
            <Sparkles size={12} /> Semantic
          </button>
          <button
            onClick={() => setMode("exact")}
            className={cn(
              "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
              mode === "exact" && "bg-honey/15 text-honey-deep",
            )}
          >
            Exact
          </button>
        </div>

        <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
          {TYPE_FILTERS.map((filter) => (
            <button
              key={filter.value}
              onClick={() => setMediaType(filter.value)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
                mediaType === filter.value && "bg-honey/15 text-honey-deep",
              )}
            >
              {filter.label}
            </button>
          ))}
        </div>

        <button
          onClick={() => setFavoritesOnly((v) => !v)}
          className={cn(
            "inline-flex h-[42px] items-center gap-2 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink",
            favoritesOnly && "border-honey/50 bg-honey/12 text-honey-deep",
          )}
        >
          Favorites only
        </button>

        {(query || mediaType !== "all" || favoritesOnly) && (
          <button
            onClick={() => {
              setQuery("");
              setMediaType("all");
              setFavoritesOnly(false);
            }}
            className="icon-button"
            aria-label="Clear filters"
          >
            <X size={15} />
          </button>
        )}
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-3">
        <div ref={dateRef} className="relative">
          <button
            onClick={() => setDateOpen((v) => !v)}
            className={cn(
              "inline-flex h-9 items-center gap-1.5 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink-muted",
              activeDateRange && "border-honey/50 bg-honey/12 text-honey-deep",
            )}
          >
            Date {dateFrom || dateTo ? `${dateFrom || "…"} → ${dateTo || "…"}` : ""}
          </button>
          {dateOpen && (
            <div className="absolute left-0 top-[calc(100%+6px)] z-20 flex items-center gap-2 rounded-2xl border border-ink/[.1] bg-panel p-3 shadow-xl">
              <input
                type="date"
                value={dateFrom}
                onChange={(event) => setDateFrom(event.target.value)}
                className="h-8 rounded-lg border border-ink/[.12] bg-canvas px-2 text-xs text-ink outline-none"
              />
              <span className="text-xs text-ink-muted">to</span>
              <input
                type="date"
                value={dateTo}
                onChange={(event) => setDateTo(event.target.value)}
                className="h-8 rounded-lg border border-ink/[.12] bg-canvas px-2 text-xs text-ink outline-none"
              />
            </div>
          )}
        </div>

        {people.length > 0 && (
          <select
            value={personId}
            onChange={(event) => setPersonId(event.target.value)}
            className="h-9 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink-muted outline-none"
          >
            <option value="all">People: anyone</option>
            {people.map((person) => (
              <option key={person.id} value={person.id}>
                {person.name ?? "Unnamed"}
              </option>
            ))}
          </select>
        )}

        {cameraOptions.length > 0 && (
          <select
            value={camera}
            onChange={(event) => setCamera(event.target.value)}
            className="h-9 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink-muted outline-none"
          >
            <option value="all">Camera: any</option>
            {cameraOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        )}

        <select
          value={orientation}
          onChange={(event) => setOrientation(event.target.value as Orientation)}
          className="h-9 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink-muted outline-none"
        >
          <option value="all">Orientation: any</option>
          <option value="landscape">Landscape</option>
          <option value="portrait">Portrait</option>
          <option value="square">Square</option>
        </select>

        {hasExtraFilters && (
          <button onClick={clearExtraFilters} className="text-xs font-bold text-honey-deep hover:underline">
            Clear filters
          </button>
        )}
      </div>

      {mode === "semantic" && aiStatus && !aiStatus.modelsReady && (
        <div className="mt-5 flex items-center justify-between gap-3 rounded-2xl border border-dashed border-ink/[.15] p-4 text-xs text-ink-muted">
          <span>Semantic search needs the local AI model, which isn't downloaded yet.</span>
          <Link to="/settings" className="font-bold text-honey-deep hover:underline">
            Enable in Settings
          </Link>
        </div>
      )}

      {failure ? (
        <div className="mt-6 rounded-2xl border border-red-500/25 bg-red-500/5 p-4 text-sm text-red-600">
          <p className="font-extrabold">Search failed</p>
          <p className="mt-1 break-words text-xs opacity-90">{failure}</p>
        </div>
      ) : loading ? (
        <div className="mt-12 text-center text-sm text-ink-muted">Searching…</div>
      ) : (
        <>
          <div className="mt-6 flex items-center justify-between">
            <p className="text-xs font-bold text-ink-muted">{total} results</p>
            <select
              value={sortBy}
              onChange={(event) => setSortBy(event.target.value as SortOption)}
              className="h-8 rounded-lg border border-ink/[.12] bg-panel px-2.5 text-[11px] font-bold text-ink outline-none"
              aria-label="Sort by"
            >
              {SORT_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  Sort by: {option.label}
                </option>
              ))}
            </select>
          </div>

          <div className="mt-3 grid grid-cols-2 gap-4 lg:grid-cols-3 2xl:grid-cols-4">
            {items.map((item) => (
              <MediaCard key={item.id} item={item} />
            ))}
          </div>
          {items.length === 0 && (
            <div className="mt-12 rounded-3xl border border-dashed border-ink/[.15] p-12 text-center text-sm text-ink-muted">
              No results match those filters.
            </div>
          )}
          {items.length < total && (
            <button onClick={() => void loadMore()} disabled={loadingMore} className="mt-6 rounded-xl border border-ink/[.12] bg-panel px-4 py-2 text-sm font-semibold text-ink">
              {loadingMore ? "Loading…" : `Show more (${items.length} of ${total})`}
            </button>
          )}
        </>
      )}
      </div>

      <div className="flex min-h-[420px] flex-col gap-4 xl:sticky xl:top-0 xl:h-[calc(100vh-120px)]">
        <SavedSearchesPanel
          current={query.trim() ? { query: query.trim(), mode } : null}
          onApply={applySavedSearch}
        />
        <div className="min-h-0 flex-1">
          <GalleryChatPanel />
        </div>
      </div>
    </div>
  );
}
