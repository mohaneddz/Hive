import type { ReactNode } from "react";

import { MediaCard } from "@/components/media/MediaCard";
import type { MediaItem } from "@/types/media";
import { cn } from "@/utils/cn";

const COLUMN_CLASSES: Record<number, string> = {
  3: "grid-cols-3",
  4: "grid-cols-4",
  5: "grid-cols-5",
  6: "grid-cols-6",
};

export function MediaGrid({
  items,
  columns = 4,
  onFavoriteChange,
  selected,
  onToggleSelect,
  renderOverlay,
  className,
}: {
  items: MediaItem[];
  columns?: 3 | 4 | 5 | 6;
  onFavoriteChange?: (mediaId: string, favorite: boolean) => void;
  selected?: Set<string>;
  onToggleSelect?: (mediaId: string) => void;
  renderOverlay?: (item: MediaItem) => ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("grid gap-4", COLUMN_CLASSES[columns], className)}>
      {items.map((item) => (
        <MediaCard
          key={item.id}
          item={item}
          onFavoriteChange={onFavoriteChange}
          selected={selected?.has(item.id)}
          onToggleSelect={onToggleSelect}
          overlayActions={renderOverlay?.(item)}
        />
      ))}
    </div>
  );
}
