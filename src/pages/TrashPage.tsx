import { useCallback, useEffect, useState } from "react";
import { RotateCcw, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { MediaThumb } from "@/components/media/MediaThumb";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { deleteMediaPermanently, getTrash, isTauri, setTrashed } from "@/lib/tauri";
import type { MediaItem } from "@/types/media";

export function TrashPage() {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(() => {
    if (!isTauri()) return;
    setLoading(true);
    getTrash(200, 0)
      .then((page) => setItems(page.items))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const restore = async (id: string) => {
    await setTrashed(id, false);
    setItems((prev) => prev.filter((item) => item.id !== id));
  };

  const deleteForever = async (id: string) => {
    if (!window.confirm("Permanently delete this file from disk? This can't be undone.")) return;
    await deleteMediaPermanently(id);
    setItems((prev) => prev.filter((item) => item.id !== id));
  };

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Trash"
        title="Recently deleted items."
        description="Restore items or delete them permanently from disk."
      />

      {loading ? (
        <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
      ) : items.length === 0 ? (
        <div className="mt-12 flex flex-col items-center gap-3 rounded-3xl border border-dashed border-ink/[.15] p-16 text-center">
          <Trash2 size={22} className="text-ink-muted" />
          <p className="text-xs text-ink-muted">Trash is empty.</p>
        </div>
      ) : (
        <div className="mt-7 grid grid-cols-4 gap-4">
          {items.map((item) => (
            <div key={item.id} className="group relative">
              <div className="artwork-frame aspect-square">
                <MediaThumb mediaId={item.id} alt={item.filename} className="size-full object-cover opacity-70" />
                <div className="absolute inset-0 flex items-end justify-center gap-2 bg-black/0 p-3 opacity-0 transition group-hover:bg-black/40 group-hover:opacity-100">
                  <Button variant="secondary" className="!h-8 px-2.5 text-[11px]" icon={<RotateCcw size={13} />} onClick={() => restore(item.id)}>
                    Restore
                  </Button>
                  <button
                    onClick={() => deleteForever(item.id)}
                    className="grid size-8 place-items-center rounded-xl bg-white/90 text-red-600 transition hover:bg-white"
                    aria-label="Delete forever"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
              <p className="mt-2 truncate text-xs text-ink-muted">{item.filename}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
