import { Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { MediaCard } from "@/components/media/MediaCard";
import { getMediaPage, searchMedia } from "@/lib/tauri";
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
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const fromUrl = searchParams.get("q");
    if (fromUrl && fromUrl !== query) setQuery(fromUrl);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const trimmed = query.trim();

    const request = trimmed
      ? searchMedia(trimmed)
      : getMediaPage({
          limit: 500,
          offset: 0,
          mediaType: mediaType === "all" ? undefined : mediaType,
          favoritesOnly: favoritesOnly || undefined,
        }).then((page) => page.items);

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
  }, [query, mediaType, favoritesOnly]);

  const visible = useMemo(() => {
    if (!query.trim()) return items;
    return items
      .filter((item) => mediaType === "all" || item.mediaType === mediaType)
      .filter((item) => !favoritesOnly || item.isFavorite);
  }, [items, query, mediaType, favoritesOnly]);

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Search"
        title="Find anything in your library."
        description="Full-text search across filenames and camera metadata. Semantic search lands in a later phase."
      />

      <div className="mt-7 flex flex-wrap items-center gap-3">
        <label className="relative min-w-[240px] flex-1">
          <Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-muted" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            className="search-input"
            placeholder="Search filename or camera"
          />
        </label>

        <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
          {TYPE_FILTERS.map((filter) => (
            <button
              key={filter.value}
              onClick={() => setMediaType(filter.value)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
                mediaType === filter.value && "bg-cream text-honey-deep",
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
            favoritesOnly && "border-honey/50 bg-cream/55 text-honey-deep",
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
  );
}
