import { useEffect, useState, type ReactNode } from "react";
import { Check, Eye, Heart, Maximize2, ShieldAlert, VideoIcon } from "lucide-react";
import { Link } from "react-router-dom";

import { MediaThumb } from "@/components/media/MediaThumb";
import { useNsfwPolicy } from "@/hooks/useNsfwPolicy";
import { setFavorite } from "@/lib/tauri";
import type { MediaItem } from "@/types/media";
import { cn } from "@/utils/cn";

export function MediaCard({
  item,
  onFavoriteChange,
  selected,
  onToggleSelect,
  overlayActions,
}: {
  item: MediaItem;
  onFavoriteChange?: (mediaId: string, favorite: boolean) => void;
  /** Present only in selection mode; the card then selects instead of navigating. */
  selected?: boolean;
  onToggleSelect?: (mediaId: string) => void;
  /** Extra buttons rendered on hover, e.g. "use as album cover". */
  overlayActions?: ReactNode;
}) {
  const selectable = typeof onToggleSelect === "function";

  // Covering is per-card and resets on navigation: revealing one photo should
  // never quietly reveal the rest of the grid. The threshold comes from the
  // backend so it cannot drift from the one the scoring job applies.
  const { threshold } = useNsfwPolicy();
  const [revealed, setRevealed] = useState(false);
  const isSensitive = (item.nsfwScore ?? 0) >= threshold && !revealed;

  // The heart fills on click, not on the round trip to SQLite. Cleared as soon
  // as the real value arrives, so the prop stays the source of truth.
  const [optimistic, setOptimistic] = useState<boolean | null>(null);
  useEffect(() => setOptimistic(null), [item.isFavorite]);
  const isFavorite = optimistic ?? item.isFavorite;

  const toggleFavorite = async (event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();

    const next = !isFavorite;
    setOptimistic(next);
    onFavoriteChange?.(item.id, next);

    try {
      await setFavorite(item.id, next);
    } catch {
      setOptimistic(!next);
      onFavoriteChange?.(item.id, !next);
    }
  };

  const frame = (
    <>
      <MediaThumb
        mediaId={item.id}
        alt={item.filename}
        className={cn(
          "relative size-full object-cover transition duration-700 group-hover:scale-[1.035]",
          // Blurred rather than hidden: you can still tell your photos apart.
          isSensitive && "scale-110 blur-2xl",
        )}
      />

      {isSensitive && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-2.5 bg-black/45 p-4 text-center">
          <ShieldAlert size={22} className="text-white/90" />
          <p className="text-[11px] font-extrabold text-white">Possibly sensitive</p>
          <button
            onClick={(event) => {
              // Inside a Link, so the click must not also open the viewer.
              event.preventDefault();
              event.stopPropagation();
              setRevealed(true);
            }}
            className="inline-flex items-center gap-1.5 rounded-lg bg-white/90 px-3 py-1.5 text-[11px] font-bold text-ink transition hover:bg-white"
          >
            <Eye size={12} />
            Show
          </button>
        </div>
      )}
      {item.mediaType === "video" && (
        <span className="absolute left-3 top-3 grid size-7 place-items-center rounded-full bg-black/55 text-white">
          <VideoIcon size={13} />
        </span>
      )}
      <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/55 to-transparent opacity-0 transition group-hover:opacity-100" />
      <div className="absolute inset-x-0 bottom-0 flex translate-y-2 items-end justify-between gap-2 p-4 opacity-0 transition duration-300 group-hover:translate-y-0 group-hover:opacity-100">
        <p className="min-w-0 truncate text-xs font-bold text-white">{item.filename}</p>
        <div className="flex shrink-0 items-center gap-1.5">
          {overlayActions}
          {!selectable && (
            <span className="icon-button bg-white/15 text-white backdrop-blur-md">
              <Maximize2 size={15} />
            </span>
          )}
        </div>
      </div>

      {selectable ? (
        <span
          className={cn(
            "absolute right-3 top-3 grid size-8 place-items-center rounded-full border-2 transition",
            selected
              ? "border-honey bg-honey text-[#3b2900]"
              : "border-white/80 bg-black/25 text-transparent",
          )}
        >
          <Check size={15} strokeWidth={3} />
        </span>
      ) : (
        <button
          onClick={toggleFavorite}
          className="absolute right-3 top-3 grid size-9 place-items-center rounded-full bg-white/86 text-ink shadow-sm backdrop-blur transition hover:scale-105"
          aria-label={isFavorite ? "Remove favorite" : "Add favorite"}
        >
          <Heart size={16} className={cn(isFavorite && "fill-honey text-honey")} />
        </button>
      )}
    </>
  );

  return (
    <article className="group relative">
      {selectable ? (
        <button
          type="button"
          onClick={() => onToggleSelect?.(item.id)}
          aria-pressed={selected}
          className={cn(
            "artwork-frame block aspect-square w-full text-left transition",
            selected && "!border-honey",
          )}
        >
          {frame}
        </button>
      ) : (
        <Link to={`/media/${item.id}`} className="artwork-frame block aspect-square">
          {frame}
        </Link>
      )}
    </article>
  );
}
