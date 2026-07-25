import { useEffect, useState } from "react";
import { Check, FolderPlus, Plus, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { addMediaToAlbum, createAlbum, listAlbums, listAlbumsForMedia } from "@/lib/tauri";
import type { Album } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCount } from "@/utils/format";

/**
 * Picks one or more albums for a set of media. When exactly one item is passed,
 * the albums it already belongs to are pre-ticked and shown as such.
 */
export function AddToAlbumDialog({
  mediaIds,
  onClose,
  onAdded,
}: {
  mediaIds: string[];
  onClose: () => void;
  onAdded?: (albumName: string, added: number) => void;
}) {
  const [albums, setAlbums] = useState<Album[]>([]);
  const [alreadyIn, setAlreadyIn] = useState<string[]>([]);
  const [newAlbumName, setNewAlbumName] = useState("");
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void listAlbums().then(setAlbums);
    if (mediaIds.length === 1) {
      void listAlbumsForMedia(mediaIds[0]).then(setAlreadyIn);
    }
  }, [mediaIds]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const addTo = async (album: Album) => {
    setBusy(true);
    try {
      const added = await addMediaToAlbum(album.id, mediaIds);
      setAlreadyIn((prev) => [...new Set([...prev, album.id])]);
      onAdded?.(album.name, added);
    } finally {
      setBusy(false);
    }
  };

  const createAndAdd = async () => {
    const name = newAlbumName.trim();
    if (!name) return;
    setBusy(true);
    try {
      const album = await createAlbum(name);
      await addMediaToAlbum(album.id, mediaIds);
      setAlbums((prev) => [{ ...album, itemCount: mediaIds.length }, ...prev]);
      setAlreadyIn((prev) => [...prev, album.id]);
      setNewAlbumName("");
      setCreating(false);
      onAdded?.(album.name, mediaIds.length);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-6 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-[22px] border border-ink/[.08] bg-panel p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="eyebrow">Collections</p>
            <h2 className="mt-1.5 text-lg font-extrabold text-ink">Add to album</h2>
            <p className="mt-0.5 text-xs text-ink-muted">
              {formatCount(mediaIds.length, "item")} selected
            </p>
          </div>
          <button onClick={onClose} className="icon-button" aria-label="Close">
            <X size={15} />
          </button>
        </div>

        <div className="mt-5 max-h-64 space-y-1.5 overflow-y-auto pr-1">
          {albums.length === 0 && !creating && (
            <p className="rounded-2xl border border-dashed border-ink/[.14] p-5 text-center text-xs text-ink-muted">
              No albums yet. Create your first one below.
            </p>
          )}
          {albums.map((album) => {
            const isIn = alreadyIn.includes(album.id);
            return (
              <button
                key={album.id}
                onClick={() => addTo(album)}
                disabled={busy}
                className={cn(
                  "flex w-full items-center gap-3 rounded-2xl border p-3 text-left transition disabled:opacity-50",
                  isIn
                    ? "border-honey/50 bg-cream/55"
                    : "border-ink/[.08] bg-canvas hover:border-honey/40",
                )}
              >
                <span
                  className={cn(
                    "grid size-8 shrink-0 place-items-center rounded-xl",
                    isIn ? "bg-honey text-[#3b2900]" : "bg-cream text-honey-deep",
                  )}
                >
                  {isIn ? <Check size={15} strokeWidth={3} /> : <FolderPlus size={15} />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-extrabold text-ink">{album.name}</span>
                  <span className="block text-[11px] text-ink-muted">
                    {formatCount(album.itemCount, "item")}
                  </span>
                </span>
              </button>
            );
          })}
        </div>

        {creating ? (
          <div className="mt-4 flex items-center gap-2">
            <input
              autoFocus
              value={newAlbumName}
              onChange={(event) => setNewAlbumName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void createAndAdd();
              }}
              placeholder="Album name"
              className="search-input !pl-3.5"
            />
            <Button onClick={createAndAdd} disabled={busy || !newAlbumName.trim()}>
              Create
            </Button>
          </div>
        ) : (
          <button
            onClick={() => setCreating(true)}
            className="mt-4 inline-flex items-center gap-2 rounded-xl bg-honey px-4 py-2.5 text-xs font-extrabold text-[#3b2900] transition hover:bg-honey-dark"
          >
            <Plus size={15} /> New album
          </button>
        )}
      </div>
    </div>
  );
}
