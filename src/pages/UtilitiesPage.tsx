import { useCallback, useEffect, useState } from "react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Archive,
  Copy,
  Download,
  FileQuestion,
  HardDrive,
  Loader2,
  RefreshCw,
  Sparkles,
  Trash2,
  Wand2,
} from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { DuplicatesPanel } from "@/components/duplicates/DuplicatesPanel";
import { BackupPanel } from "@/components/utilities/BackupPanel";
import { BatchPanel } from "@/components/utilities/BatchPanel";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import {
  clearThumbnailCache,
  exportMedia,
  getMediaPage,
  getStorageStats,
  isTauri,
  removeMissingEntries,
  scanLibraryHealth,
} from "@/lib/tauri";
import type { LibraryHealth, StorageStats } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatBytes, formatCount, formatDateTime } from "@/utils/format";

type Tab = "storage" | "duplicates" | "health" | "batch" | "export" | "backup";

const TABS: { key: Tab; label: string; icon: typeof HardDrive }[] = [
  { key: "storage", label: "Storage", icon: HardDrive },
  { key: "duplicates", label: "Duplicates", icon: Copy },
  { key: "health", label: "Missing & broken", icon: AlertTriangle },
  { key: "batch", label: "Batch", icon: Wand2 },
  { key: "export", label: "Export", icon: Download },
  { key: "backup", label: "Backup", icon: Archive },
];

function UsageBar({ segments }: { segments: { label: string; bytes: number; className: string }[] }) {
  const total = segments.reduce((sum, segment) => sum + segment.bytes, 0) || 1;
  return (
    <>
      <div className="mt-5 flex h-3 overflow-hidden rounded-full bg-shell">
        {segments.map((segment) => (
          <div
            key={segment.label}
            className={segment.className}
            style={{ width: `${(segment.bytes / total) * 100}%` }}
            title={`${segment.label}: ${formatBytes(segment.bytes)}`}
          />
        ))}
      </div>
      <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1.5">
        {segments.map((segment) => (
          <span
            key={segment.label}
            className="inline-flex items-center gap-2 text-[11px] font-bold text-ink-muted"
          >
            <span className={cn("size-2.5 rounded-full", segment.className)} />
            {segment.label} — {formatBytes(segment.bytes)}
          </span>
        ))}
      </div>
    </>
  );
}

export function UtilitiesPage() {
  const [tab, setTab] = useState<Tab>("storage");
  const [storage, setStorage] = useState<StorageStats | null>(null);
  const [health, setHealth] = useState<LibraryHealth | null>(null);
  const [busy, setBusy] = useState(false);

  const loadStorage = useCallback(async () => {
    if (!isTauri()) return;
    setStorage(await getStorageStats());
  }, []);

  useEffect(() => {
    void loadStorage();
  }, [loadStorage]);

  const runHealth = async () => {
    setBusy(true);
    try {
      setHealth(await scanLibraryHealth());
    } finally {
      setBusy(false);
    }
  };

  const cleanMissing = async () => {
    const confirmed = await confirm(
      "Remove every entry whose file no longer exists? Nothing is deleted on disk — those files are already gone.",
      { title: "Clean missing entries", kind: "warning" },
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      const removed = await removeMissingEntries();
      await confirm(`${formatCount(removed, "entry", "entries")} removed from the index.`, {
        title: "Done",
        kind: "info",
      });
      setHealth(null);
      await loadStorage();
    } finally {
      setBusy(false);
    }
  };

  const clearCache = async () => {
    const confirmed = await confirm(
      "Delete every generated thumbnail? They are rebuilt on the next scan — this only costs time, never photos.",
      { title: "Clear thumbnail cache", kind: "warning" },
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      const freed = await clearThumbnailCache();
      await confirm(`${formatBytes(freed)} freed.`, { title: "Cache cleared", kind: "info" });
      await loadStorage();
    } finally {
      setBusy(false);
    }
  };

  const exportSet = async (mediaIds: string[]) => {
    if (mediaIds.length === 0) {
      await confirm("There is nothing to export yet.", { title: "Nothing to export", kind: "info" });
      return;
    }
    const destination = await open({ directory: true, multiple: false, title: "Export to…" });
    if (typeof destination !== "string") return;

    setBusy(true);
    try {
      const report = await exportMedia(mediaIds, destination);
      await confirm(`${report.exported} exported, ${report.skipped} skipped.`, {
        title: "Export finished",
        kind: "info",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Utilities"
        title="Tools for your library."
        description="Find what is duplicated or broken, batch-process your files, and keep a backup of everything Hive knows."
      />

      <div className="mt-7 flex flex-wrap items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
        {TABS.map((entry) => (
          <button
            key={entry.key}
            onClick={() => setTab(entry.key)}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-bold text-ink-muted transition",
              tab === entry.key && "bg-cream text-honey-deep",
            )}
          >
            <entry.icon size={14} />
            {entry.label}
          </button>
        ))}
      </div>

      {/* ------------------------------------------------------- storage -- */}
      {tab === "storage" && storage && (
        <>
          <Card className="mt-6 p-6">
            <div className="flex items-start justify-between gap-4">
              <div>
                <h2 className="text-base font-extrabold text-ink">Disk usage</h2>
                <p className="mt-0.5 text-xs text-ink-muted">
                  {formatCount(storage.totalItems, "item")} · {storage.imageCount.toLocaleString()}{" "}
                  photos · {storage.videoCount.toLocaleString()} videos
                </p>
              </div>
              <Button variant="secondary" icon={<RefreshCw size={15} />} onClick={loadStorage}>
                Refresh
              </Button>
            </div>
            <UsageBar
              segments={[
                { label: "Originals", bytes: storage.originalBytes, className: "bg-honey" },
                { label: "Thumbnails", bytes: storage.thumbnailBytes, className: "bg-honey-deep" },
                { label: "Database", bytes: storage.databaseBytes, className: "bg-ink-muted" },
              ]}
            />
            <p className="mt-4 text-[11px] text-ink-muted">
              Originals are your own files, counted where they already live. Only thumbnails and the
              database are storage Hive itself created.
            </p>
          </Card>

          <Card className="mt-4 flex items-center justify-between gap-4 p-6">
            <div>
              <h2 className="text-base font-extrabold text-ink">Thumbnail cache</h2>
              <p className="mt-0.5 text-xs text-ink-muted">
                {formatBytes(storage.thumbnailBytes)} of generated previews. Safe to delete at any
                time.
              </p>
            </div>
            <Button
              variant="secondary"
              icon={<Sparkles size={15} />}
              onClick={clearCache}
              disabled={busy}
            >
              Clear cache
            </Button>
          </Card>

          {storage.byFolder.length > 0 && (
            <Card className="mt-4 p-6">
              <h2 className="text-base font-extrabold text-ink">By folder</h2>
              <div className="mt-4 space-y-2">
                {storage.byFolder.map((folder) => (
                  <div
                    key={folder.folderId}
                    className="flex items-center gap-3 rounded-2xl border border-ink/[.08] bg-canvas p-3"
                  >
                    <HardDrive size={15} className="shrink-0 text-honey-deep" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs font-extrabold text-ink">{folder.name}</p>
                      <p className="truncate text-[11px] text-ink-muted">{folder.path}</p>
                    </div>
                    <div className="shrink-0 text-right">
                      <p className="text-xs font-extrabold text-ink">{formatBytes(folder.bytes)}</p>
                      <p className="text-[11px] text-ink-muted">
                        {formatCount(folder.itemCount, "item")}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </>
      )}

      {/* Perceptual duplicate detection is Mohaned's, and lives in his panel. */}
      {tab === "duplicates" && <DuplicatesPanel />}

      {/* -------------------------------------------------------- health -- */}
      {tab === "health" && (
        <>
          <Card className="mt-6 flex items-center justify-between gap-4 p-6">
            <div>
              <h2 className="text-base font-extrabold text-ink">Library health</h2>
              <p className="mt-0.5 max-w-xl text-xs text-ink-muted">
                Checks every indexed row against the disk. “Missing” means the file was moved or
                deleted outside Hive; “broken” means it is still there but no longer opens.
              </p>
            </div>
            <Button
              icon={busy ? <Loader2 size={15} className="animate-spin" /> : <AlertTriangle size={15} />}
              onClick={runHealth}
              disabled={busy}
            >
              {health ? "Scan again" : "Scan"}
            </Button>
          </Card>

          {health && (
            <>
              <div className="mt-4 grid grid-cols-3 gap-4">
                {[
                  { label: "Checked", value: health.checked, className: "text-ink" },
                  { label: "Missing", value: health.missing.length, className: "text-red-600" },
                  { label: "Broken", value: health.broken.length, className: "text-honey-deep" },
                ].map((entry) => (
                  <Card key={entry.label} className="p-5">
                    <p
                      className={cn(
                        "text-[26px] font-extrabold leading-none tracking-[-.03em]",
                        entry.className,
                      )}
                    >
                      {entry.value.toLocaleString()}
                    </p>
                    <p className="mt-1.5 text-[11px] font-bold text-ink-muted">{entry.label}</p>
                  </Card>
                ))}
              </div>

              {health.missing.length === 0 && health.broken.length === 0 && (
                <EmptyState
                  icon={<Sparkles size={22} />}
                  title="Everything checks out"
                  description="Every indexed file is present on disk and opens correctly."
                />
              )}

              {health.missing.length > 0 && (
                <Card className="mt-4 p-6">
                  <div className="flex items-center justify-between gap-4">
                    <h3 className="text-sm font-extrabold text-ink">
                      Missing files ({health.missing.length})
                    </h3>
                    <Button
                      variant="secondary"
                      icon={<Trash2 size={15} />}
                      onClick={cleanMissing}
                      disabled={busy}
                    >
                      Remove from index
                    </Button>
                  </div>
                  <div className="mt-4 max-h-72 space-y-1.5 overflow-y-auto pr-1">
                    {health.missing.map((item) => (
                      <div key={item.id} className="rounded-xl border border-ink/[.08] bg-canvas p-3">
                        <p className="truncate text-xs font-bold text-ink">{item.filename}</p>
                        <p className="truncate text-[10px] text-ink-muted">{item.path}</p>
                        <p className="text-[10px] text-ink-muted">
                          Indexed {formatDateTime(item.indexedAt)}
                        </p>
                      </div>
                    ))}
                  </div>
                </Card>
              )}

              {health.broken.length > 0 && (
                <Card className="mt-4 p-6">
                  <h3 className="text-sm font-extrabold text-ink">
                    Broken media ({health.broken.length})
                  </h3>
                  <p className="mt-0.5 text-xs text-ink-muted">
                    These files exist but cannot be decoded — usually a truncated or corrupted copy.
                  </p>
                  <div className="mt-4 max-h-72 space-y-1.5 overflow-y-auto pr-1">
                    {health.broken.map((item) => (
                      <div key={item.id} className="rounded-xl border border-ink/[.08] bg-canvas p-3">
                        <p className="truncate text-xs font-bold text-ink">{item.filename}</p>
                        <p className="truncate text-[10px] text-ink-muted">{item.path}</p>
                      </div>
                    ))}
                  </div>
                </Card>
              )}
            </>
          )}

          {!health && !busy && (
            <EmptyState
              icon={<FileQuestion size={22} />}
              title="Nothing scanned yet"
              description="Run a scan to check every indexed file against what is actually on your disk."
            />
          )}
        </>
      )}

      {tab === "batch" && <BatchPanel />}

      {tab === "backup" && <BackupPanel />}

      {/* -------------------------------------------------------- export -- */}
      {tab === "export" && (
        <Card className="mt-6 p-6">
          <h2 className="text-base font-extrabold text-ink">Export originals</h2>
          <p className="mt-0.5 max-w-xl text-xs text-ink-muted">
            Copies your original files into a folder you choose. Always a copy, never a move — an
            export can never damage your library. Name collisions get a “ (2)” suffix rather than
            overwriting anything.
          </p>
          <div className="mt-5 flex flex-wrap gap-3">
            <Button
              icon={<Download size={15} />}
              disabled={busy}
              onClick={async () => {
                const page = await getMediaPage({ limit: 10_000, offset: 0, favoritesOnly: true });
                await exportSet(page.items.map((item) => item.id));
              }}
            >
              Export all favorites
            </Button>
            <Button
              variant="secondary"
              icon={<Download size={15} />}
              disabled={busy}
              onClick={async () => {
                const page = await getMediaPage({ limit: 10_000, offset: 0 });
                await exportSet(page.items.map((item) => item.id));
              }}
            >
              Export whole library
            </Button>
          </div>
          <p className="mt-4 text-[11px] text-ink-muted">
            To export a hand-picked set, open an album and use its Select mode.
          </p>
        </Card>
      )}
    </div>
  );
}
