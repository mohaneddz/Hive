import { useRef, useEffect } from "react";
import type { MediaItem } from "@/types/media";
import { MediaThumb } from "@/components/media/MediaThumb";
import { cn } from "@/utils/cn";

interface MediaViewerFilmstripProps {
  items: MediaItem[];
  currentId: string;
  onSelect: (item: MediaItem) => void;
}

export function MediaViewerFilmstrip({
  items,
  currentId,
  onSelect,
}: MediaViewerFilmstripProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const activeRef = useRef<HTMLButtonElement>(null);

  // Scroll active item into view on change
  useEffect(() => {
    if (activeRef.current && scrollRef.current) {
      activeRef.current.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
        inline: "center",
      });
    }
  }, [currentId]);

  if (items.length <= 1) return null;

  return (
    <div className="w-full max-w-4xl px-4 py-2 select-none">
      <div
        ref={scrollRef}
        className="flex items-center gap-2.5 overflow-x-auto py-2 px-3 no-scrollbar rounded-2xl border border-white/20 dark:border-white/10 bg-panel/75 dark:bg-panel/75 backdrop-blur-xl shadow-lg"
      >
        {items.map((entry) => {
          const isActive = entry.id === currentId;
          return (
            <button
              key={entry.id}
              ref={isActive ? activeRef : null}
              onClick={() => onSelect(entry)}
              className={cn(
                "relative shrink-0 rounded-xl overflow-hidden transition-all duration-150 group",
                isActive
                  ? "ring-2 ring-honey ring-offset-2 ring-offset-panel scale-105 opacity-100 z-10"
                  : "opacity-60 hover:opacity-100 hover:scale-102"
              )}
            >
              <MediaThumb
                mediaId={entry.id}
                variant="sm"
                alt={entry.filename}
                className="size-14 object-cover rounded-xl"
              />
              <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-end p-1">
                <span className="text-[9px] font-medium text-white truncate w-full leading-tight">
                  {entry.filename}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
