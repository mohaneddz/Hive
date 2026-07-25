import { useEffect, useState } from "react";
import { FolderOpen, Image as ImageIcon } from "lucide-react";

import { MediaCard } from "@/components/media/MediaCard";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { getMediaPage, isTauri, listFoldersWithStats } from "@/lib/tauri";
import { cn } from "@/utils/cn";
import type { FolderStats, MediaItem } from "@/types/media";

export function ExplorerPage() {
  const [folders, setFolders] = useState<FolderStats[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    listFoldersWithStats().then((entries) => {
      setFolders(entries);
      if (entries.length > 0) setSelected(entries[0].folder.id);
    });
  }, []);

  useEffect(() => {
    if (!isTauri() || !selected) return;
    setLoading(true);
    getMediaPage({ limit: 200, offset: 0, folderId: selected })
      .then((page) => setItems(page.items))
      .finally(() => setLoading(false));
  }, [selected]);

  const selectedFolder = folders.find((f) => f.folder.id === selected);

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Explorer"
        title="Browse by folder."
        description="The raw file tree behind your library."
      />

      <div className="mt-7 grid grid-cols-[220px_1fr] gap-6">
        <div className="space-y-1">
          {folders.length === 0 && (
            <p className="text-xs text-ink-muted">No watched folders yet.</p>
          )}
          {folders.map((entry) => (
            <button
              key={entry.folder.id}
              onClick={() => setSelected(entry.folder.id)}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-left text-xs font-bold text-ink-muted transition hover:bg-ink/5",
                selected === entry.folder.id && "bg-cream/55 text-honey-deep",
              )}
            >
              <FolderOpen size={15} className="shrink-0" />
              <span className="min-w-0 flex-1 truncate">{entry.folder.name}</span>
              <span className="shrink-0 text-[10px] text-ink-muted">{entry.itemCount}</span>
            </button>
          ))}
        </div>

        <div>
          {selectedFolder && (
            <p className="mb-4 truncate text-[11px] text-ink-muted">{selectedFolder.folder.path}</p>
          )}
          {loading ? (
            <div className="text-center text-sm text-ink-muted">Loading…</div>
          ) : items.length === 0 ? (
            <div className="flex flex-col items-center gap-3 rounded-3xl border border-dashed border-ink/[.15] p-16 text-center">
              <ImageIcon size={22} className="text-ink-muted" />
              <p className="text-xs text-ink-muted">This folder has no indexed media.</p>
            </div>
          ) : (
            <div className="grid grid-cols-3 gap-4">
              {items.map((item) => (
                <MediaCard key={item.id} item={item} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
