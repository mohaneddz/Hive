import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Check, RotateCcw, Trash2, Trash, VideoIcon, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { PageError, PageSkeleton } from "@/components/ui/AsyncState";
import { MediaThumb } from "@/components/media/MediaThumb";
import { deleteMediaPermanently, emptyTrash, getMediaPage, isTauri, setTrashed } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem } from "@/types/media";
import { formatCount, relativeTime } from "@/utils/format";

const PAGE_SIZE = 60;

const deletedAgo = (isoDate: string | null) => `Deleted ${relativeTime(isoDate)}`;

export function TrashPage() {
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!isTauri()) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setFailure(null);
    try {
      const page = await getMediaPage({ limit: PAGE_SIZE, offset: 0, scope: "trash" });
      setItems(page.items);
      setTotal(page.total);
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  const loadMore = async () => {
    setLoading(true);
    try {
      const page = await getMediaPage({ limit: PAGE_SIZE, offset: items.length, scope: "trash" });
      setItems((prev) => [...prev, ...page.items.filter((item) => !prev.some((entry) => entry.id === item.id))]);
      setTotal(page.total);
    } finally {
      setLoading(false);
    }
  };

  const toggleSelected = (mediaId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(mediaId)) next.delete(mediaId);
      else next.add(mediaId);
      return next;
    });
  };

  const finishSelecting = () => {
    setSelecting(false);
    setSelected(new Set());
  };

  const restoreSelected = async () => {
    if (selected.size === 0) return;
    setBusy(true);
    try {
      await Promise.all([...selected].map((mediaId) => setTrashed(mediaId, false)));
      setItems((prev) => prev.filter((item) => !selected.has(item.id)));
      setTotal((prev) => Math.max(0, prev - selected.size));
      finishSelecting();
    } finally {
      setBusy(false);
    }
  };

  const deleteSelected = async () => {
    if (selected.size === 0) return;
    const confirmed = await confirm(
      `Erase ${formatCount(selected.size, "item")} from your disk?\n\nThis is permanent and cannot be recovered.`,
      { title: "Delete selected permanently", kind: "warning" },
    );
    if (!confirmed) return;
    setBusy(true);
    try {
      await Promise.all([...selected].map(deleteMediaPermanently));
      setItems((prev) => prev.filter((item) => !selected.has(item.id)));
      setTotal((prev) => Math.max(0, prev - selected.size));
      finishSelecting();
    } finally {
      setBusy(false);
    }
  };

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
      setTotal((prev) => Math.max(0, prev - 1));
    } finally {
      setBusyId(null);
    }
  };

  const deleteForever = async (item: MediaItem) => {
    const confirmed = await confirm(
      `Erase "${item.filename}" from your disk?\n\nThis is permanent — the file does not go to the Windows recycle bin and cannot be recovered.`,
      { title: "Delete permanently", kind: "warning" },
    );
    if (!confirmed) return;

    setBusyId(item.id);
    try {
      await deleteMediaPermanently(item.id);
      setItems((prev) => prev.filter((entry) => entry.id !== item.id));
      setTotal((prev) => Math.max(0, prev - 1));
    } finally {
      setBusyId(null);
    }
  };

  const emptyAll = async () => {
    const confirmed = await confirm(
      `Erase all ${formatCount(total, "item")} from your disk?\n\nThis is permanent — the files do not go to the Windows recycle bin and cannot be recovered.`,
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
          total === 0
            ? "Items you delete land here first. Nothing leaves your disk until you say so."
            : `${formatCount(total, "item")} waiting. Restore them, or erase them — deleting from here is permanent.`
        }
        action={
          items.length > 0 ? selecting ? (
            <div className="flex items-center gap-2">
              <Button variant="secondary" icon={<RotateCcw size={15} />} onClick={restoreSelected} disabled={selected.size === 0 || busy}>
                Restore ({selected.size})
              </Button>
              <Button variant="secondary" icon={<Trash2 size={15} />} onClick={deleteSelected} disabled={selected.size === 0 || busy}>
                Delete ({selected.size})
              </Button>
              <Button variant="ghost" icon={<X size={15} />} onClick={finishSelecting}>Done</Button>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <Button variant="secondary" onClick={() => setSelecting(true)}>Select</Button>
              <Button variant="secondary" icon={<Trash size={16} />} onClick={emptyAll}>
                Empty trash
              </Button>
            </div>
          ) : undefined
        }
      />

      {failure && <PageError message={failure} onRetry={() => void load()} />}
      {loading && items.length === 0 && !failure && <PageSkeleton cards={8} />}

      {!loading && items.length === 0 && !failure && (
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
              <div
                onClick={() => selecting && toggleSelected(item.id)}
                onKeyDown={(event) => {
                  if (selecting && (event.key === "Enter" || event.key === " ")) {
                    event.preventDefault();
                    toggleSelected(item.id);
                  }
                }}
                role={selecting ? "button" : undefined}
                tabIndex={selecting ? 0 : undefined}
                className="artwork-frame block aspect-square w-full text-left"
                aria-label={selecting ? `${selected.has(item.id) ? "Deselect" : "Select"} ${item.filename}` : item.filename}
              >
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
                {selecting && (
                  <span className={selected.has(item.id) ? "absolute right-3 top-3 grid size-7 place-items-center rounded-full bg-honey text-white" : "absolute right-3 top-3 size-7 rounded-full border-2 border-white/80 bg-black/25"}>
                    {selected.has(item.id) && <Check size={15} />}
                  </span>
                )}
                {!selecting && <div className="absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 bg-gradient-to-t from-black/70 to-transparent p-4 opacity-0 transition group-hover:opacity-100">
                  <button
                    onClick={(event) => { event.stopPropagation(); void restore(item); }}
                    disabled={busyId === item.id}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-white/90 px-3 py-1.5 text-xs font-bold text-ink transition hover:bg-white disabled:opacity-50"
                  >
                    <RotateCcw size={13} />
                    Restore
                  </button>
                  <button
                    onClick={(event) => { event.stopPropagation(); void deleteForever(item); }}
                    disabled={busyId === item.id}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-red-600/90 px-3 py-1.5 text-xs font-bold text-white transition hover:bg-red-600 disabled:opacity-50"
                  >
                    <Trash2 size={13} />
                    Delete
                  </button>
                </div>}
              </div>
              <p className="mt-2 truncate text-xs font-bold text-ink">{item.filename}</p>
              <p className="text-[11px] text-ink-muted">{deletedAgo(item.trashedAt)}</p>
            </article>
          ))}
        </div>
      )}
      {items.length > 0 && items.length < total && (
        <div className="mt-8 flex justify-center">
          <Button variant="secondary" onClick={loadMore} disabled={loading}>
            {loading ? "Loading…" : `Load more (${total - items.length} remaining)`}
          </Button>
        </div>
      )}
    </div>
  );
}
