import { useCallback, useEffect, useState } from "react";
import { ChevronRight, Eye, Folder, FolderPlus, HardDrive, Home, Images } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import {
  addWatchedFolder,
  isTauri,
  listDirectory,
  listDrives,
  listFolders,
  parentDirectory,
  scanFolder,
} from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { ExplorerEntry, Folder as WatchedFolder } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCount } from "@/utils/format";

/** Splits "C:\Users\me\Pictures" into clickable crumbs. */
function breadcrumbs(path: string): { label: string; path: string }[] {
  const segments = path.split(/[\\/]+/).filter(Boolean);
  const crumbs: { label: string; path: string }[] = [];
  let current = "";
  segments.forEach((segment, index) => {
    current = index === 0 ? `${segment}\\` : `${current}${index === 1 ? "" : "\\"}${segment}`;
    crumbs.push({ label: segment, path: current });
  });
  return crumbs;
}

export function ExplorerPage() {
  const [path, setPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<ExplorerEntry[]>([]);
  const [watched, setWatched] = useState<WatchedFolder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    setError(null);
    try {
      setWatched(await listFolders());
      setEntries(path === null ? await listDrives() : await listDirectory(path));
    } catch (cause) {
      setError(String(cause));
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [path]);

  useEffect(() => {
    void load();
  }, [load]);

  const goUp = async () => {
    if (!path) return;
    const parent = await parentDirectory(path);
    setPath(parent);
  };

  const watch = async (entry: ExplorerEntry) => {
    const folder = await addWatchedFolder(entry.path);
    void scanFolder(folder.id);
    await load();
  };

  // The first row returned by `list_directory` describes the folder itself.
  const self = path !== null ? entries[0] : null;
  const children = path !== null ? entries.slice(1) : entries;

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Explorer"
        title="Browse your drives."
        description="Look through the filesystem exactly as it is, and hand any folder to Hive to index."
        action={
          self && !self.isWatched && self.mediaCount > 0 ? (
            <Button icon={<FolderPlus size={16} />} onClick={() => watch(self)}>
              Watch this folder
            </Button>
          ) : undefined
        }
      />

      <div className="mt-6 flex flex-wrap items-center gap-1.5 text-xs font-bold">
        <button
          onClick={() => setPath(null)}
          className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-ink-muted transition hover:bg-ink/5 hover:text-ink"
        >
          <HardDrive size={13} /> Drives
        </button>
        {path !== null &&
          breadcrumbs(path).map((crumb) => (
            <span key={crumb.path} className="flex items-center gap-1.5">
              <ChevronRight size={13} className="text-ink-muted" />
              <button
                onClick={() => setPath(crumb.path)}
                className="rounded-lg px-2 py-1 text-ink-muted transition hover:bg-ink/5 hover:text-ink"
              >
                {crumb.label}
              </button>
            </span>
          ))}
      </div>

      {watched.length > 0 && path === null && (
        <>
          <div className="mt-8 flex items-center gap-3">
            <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
              <Eye size={17} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Watched folders</h2>
              <p className="text-xs text-ink-muted">Already indexed and kept in sync</p>
            </div>
          </div>
          <div className="mt-4 grid grid-cols-2 gap-3">
            {watched.map((folder) => (
              <button
                key={folder.id}
                onClick={() => setPath(folder.path)}
                className="flex items-center gap-3 rounded-2xl border border-honey/30 bg-cream/40 p-3.5 text-left transition hover:border-honey/60"
              >
                <Folder size={17} className="shrink-0 text-honey-deep" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-extrabold text-ink">{folder.name}</span>
                  <span className="block truncate text-[11px] text-ink-muted">{folder.path}</span>
                </span>
              </button>
            ))}
          </div>
        </>
      )}

      {error && (
        <Card className="mt-7 border-red-500/30 p-5 text-xs font-semibold text-red-600">
          {error}
        </Card>
      )}

      {self && (
        <Card className="mt-7 flex items-center gap-4 p-5">
          <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
            <Images size={19} />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-extrabold text-ink">
              {formatCount(self.mediaCount, "media file")} in this folder
            </p>
            <p className="mt-0.5 truncate text-xs text-ink-muted">
              {self.indexedCount > 0
                ? `${self.indexedCount.toLocaleString()} indexed by Hive (including sub-folders)`
                : "Not indexed by Hive yet"}
            </p>
          </div>
          {self.isWatched && (
            <span className="shrink-0 rounded-full bg-honey/20 px-3 py-1 text-[11px] font-extrabold text-honey-deep">
              Watched
            </span>
          )}
        </Card>
      )}

      {loading && entries.length === 0 && (
        <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
      )}

      {!loading && children.length === 0 && !error && (
        <EmptyState
          icon={<Folder size={22} />}
          title="No sub-folders here"
          description="This folder has no sub-folders to descend into."
        />
      )}

      {children.length > 0 && (
        <div className="mt-5 space-y-2">
          {children.map((entry) => (
            <div
              key={entry.path}
              className={cn(
                "flex items-center gap-3 rounded-2xl border p-3.5 transition",
                entry.isWatched ? "border-honey/40 bg-cream/35" : "border-ink/[.08] bg-panel",
              )}
            >
              <button
                onClick={() => setPath(entry.path)}
                className="flex min-w-0 flex-1 items-center gap-3 text-left"
              >
                <Folder size={17} className="shrink-0 text-honey-deep" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-extrabold text-ink">{entry.name}</span>
                  <span className="block truncate text-[11px] text-ink-muted">
                    {entry.mediaCount > 0 ? formatCount(entry.mediaCount, "media file") : "No media directly inside"}
                    {entry.indexedCount > 0 && ` · ${entry.indexedCount.toLocaleString()} indexed`}
                  </span>
                </span>
              </button>
              {entry.isWatched ? (
                <span className="shrink-0 rounded-full bg-honey/20 px-3 py-1 text-[11px] font-extrabold text-honey-deep">
                  Watched
                </span>
              ) : (
                <button
                  onClick={() => watch(entry)}
                  className="shrink-0 rounded-xl border border-ink/10 px-3 py-1.5 text-[11px] font-extrabold text-ink transition hover:border-honey/50 hover:bg-cream/40"
                >
                  Watch
                </button>
              )}
              <ChevronRight size={15} className="shrink-0 text-ink-muted" />
            </div>
          ))}
        </div>
      )}

      {path !== null && (
        <button
          onClick={goUp}
          className="mt-6 inline-flex items-center gap-2 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <Home size={13} /> Up one level
        </button>
      )}
    </div>
  );
}
