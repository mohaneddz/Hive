import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ChevronLeft, ChevronRight, Heart, Trash2, X } from "lucide-react";

import { MediaThumb } from "@/components/media/MediaThumb";
import { getMediaDetail, getMediaPage, setFavorite, setTrashed } from "@/lib/tauri";
import type { MediaItem } from "@/types/media";
import { cn } from "@/utils/cn";

const NAV_WINDOW = 500;

export function ViewerPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [neighbors, setNeighbors] = useState<MediaItem[]>([]);
  const [item, setItem] = useState<MediaItem | null>(null);

  useEffect(() => {
    let cancelled = false;
    getMediaPage({ limit: NAV_WINDOW, offset: 0 }).then((page) => {
      if (!cancelled) setNeighbors(page.items);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    const fromWindow = neighbors.find((entry) => entry.id === id);
    if (fromWindow) {
      setItem(fromWindow);
      return;
    }
    getMediaDetail(id).then((detail) => {
      if (!cancelled) setItem(detail);
    });
    return () => {
      cancelled = true;
    };
  }, [id, neighbors]);

  const index = neighbors.findIndex((entry) => entry.id === id);
  const prevItem = index > 0 ? neighbors[index - 1] : null;
  const nextItem = index >= 0 && index < neighbors.length - 1 ? neighbors[index + 1] : null;

  const goTo = useCallback(
    (target: MediaItem | null) => {
      if (target) navigate(`/media/${target.id}`);
    },
    [navigate],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") goTo(prevItem);
      if (event.key === "ArrowRight") goTo(nextItem);
      if (event.key === "Escape") navigate("/");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goTo, prevItem, nextItem, navigate]);

  if (!item) {
    return <div className="grid h-full place-items-center text-sm text-ink-muted">Loading…</div>;
  }

  const toggleFavorite = async () => {
    const next = !item.isFavorite;
    await setFavorite(item.id, next);
    setItem({ ...item, isFavorite: next });
  };

  const trash = async () => {
    await setTrashed(item.id, true);
    navigate("/");
  };

  return (
    <div className="-mx-8 -my-7 flex h-[calc(100vh-40px)] flex-col bg-black/90">
      <div className="flex items-center justify-between px-6 py-4 text-white">
        <button onClick={() => navigate("/")} className="icon-button bg-white/10 text-white" aria-label="Close">
          <X size={16} />
        </button>
        <div className="text-center">
          <p className="text-sm font-bold">{item.filename}</p>
          <p className="text-[11px] text-white/60">
            {item.takenAt ?? item.createdAt} {item.width && item.height ? `· ${item.width}×${item.height}` : ""}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={toggleFavorite}
            className="icon-button bg-white/10 text-white"
            aria-label="Toggle favorite"
          >
            <Heart size={16} className={cn(item.isFavorite && "fill-honey text-honey")} />
          </button>
          <button onClick={trash} className="icon-button bg-white/10 text-white" aria-label="Move to trash">
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      <div className="relative flex flex-1 items-center justify-center px-6 pb-6">
        {prevItem && (
          <button
            onClick={() => goTo(prevItem)}
            className="absolute left-4 top-1/2 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white transition hover:bg-white/20"
            aria-label="Previous"
          >
            <ChevronLeft size={22} />
          </button>
        )}
        <MediaThumb
          mediaId={item.id}
          variant="original"
          alt={item.filename}
          className="max-h-full max-w-full rounded-lg object-contain"
        />
        {nextItem && (
          <button
            onClick={() => goTo(nextItem)}
            className="absolute right-4 top-1/2 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white transition hover:bg-white/20"
            aria-label="Next"
          >
            <ChevronRight size={22} />
          </button>
        )}
      </div>

      {item.exif && (
        <div className="flex flex-wrap gap-x-6 gap-y-1 border-t border-white/10 px-6 py-3 text-[11px] text-white/70">
          {item.exif.cameraModel && <span>{item.exif.cameraModel}</span>}
          {item.exif.focalLength && <span>{item.exif.focalLength}mm</span>}
          {item.exif.fNumber && <span>f/{item.exif.fNumber}</span>}
          {item.exif.exposureTime && <span>{item.exif.exposureTime}s</span>}
          {item.exif.iso && <span>ISO {item.exif.iso}</span>}
        </div>
      )}
    </div>
  );
}
