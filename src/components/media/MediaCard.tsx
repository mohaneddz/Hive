import { Heart, Maximize2, VideoIcon } from "lucide-react";
import { Link } from "react-router-dom";

import { MediaThumb } from "@/components/media/MediaThumb";
import { setFavorite } from "@/lib/tauri";
import type { MediaItem } from "@/types/media";
import { cn } from "@/utils/cn";

export function MediaCard({
  item,
  onFavoriteChange,
}: {
  item: MediaItem;
  onFavoriteChange?: (mediaId: string, favorite: boolean) => void;
}) {
  const toggleFavorite = async (event: React.MouseEvent) => {
    event.preventDefault();
    const next = !item.isFavorite;
    await setFavorite(item.id, next);
    onFavoriteChange?.(item.id, next);
  };

  return (
    <article className="group relative">
      <Link to={`/media/${item.id}`} className="artwork-frame block aspect-square">
        <MediaThumb
          mediaId={item.id}
          alt={item.filename}
          className="relative size-full object-cover transition duration-700 group-hover:scale-[1.035]"
        />
        {item.mediaType === "video" && (
          <span className="absolute left-3 top-3 grid size-7 place-items-center rounded-full bg-black/55 text-white">
            <VideoIcon size={13} />
          </span>
        )}
        <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/55 to-transparent opacity-0 transition group-hover:opacity-100" />
        <div className="absolute inset-x-0 bottom-0 flex translate-y-2 items-end justify-between p-4 opacity-0 transition duration-300 group-hover:translate-y-0 group-hover:opacity-100">
          <p className="truncate text-xs font-bold text-white">{item.filename}</p>
          <span className="icon-button bg-white/15 text-white backdrop-blur-md">
            <Maximize2 size={15} />
          </span>
        </div>
        <button
          onClick={toggleFavorite}
          className="absolute right-3 top-3 grid size-9 place-items-center rounded-full bg-white/86 text-ink shadow-sm backdrop-blur transition hover:scale-105"
          aria-label={item.isFavorite ? "Remove favorite" : "Add favorite"}
        >
          <Heart size={16} className={cn(item.isFavorite && "fill-honey text-honey")} />
        </button>
      </Link>
    </article>
  );
}
