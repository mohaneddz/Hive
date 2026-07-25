import { Database, Download, FolderOpen, HardDrive, Image, ImageOff, Keyboard, Monitor, Moon, Palette, Pause, Play, Plus, RefreshCw, ScanText, Shield, Sparkles, Sun, Trash2, Users } from "lucide-react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useJobProgress } from "@/hooks/useJobProgress";
import { useTheme, type Theme } from "@/hooks/useTheme";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import {
  backfillEmbeddings,
  backfillFaces,
  backfillOcr,
  backfillThumbnails,
  downloadAiModels,
  applyCacheLimit,
  clearThumbnailCache,
  downloadFaceModels,
  downloadOcrModels,
  getCacheLimitMb,
  getGeocodingEnabled,
  getStorageStats,
  isTauri,
  setCacheLimitMb,
  setFolderWatched,
  setGeocodingEnabled,
} from "@/lib/tauri";
import { ShortcutEditor } from "@/components/settings/ShortcutEditor";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import type { StorageStats } from "@/types/media";
import { formatCount } from "@/utils/format";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { formatBytes } from "@/utils/format";
import { cn } from "@/utils/cn";

/** 0 keeps the cache unbounded, which is how Hive behaved before this existed. */
const CACHE_LIMITS = [
  { label: "No limit", megabytes: 0 },
  { label: "500 MB", megabytes: 500 },
  { label: "2 GB", megabytes: 2048 },
  { label: "5 GB", megabytes: 5120 },
];

const choices: { value: Theme; label: string; icon: typeof Sun; caption: string }[] = [
  { value: "light", label: "Light", icon: Sun, caption: "Warm gallery walls" },
  { value: "dark", label: "Dark", icon: Moon, caption: "A quieter viewing room" },
];

export function GallerySettingsPage() {
  const { theme, setTheme } = useTheme();
  const { folders, addFolder, removeFolder, rescan, refreshFolders } = useMediaLibrary();
  const { status: aiStatus, refresh: refreshAiStatus } = useAiStatus();
  const jobs = useJobProgress();
  const [downloading, setDownloading] = useState(false);
  const [backfilling, setBackfilling] = useState(false);
  const [ocrDownloading, setOcrDownloading] = useState(false);
  const [ocrBackfilling, setOcrBackfilling] = useState(false);
  const [faceDownloading, setFaceDownloading] = useState(false);
  const [faceBackfilling, setFaceBackfilling] = useState(false);
  const [thumbsRebuilding, setThumbsRebuilding] = useState(false);
  const [storage, setStorage] = useState<StorageStats | null>(null);
  const [cacheBusy, setCacheBusy] = useState(false);
  const [geocoding, setGeocoding] = useState(false);
  const [cacheLimit, setCacheLimit] = useState(0);
  const { stats } = useLibraryStats();

  useEffect(() => {
    if (!isTauri()) return;
    void getGeocodingEnabled().then(setGeocoding);
    void getCacheLimitMb().then(setCacheLimit);
  }, []);

  const loadStorage = useCallback(async () => {
    if (!isTauri()) return;
    setStorage(await getStorageStats());
  }, []);

  useEffect(() => {
    void loadStorage();
  }, [loadStorage]);

  const clearCache = async () => {
    const confirmed = await confirm(
      "Delete every generated thumbnail? They are rebuilt on the next scan — this only costs time, never photos.",
      { title: "Clear thumbnail cache", kind: "warning" },
    );
    if (!confirmed) return;

    setCacheBusy(true);
    try {
      const freed = await clearThumbnailCache();
      await confirm(`${formatBytes(freed)} freed.`, { title: "Cache cleared", kind: "info" });
      await loadStorage();
    } finally {
      setCacheBusy(false);
    }
  };

  const downloadJob = jobs.find((j) => j.kind === "download_models" && j.status === "running");
  const backfillJob = jobs.find((j) => j.kind === "embed_backfill" && j.status === "running");
  const ocrDownloadJob = jobs.find((j) => j.kind === "download_ocr_models" && j.status === "running");
  const ocrBackfillJob = jobs.find((j) => j.kind === "ocr_backfill" && j.status === "running");
  const faceDownloadJob = jobs.find((j) => j.kind === "download_face_models" && j.status === "running");
  const faceBackfillJob = jobs.find((j) => j.kind === "face_backfill" && j.status === "running");
  const thumbsJob = jobs.find((j) => j.kind === "thumbnail_backfill" && j.status === "running");

  const startDownload = async () => {
    setDownloading(true);
    try {
      await downloadAiModels();
      refreshAiStatus();
    } catch {
      /* surfaced via job status */
    } finally {
      setDownloading(false);
    }
  };

  const startBackfill = async () => {
    setBackfilling(true);
    try {
      await backfillEmbeddings();
      refreshAiStatus();
    } finally {
      setBackfilling(false);
    }
  };

  const startOcrDownload = async () => {
    setOcrDownloading(true);
    try {
      await downloadOcrModels();
      refreshAiStatus();
    } catch {
      /* surfaced via job status */
    } finally {
      setOcrDownloading(false);
    }
  };

  const startOcrBackfill = async () => {
    setOcrBackfilling(true);
    try {
      await backfillOcr();
      refreshAiStatus();
    } finally {
      setOcrBackfilling(false);
    }
  };

  const startFaceDownload = async () => {
    setFaceDownloading(true);
    try {
      await downloadFaceModels();
      refreshAiStatus();
    } catch {
      /* surfaced via job status */
    } finally {
      setFaceDownloading(false);
    }
  };

  const startFaceBackfill = async () => {
    setFaceBackfilling(true);
    try {
      await backfillFaces();
      refreshAiStatus();
    } finally {
      setFaceBackfilling(false);
    }
  };

  const startThumbnailRebuild = async () => {
    setThumbsRebuilding(true);
    try {
      await backfillThumbnails();
    } finally {
      setThumbsRebuilding(false);
    }
  };

  const chooseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Choose a media folder" });
      if (typeof selected === "string") await addFolder(selected);
    } catch {
      /* The web preview has no native folder picker. */
    }
  };

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Preferences"
        title="Make Hive yours."
        description="Choose which folders Hive watches and how your library feels."
      />
      <div className="mt-7 grid max-w-4xl gap-5">
        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
              <FolderOpen size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Watched folders</h2>
              <p className="mt-0.5 text-xs text-ink-muted">
                Hive indexes photos and videos in these folders and keeps them in sync automatically.
              </p>
            </div>
          </div>
          <div className="mt-5 space-y-2">
            {folders.length === 0 && (
              <div className="rounded-2xl border border-dashed border-ink/[.14] p-5 text-center text-xs text-ink-muted">
                No folders connected yet. Add a folder to get started.
              </div>
            )}
            {folders.map((folder) => (
              <div
                key={folder.id}
                className="flex items-center gap-3 rounded-2xl border border-ink/[.08] bg-canvas p-3"
              >
                <Image
                  size={16}
                  className={cn("shrink-0", folder.isWatched ? "text-honey-deep" : "text-ink-muted")}
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-semibold text-ink">{folder.path}</span>
                  {!folder.isWatched && (
                    <span className="block text-[10px] font-bold text-ink-muted">
                      Watching paused — new files are not picked up
                    </span>
                  )}
                </span>
                <button
                  onClick={async () => {
                    await setFolderWatched(folder.id, !folder.isWatched);
                    await refreshFolders();
                  }}
                  className="icon-button !h-8 !w-8"
                  aria-label={
                    folder.isWatched
                      ? `Pause watching ${folder.path}`
                      : `Resume watching ${folder.path}`
                  }
                  title={folder.isWatched ? "Pause watching" : "Resume watching"}
                >
                  {folder.isWatched ? <Pause size={13} /> : <Play size={13} />}
                </button>
                <button
                  onClick={() => rescan(folder.id)}
                  className="icon-button !h-8 !w-8"
                  aria-label={`Rescan ${folder.path}`}
                >
                  <RefreshCw size={13} />
                </button>
                <button
                  onClick={() => removeFolder(folder.id)}
                  className="icon-button !h-8 !w-8"
                  aria-label={`Remove ${folder.path}`}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            ))}
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <button
              onClick={chooseFolder}
              className="inline-flex items-center gap-2 rounded-xl bg-honey px-4 py-2.5 text-xs font-extrabold text-[#3b2900] transition hover:bg-honey-dark"
            >
              <Plus size={15} /> Add folder
            </button>
            {folders.length > 0 && (
              <Button
                variant="secondary"
                icon={<ImageOff size={14} />}
                disabled={thumbsRebuilding || !!thumbsJob}
                onClick={startThumbnailRebuild}
              >
                {thumbsJob
                  ? `Rebuilding… ${thumbsJob.current}/${thumbsJob.total}`
                  : "Rebuild missing thumbnails"}
              </Button>
            )}
          </div>
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
              <Sparkles size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">AI features</h2>
              <p className="mt-0.5 text-xs text-ink-muted">
                Semantic search runs a local CLIP model — everything happens on this device.
              </p>
            </div>
          </div>

          {!aiStatus?.modelsReady ? (
            <div className="mt-5 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0">
                <p className="text-xs font-bold text-ink">Local AI model</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {downloadJob
                    ? `Downloading… ${formatBytes(downloadJob.current)} / ${formatBytes(downloadJob.total)}`
                    : "~150 MB, one-time download from Hugging Face."}
                </p>
              </div>
              <Button
                icon={<Download size={14} />}
                disabled={downloading || !!downloadJob}
                onClick={startDownload}
                className="shrink-0"
              >
                {downloadJob ? "Downloading…" : "Download"}
              </Button>
            </div>
          ) : (
            <div className="mt-5 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0">
                <p className="text-xs font-bold text-ink">Semantic search is ready</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {backfillJob
                    ? `Embedding photos… ${backfillJob.current}/${backfillJob.total}`
                    : `${aiStatus.embeddedCount.toLocaleString()} of ${aiStatus.eligibleCount.toLocaleString()} photos embedded.`}
                </p>
              </div>
              {aiStatus.embeddedCount < aiStatus.eligibleCount && (
                <Button
                  variant="secondary"
                  disabled={backfilling || !!backfillJob}
                  onClick={startBackfill}
                  className="shrink-0"
                >
                  {backfillJob ? "Working…" : "Embed remaining"}
                </Button>
              )}
            </div>
          )}

          {!aiStatus?.ocrModelsReady ? (
            <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0 flex items-center gap-3">
                <ScanText size={16} className="shrink-0 text-honey-deep" />
                <div>
                  <p className="text-xs font-bold text-ink">Text in photos (OCR)</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {ocrDownloadJob
                      ? `Downloading… ${formatBytes(ocrDownloadJob.current)} / ${formatBytes(ocrDownloadJob.total)}`
                      : "~96 MB, lets search find text inside photos."}
                  </p>
                </div>
              </div>
              <Button
                variant="secondary"
                icon={<Download size={14} />}
                disabled={ocrDownloading || !!ocrDownloadJob}
                onClick={startOcrDownload}
                className="shrink-0"
              >
                {ocrDownloadJob ? "Downloading…" : "Download"}
              </Button>
            </div>
          ) : (
            <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0 flex items-center gap-3">
                <ScanText size={16} className="shrink-0 text-honey-deep" />
                <div>
                  <p className="text-xs font-bold text-ink">Text in photos is searchable</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {ocrBackfillJob
                      ? `Reading photos… ${ocrBackfillJob.current}/${ocrBackfillJob.total}`
                      : `${aiStatus.ocrIndexedCount.toLocaleString()} of ${aiStatus.eligibleCount.toLocaleString()} photos scanned for text.`}
                  </p>
                </div>
              </div>
              {aiStatus.ocrIndexedCount < aiStatus.eligibleCount && (
                <Button
                  variant="secondary"
                  disabled={ocrBackfilling || !!ocrBackfillJob}
                  onClick={startOcrBackfill}
                  className="shrink-0"
                >
                  {ocrBackfillJob ? "Working…" : "Scan remaining"}
                </Button>
              )}
            </div>
          )}

          {!aiStatus?.faceModelsReady ? (
            <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0 flex items-center gap-3">
                <Users size={16} className="shrink-0 text-honey-deep" />
                <div>
                  <p className="text-xs font-bold text-ink">People (face recognition)</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {faceDownloadJob
                      ? `Downloading… ${formatBytes(faceDownloadJob.current)} / ${formatBytes(faceDownloadJob.total)}`
                      : "~67 MB, groups photos by the people in them."}
                  </p>
                </div>
              </div>
              <Button
                variant="secondary"
                icon={<Download size={14} />}
                disabled={faceDownloading || !!faceDownloadJob}
                onClick={startFaceDownload}
                className="shrink-0"
              >
                {faceDownloadJob ? "Downloading…" : "Download"}
              </Button>
            </div>
          ) : (
            <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0 flex items-center gap-3">
                <Users size={16} className="shrink-0 text-honey-deep" />
                <div>
                  <p className="text-xs font-bold text-ink">People</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {faceBackfillJob
                      ? `Scanning for faces… ${faceBackfillJob.current}/${faceBackfillJob.total}`
                      : `${aiStatus.peopleCount.toLocaleString()} people found in ${aiStatus.facesIndexedCount.toLocaleString()} of ${aiStatus.eligibleCount.toLocaleString()} photos.`}
                  </p>
                </div>
              </div>
              {aiStatus.facesIndexedCount < aiStatus.eligibleCount && (
                <Button
                  variant="secondary"
                  disabled={faceBackfilling || !!faceBackfillJob}
                  onClick={startFaceBackfill}
                  className="shrink-0"
                >
                  {faceBackfillJob ? "Working…" : "Scan remaining"}
                </Button>
              )}
            </div>
          )}
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
              <Palette size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Appearance</h2>
              <p className="mt-0.5 text-xs text-ink-muted">Choose a view that feels right.</p>
            </div>
          </div>
          <div className="mt-6 grid grid-cols-2 gap-4">
            {choices.map((choice) => (
              <button
                key={choice.value}
                onClick={() => setTheme(choice.value)}
                className={cn(
                  "flex items-center gap-4 rounded-2xl border p-4 text-left transition",
                  theme === choice.value ? "border-honey bg-honey/12" : "border-ink/[.08] bg-canvas hover:border-honey/40",
                )}
              >
                <div className="grid size-10 place-items-center rounded-xl bg-panel text-ink">
                  <choice.icon size={18} />
                </div>
                <div>
                  <p className="text-sm font-extrabold text-ink">{choice.label}</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">{choice.caption}</p>
                </div>
              </button>
            ))}
          </div>
          <div className="mt-6 flex items-center gap-3 rounded-2xl border border-ink/[.07] bg-canvas p-4 text-xs text-ink-muted">
            <Monitor size={16} />
            <span>Hive uses your saved preference every time it opens.</span>
          </div>
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
              <HardDrive size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Storage and cache</h2>
              <p className="mt-0.5 text-xs text-ink-muted">
                What Hive keeps on disk, and what is safe to reclaim.
              </p>
            </div>
          </div>
          <div className="mt-5 grid grid-cols-3 gap-3">
            {[
              {
                label: "Originals",
                value: storage ? formatBytes(storage.originalBytes) : "—",
                hint: "Your own files",
              },
              {
                label: "Thumbnails",
                value: storage ? formatBytes(storage.thumbnailBytes) : "—",
                hint: "Created by Hive",
              },
              {
                label: "Database",
                value: storage ? formatBytes(storage.databaseBytes) : "—",
                hint: "Index and metadata",
              },
            ].map((entry) => (
              <div key={entry.label} className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
                <p className="text-lg font-extrabold leading-none text-ink">{entry.value}</p>
                <p className="mt-1.5 text-[11px] font-bold text-ink-muted">{entry.label}</p>
                <p className="text-[10px] text-ink-muted">{entry.hint}</p>
              </div>
            ))}
          </div>
          <div className="mt-4 flex flex-wrap gap-2">
            <Button variant="secondary" onClick={clearCache} disabled={cacheBusy}>
              Clear thumbnail cache
            </Button>
            <Button variant="secondary" icon={<RefreshCw size={14} />} onClick={loadStorage}>
              Refresh
            </Button>
          </div>
          <p className="mt-3 text-[11px] text-ink-muted">
            Only thumbnails and the database are storage Hive created. Clearing the cache costs
            time on the next scan, never photos.
          </p>

          <div className="mt-5 border-t border-ink/[.07] pt-5">
            <p className="text-xs font-extrabold text-ink">Cache ceiling</p>
            <p className="mt-1 max-w-xl text-[11px] leading-relaxed text-ink-muted">
              Thumbnails otherwise grow for as long as you add photos. With a ceiling set, Hive
              drops the ones you have not looked at in a while until the cache fits — they are
              rebuilt on demand, so nothing is lost but time.
            </p>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              {CACHE_LIMITS.map((entry) => (
                <button
                  key={entry.label}
                  onClick={async () => {
                    await setCacheLimitMb(entry.megabytes);
                    setCacheLimit(entry.megabytes);
                    await applyCacheLimit();
                    await loadStorage();
                  }}
                  className={cn(
                    "rounded-xl border px-3.5 py-2 text-[11px] font-bold transition",
                    cacheLimit === entry.megabytes
                      ? "border-honey bg-cream/55 text-honey-deep"
                      : "border-ink/[.1] bg-canvas text-ink hover:border-honey/40",
                  )}
                >
                  {entry.label}
                </button>
              ))}
            </div>
            <p className="mt-2.5 text-[10px] text-ink-muted">
              Applied when Hive starts, and whenever you change it here.
            </p>
          </div>
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
              <Database size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Library</h2>
              <p className="mt-0.5 text-xs text-ink-muted">What Hive currently knows about.</p>
            </div>
          </div>
          <div className="mt-5 flex flex-wrap gap-x-8 gap-y-3 text-xs">
            {[
              ["Items indexed", stats ? formatCount(stats.totalItems, "item") : "—"],
              ["Photos", stats ? stats.imageCount.toLocaleString() : "—"],
              ["Videos", stats ? stats.videoCount.toLocaleString() : "—"],
              ["Albums", stats ? stats.albumCount.toLocaleString() : "—"],
              ["Geotagged", stats ? stats.placeCount.toLocaleString() : "—"],
              ["Watched folders", formatCount(folders.length, "folder")],
            ].map(([label, value]) => (
              <div key={label}>
                <p className="text-[11px] text-ink-muted">{label}</p>
                <p className="font-extrabold text-ink">{value}</p>
              </div>
            ))}
          </div>
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
              <Keyboard size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Shortcuts</h2>
              <p className="mt-0.5 text-xs text-ink-muted">
                Click any key to rebind it. Changes apply immediately.
              </p>
            </div>
          </div>
          <ShortcutEditor />
        </Card>

        <Card className="p-6">
          <div className="flex items-center gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
              <Shield size={19} />
            </div>
            <div>
              <h2 className="text-base font-extrabold text-ink">Privacy</h2>
              <p className="mt-0.5 text-xs text-ink-muted">Where your library lives.</p>
            </div>
          </div>
          <p className="mt-5 text-xs leading-relaxed text-ink-muted">
            Hive is local-first. Your photos are read from the folders you choose and never copied
            anywhere else. The index, thumbnails, AI models and preferences all live in your own
            user profile, and recognition runs on this machine — no image is ever uploaded.
          </p>

          <div className="mt-5 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-xs font-extrabold text-ink">Look up place names</p>
                <p className="mt-1 max-w-xl text-[11px] leading-relaxed text-ink-muted">
                  Turns coordinates into names like “Lyon, France” on the Places page. There is no
                  offline way to do this, so lookups go to OpenStreetMap. What leaves your machine
                  is a pair of coordinates rounded to about a kilometre — no photo, no filename, no
                  identifier. Every answer is cached, so a place is looked up once and never again.
                </p>
              </div>
              <button
                role="switch"
                aria-checked={geocoding}
                onClick={async () => {
                  const next = !geocoding;
                  await setGeocodingEnabled(next);
                  setGeocoding(next);
                }}
                className={cn(
                  "relative mt-0.5 h-6 w-11 shrink-0 rounded-full transition",
                  geocoding ? "bg-honey" : "bg-ink/20",
                )}
              >
                <span
                  className={cn(
                    "absolute top-0.5 size-5 rounded-full bg-white shadow transition-all",
                    geocoding ? "left-[22px]" : "left-0.5",
                  )}
                />
              </button>
            </div>
            <p className="mt-3 text-[10px] font-bold text-ink-muted">
              {geocoding ? "On — names are fetched when you ask for them" : "Off — coordinates only"}
            </p>
          </div>
        </Card>
      </div>
    </div>
  );
}
