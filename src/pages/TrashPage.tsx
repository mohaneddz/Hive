import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { RotateCcw, Trash2, Trash, VideoIcon } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaThumb } from "@/components/media/MediaThumb";
import { deleteMediaPermanently, emptyTrash, getMediaPage, isTauri, setTrashed } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem } from "@/types/media";
import { formatCount, relativeTime } from "@/utils/format";

const PAGE_SIZE = 120;

const deletedAgo = (isoDate: string | null) => `Deleted ${relativeTime(isoDate)}`;

export function TrashPage() {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    try {
      const page = await getMediaPage({ limit: PAGE_SIZE, offset: 0, scope: "trash" });
      setItems(page.items);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Trashing from the viewer emits this too, so the page stays in sync.
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("media:changed", () => {
      void load();
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [load]);

  const restore = async (item: MediaItem) => {
    setBusyId(item.id);
    try {
      await setTrashed(item.id, false);
      setItems((prev) => prev.filter((entry) => entry.id !== item.id));
    } finally {
      setBusyId(null);
    }
  };

  const deleteForever = async (item: MediaItem) => {
    const confirmed = await confirm(
      `Send "${item.filename}" to the Windows recycle bin and remove it from Hive?`,
      { title: "Delete permanently", kind: "warning" },
    );
    if (!confirmed) return;

    setBusyId(item.id);
    try {
      await deleteMediaPermanently(item.id);
      setItems((prev) => prev.filter((entry) => entry.id !== item.id));
    } finally {
      setBusyId(null);
    }
  };

  const emptyAll = async () => {
    const confirmed = await confirm(
      `Send all ${formatCount(items.length, "item")} to the Windows recycle bin?`,
      { title: "Empty trash", kind: "warning" },
    );
    if (!confirmed) return;

    await emptyTrash();
    await load();
  };

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Trash"
        title="Recently deleted."
        description={
          items.length === 0
            ? "Items you delete land here first. Nothing leaves your disk until you say so."
            : `${formatCount(items.length, "item")} waiting. Restore them, or clear them for good.`
        }
        action={
          items.length > 0 ? (
            <Button variant="secondary" icon={<Trash size={16} />} onClick={emptyAll}>
              Empty trash
            </Button>
          ) : undefined
        }
      />

      {loading && items.length === 0 && (
        <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
      )}

      {!loading && items.length === 0 && (
        <EmptyState
          icon={<Trash2 size={22} />}
          title="Trash is empty"
          description="Deleted photos and videos stay here until you remove them for good."
        />
      )}

      {items.length > 0 && (
        <div className="mt-7 grid grid-cols-4 gap-4">
          {items.map((item) => (
            <article key={item.id} className="group relative">
              <div className="artwork-frame block aspect-square">
                <MediaThumb
                  mediaId={item.id}
                  alt={item.filename}
                  className="relative size-full object-cover opacity-60 transition group-hover:opacity-100"
                />
                {item.mediaType === "video" && (
                  <span className="absolute left-3 top-3 grid size-7 place-items-center rounded-full bg-black/55 text-white">
                    <VideoIcon size={13} />
                  </span>
                )}
                <div className="absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 bg-gradient-to-t from-black/70 to-transparent p-4 opacity-0 transition group-hover:opacity-100">
                  <button
                    onClick={() => restore(item)}
                    disabled={busyId === item.id}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-white/90 px-3 py-1.5 text-xs font-bold text-ink transition hover:bg-white disabled:opacity-50"
                  >
                    <RotateCcw size={13} />
                    Restore
                  </button>
                  <button
                    onClick={() => deleteForever(item)}
                    disabled={busyId === item.id}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-red-600/90 px-3 py-1.5 text-xs font-bold text-white transition hover:bg-red-600 disabled:opacity-50"
                  >
                    <Trash2 size={13} />
                    Delete
                  </button>
                </div>
              </div>
              <p className="mt-2 truncate text-xs font-bold text-ink">{item.filename}</p>
              <p className="text-[11px] text-ink-muted">{deletedAgo(item.trashedAt)}</p>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
