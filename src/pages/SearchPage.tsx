import { Search, Sparkles, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { GalleryChatPanel } from "@/components/chat/GalleryChatPanel";
import { MediaCard } from "@/components/media/MediaCard";
import { useAiStatus } from "@/hooks/useAiStatus";
import { getMediaPage, searchMedia, semanticSearch } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem, MediaType } from "@/types/media";
import { cn } from "@/utils/cn";

const TYPE_FILTERS: { label: string; value: MediaType | "all" }[] = [
  { label: "All", value: "all" },
  { label: "Photos", value: "image" },
  { label: "Videos", value: "video" },
];

export function SearchPage() {
  const [searchParams] = useSearchParams();
  const [query, setQuery] = useState(() => searchParams.get("q") ?? "");
  const [mediaType, setMediaType] = useState<MediaType | "all">("all");
  const [favoritesOnly, setFavoritesOnly] = useState(() => searchParams.get("favorites") === "1");
  const [mode, setMode] = useState<"exact" | "semantic">("exact");
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);
  const { status: aiStatus } = useAiStatus();

  useEffect(() => {
    const fromUrl = searchParams.get("q");
    if (fromUrl && fromUrl !== query) setQuery(fromUrl);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const trimmed = query.trim();

    const request = !trimmed
      ? getMediaPage({
          limit: 500,
          offset: 0,
          mediaType: mediaType === "all" ? undefined : mediaType,
          favoritesOnly: favoritesOnly || undefined,
        }).then((page) => page.items)
      : mode === "semantic" && aiStatus?.modelsReady
        ? semanticSearch(trimmed)
        : searchMedia(trimmed);

    request
      .then((results) => {
        if (!cancelled) setItems(results);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [query, mediaType, favoritesOnly, mode, aiStatus?.modelsReady]);

  const visible = useMemo(() => {
    if (!query.trim()) return items;
    return items
      .filter((item) => mediaType === "all" || item.mediaType === mediaType)
      .filter((item) => !favoritesOnly || item.isFavorite);
  }, [items, query, mediaType, favoritesOnly]);

  return (
    <div className="grid grid-cols-[1fr_320px] gap-6">
      <div>
      <GalleryPageHeader
        eyebrow="Search"
        title="Find anything in your library."
        description={
          mode === "semantic"
            ? "Semantic search — describe what you're looking for, not just the filename."
            : "Full-text search across filenames and camera metadata."
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

      {mode === "semantic" && aiStatus && !aiStatus.modelsReady && (
        <div className="mt-5 flex items-center justify-between gap-3 rounded-2xl border border-dashed border-ink/[.15] p-4 text-xs text-ink-muted">
          <span>Semantic search needs the local AI model, which isn't downloaded yet.</span>
          <Link to="/settings" className="font-bold text-honey-deep hover:underline">
            Enable in Settings
          </Link>
        </div>
      )}

      {loading ? (
        <div className="mt-12 text-center text-sm text-ink-muted">Searching…</div>
      ) : (
        <>
          <div className="mt-7 grid grid-cols-4 gap-4">
            {visible.map((item) => (
              <MediaCard key={item.id} item={item} />
            ))}
          </div>
          {visible.length === 0 && (
            <div className="mt-12 rounded-3xl border border-dashed border-ink/[.15] p-12 text-center text-sm text-ink-muted">
              No results match those filters.
            </div>
          )}
        </>
      )}
      </div>

      <div className="sticky top-0 h-[calc(100vh-120px)]">
        <GalleryChatPanel />
      </div>
    </div>
  );
}
