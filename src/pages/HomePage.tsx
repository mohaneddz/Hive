import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Activity, FolderOpen, Heart, ImagePlus, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { MediaCard } from "@/components/media/MediaCard";
import { MediaThumb } from "@/components/media/MediaThumb";
import { useJobProgress } from "@/hooks/useJobProgress";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { getMediaPage, isTauri, listFoldersWithStats } from "@/lib/tauri";
import { formatBytes } from "@/utils/format";
import type { FolderStats, MediaItem } from "@/types/media";

export function HomePage() {
  const stats = useLibraryStats();
  const jobs = useJobProgress();
  const [recent, setRecent] = useState<MediaItem[]>([]);
  const [favorites, setFavorites] = useState<MediaItem[]>([]);
  const [latestFolder, setLatestFolder] = useState<FolderStats | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    getMediaPage({ limit: 8, offset: 0 }).then((page) => setRecent(page.items));
    getMediaPage({ limit: 6, offset: 0, favoritesOnly: true }).then((page) => setFavorites(page.items));
    listFoldersWithStats().then((folders) => setLatestFolder(folders[0] ?? null));
  }, []);

  const activeJob = jobs.find((job) => job.status === "running");

  return (
    <div className="space-y-8">
      <div>
        <p className="eyebrow">Welcome back</p>
        <h1 className="mt-2 text-[30px] font-extrabold tracking-[-.04em] text-ink">
          Your library, at a glance.
        </h1>
      </div>

      {latestFolder ? (
        <Card className="relative overflow-hidden p-0">
          <div className="grid h-[220px] grid-cols-[1.1fr_1.4fr] items-stretch">
            <div className="flex flex-col justify-between p-7">
              <div>
                <p className="text-xs font-bold uppercase tracking-[.13em] text-ink-muted">
                  Continue where you left off
                </p>
                <div className="mt-4 flex items-center gap-2 text-ink">
                  <FolderOpen size={16} className="shrink-0 text-honey-deep" />
                  <h2 className="truncate text-lg font-extrabold">{latestFolder.folder.name}</h2>
                </div>
                <p className="mt-1 truncate text-xs text-ink-muted" title={latestFolder.folder.path}>
                  {latestFolder.itemCount} item{latestFolder.itemCount === 1 ? "" : "s"} · {latestFolder.folder.path}
                </p>
              </div>
              <Link to={`/gallery?folder=${latestFolder.folder.id}`}>
                <Button className="mt-6 w-fit">Open collection</Button>
              </Link>
            </div>
            <div className="relative h-full w-full overflow-hidden bg-shell">
              {latestFolder.coverMediaId && (
                <MediaThumb
                  mediaId={latestFolder.coverMediaId}
                  variant="md"
                  alt={latestFolder.folder.name}
                  className="absolute inset-0 size-full object-cover"
                />
              )}
            </div>
          </div>
        </Card>
      ) : (
        <Card className="flex flex-col items-center gap-3 p-12 text-center">
          <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
            <ImagePlus size={22} />
          </div>
          <p className="text-sm font-extrabold text-ink">Nothing indexed yet</p>
          <p className="text-xs text-ink-muted">Add a folder from the Gallery page to start building your library.</p>
          <Link to="/gallery">
            <Button className="mt-1">Go to Gallery</Button>
          </Link>
        </Card>
      )}

      <section>
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-extrabold text-ink">Recent photos</h2>
          <Link to="/gallery" className="text-xs font-bold text-honey-deep hover:underline">
            View all
          </Link>
        </div>
        {recent.length > 0 ? (
          <div className="mt-4 grid grid-cols-4 gap-4">
            {recent.map((item) => (
              <MediaCard key={item.id} item={item} />
            ))}
          </div>
        ) : (
          <p className="mt-3 text-xs text-ink-muted">Nothing indexed yet.</p>
        )}
      </section>

      <div className="grid grid-cols-2 gap-5">
        <Card className="p-6">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Heart size={15} className="text-honey-deep" />
              <h2 className="text-sm font-extrabold text-ink">Favorites</h2>
            </div>
            <Link to="/search" className="text-xs font-bold text-honey-deep hover:underline">
              View all
            </Link>
          </div>
          {favorites.length > 0 ? (
            <div className="mt-4 grid grid-cols-3 gap-2">
              {favorites.map((item) => (
                <Link key={item.id} to={`/media/${item.id}`} className="artwork-frame block aspect-square">
                  <MediaThumb mediaId={item.id} alt={item.filename} className="size-full object-cover" />
                </Link>
              ))}
            </div>
          ) : (
            <p className="mt-4 text-xs text-ink-muted">Photos you favorite will show up here.</p>
          )}
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-2">
            <Activity size={15} className="text-honey-deep" />
            <h2 className="text-sm font-extrabold text-ink">Library activity</h2>
          </div>
          <div className="mt-4 space-y-3 text-xs">
            {activeJob ? (
              <div className="flex items-center justify-between rounded-xl bg-honey/10 px-3 py-2 font-bold text-honey-deep">
                <span>Indexing…</span>
                <span>
                  {activeJob.current}/{activeJob.total}
                </span>
              </div>
            ) : (
              <div className="flex items-center gap-2 rounded-xl bg-canvas px-3 py-2 text-ink-muted">
                <Sparkles size={13} />
                <span>Up to date</span>
              </div>
            )}
            <div className="flex items-center justify-between text-ink-muted">
              <span>Items indexed</span>
              <span className="font-bold text-ink">{stats?.totalItems.toLocaleString() ?? "—"}</span>
            </div>
            <div className="flex items-center justify-between text-ink-muted">
              <span>On disk</span>
              <span className="font-bold text-ink">{stats ? formatBytes(stats.totalBytes) : "—"}</span>
            </div>
            <div className="flex items-center justify-between text-ink-muted">
              <span>Favorites</span>
              <span className="font-bold text-ink">{stats?.favorites.toLocaleString() ?? "—"}</span>
            </div>
            <div className="flex items-center justify-between text-ink-muted">
              <span>In trash</span>
              <span className="font-bold text-ink">{stats?.trashed.toLocaleString() ?? "—"}</span>
            </div>
          </div>
        </Card>
      </div>
    </div>
  );
}
