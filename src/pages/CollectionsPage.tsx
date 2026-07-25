import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { FolderOpen, Heart, Sparkles, Trash2 } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { MediaThumb } from "@/components/media/MediaThumb";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { isTauri, listFoldersWithStats } from "@/lib/tauri";
import type { FolderStats } from "@/types/media";

export function CollectionsPage() {
  const [folders, setFolders] = useState<FolderStats[]>([]);
  const stats = useLibraryStats();

  useEffect(() => {
    if (!isTauri()) return;
    listFoldersWithStats().then(setFolders);
  }, []);

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Collections"
        title="Albums and smart groupings."
        description="Every watched folder becomes an album automatically."
      />

      <div className="mt-7 flex items-start gap-3 rounded-2xl border border-dashed border-ink/[.15] p-4 text-xs text-ink-muted">
        <Sparkles size={15} className="mt-0.5 shrink-0 text-honey-deep" />
        <p>
          Smart albums (auto-grouped by event, trip, or content) arrive once the AI organization
          pipeline lands. For now, collections mirror your watched folders.
        </p>
      </div>

      <h2 className="mt-8 text-sm font-extrabold text-ink">Albums</h2>
      {folders.length === 0 ? (
        <div className="mt-4 rounded-3xl border border-dashed border-ink/[.15] p-12 text-center text-sm text-ink-muted">
          No folders yet — add one from the Gallery page.
        </div>
      ) : (
        <div className="mt-4 grid grid-cols-4 gap-4">
          {folders.map((entry) => (
            <Link key={entry.folder.id} to={`/gallery?folder=${entry.folder.id}`} className="group block">
              <div className="artwork-frame aspect-square">
                {entry.coverMediaId ? (
                  <MediaThumb
                    mediaId={entry.coverMediaId}
                    alt={entry.folder.name}
                    className="size-full object-cover transition duration-500 group-hover:scale-[1.03]"
                  />
                ) : (
                  <div className="grid size-full place-items-center bg-shell text-ink-muted">
                    <FolderOpen size={22} />
                  </div>
                )}
              </div>
              <p className="mt-2 truncate text-xs font-extrabold text-ink">{entry.folder.name}</p>
              <p className="text-[11px] text-ink-muted">
                {entry.itemCount} item{entry.itemCount === 1 ? "" : "s"}
              </p>
            </Link>
          ))}
        </div>
      )}

      <div className="mt-8 grid grid-cols-2 gap-4">
        <Link to="/search?favorites=1">
          <Card className="flex items-center gap-3 p-5 transition hover:border-honey/40">
            <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
              <Heart size={18} />
            </div>
            <div>
              <p className="text-sm font-extrabold text-ink">Favorites</p>
              <p className="text-xs text-ink-muted">{stats?.favorites.toLocaleString() ?? "—"} items</p>
            </div>
          </Card>
        </Link>
        <Link to="/trash">
          <Card className="flex items-center gap-3 p-5 transition hover:border-honey/40">
            <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
              <Trash2 size={18} />
            </div>
            <div>
              <p className="text-sm font-extrabold text-ink">Trash</p>
              <p className="text-xs text-ink-muted">{stats?.trashed.toLocaleString() ?? "—"} items</p>
            </div>
          </Card>
        </Link>
      </div>
    </div>
  );
}
