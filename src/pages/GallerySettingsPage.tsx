import { AlertTriangle, Database, Download, Eye, FileText, FolderOpen, HardDrive, Image, ImageOff, Keyboard, Monitor, Moon, Palette, Pause, Play, Plus, RefreshCw, ScanText, Shield, Sparkles, Star, Sun, Tag, Trash2, Users } from "lucide-react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useJobProgress } from "@/hooks/useJobProgress";
import { useTheme, type Theme } from "@/hooks/useTheme";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import {
  backfillAesthetic,
  backfillCaptions,
  backfillEmbeddings,
  backfillFaces,
  backfillNsfw,
  backfillOcr,
  backfillTags,
  backfillThumbnails,
  downloadAiModels,
  downloadCaptionModel,
  downloadFaceModels,
  downloadNsfwModel,
  downloadOcrModels,
  applyCacheLimit,
  clearThumbnailCache,
  downloadLlmModel,
  getCacheLimitMb,
  getGeocodingEnabled,
  getNsfwPolicy,
  getStorageStats,
  isTauri,
  setCacheLimitMb,
  setFolderWatched,
  setGeocodingEnabled,
  setNsfwPolicy as setNsfwPolicyCommand,
} from "@/lib/tauri";
import { ShortcutEditor } from "@/components/settings/ShortcutEditor";
import { useDownloadProgress } from "@/hooks/useDownloadProgress";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { refreshNsfwPolicy } from "@/hooks/useNsfwPolicy";
import type { NsfwPolicy, StorageStats } from "@/types/media";
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
  const [llmDownloading, setLlmDownloading] = useState(false);
  const [thumbsRebuilding, setThumbsRebuilding] = useState(false);
  const [storage, setStorage] = useState<StorageStats | null>(null);
  const [cacheBusy, setCacheBusy] = useState(false);
  const [geocoding, setGeocoding] = useState(false);
  const [nsfwPolicy, setNsfwPolicy] = useState<NsfwPolicy>({ threshold: 0.7, autoHide: false });
  const [cacheLimit, setCacheLimit] = useState(0);
  /// Model downloads create no job, so a failure has nowhere else to surface.
  const [failure, setFailure] = useState<string | null>(null);

  // Without these, a 230 MB download is indistinguishable from a dead button.
  const nsfwProgress = useDownloadProgress("nsfw:download:progress");
  const captionProgress = useDownloadProgress("caption:download:progress");
  const { stats } = useLibraryStats();

  useEffect(() => {
    if (!isTauri()) return;
    void getGeocodingEnabled().then(setGeocoding);
    void getCacheLimitMb().then(setCacheLimit);
    void getNsfwPolicy().then(setNsfwPolicy);
  }, []);

  /** Saves, then wakes every open grid so the covering matches the new threshold. */
  const saveNsfwPolicy = async (threshold: number, autoHide: boolean) => {
    setFailure(null);
    try {
      await setNsfwPolicyCommand(threshold, autoHide);
      setNsfwPolicy({ threshold, autoHide });
      refreshNsfwPolicy();
    } catch (cause) {
      setFailure(String(cause));
    }
  };

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

  const [tagBackfilling, setTagBackfilling] = useState(false);
  const [aestheticBackfilling, setAestheticBackfilling] = useState(false);
  const [nsfwDownloading, setNsfwDownloading] = useState(false);
  const [nsfwBackfilling, setNsfwBackfilling] = useState(false);
  const [captionDownloading, setCaptionDownloading] = useState(false);
  const [captionBackfilling, setCaptionBackfilling] = useState(false);

  const downloadJob = jobs.find((j) => j.kind === "download_models" && j.status === "running");
  const backfillJob = jobs.find((j) => j.kind === "embed_backfill" && j.status === "running");
  const ocrDownloadJob = jobs.find((j) => j.kind === "download_ocr_models" && j.status === "running");
  const ocrBackfillJob = jobs.find((j) => j.kind === "ocr_backfill" && j.status === "running");
  const faceDownloadJob = jobs.find((j) => j.kind === "download_face_models" && j.status === "running");
  const faceBackfillJob = jobs.find((j) => j.kind === "face_backfill" && j.status === "running");
  const tagBackfillJob = jobs.find((j) => j.kind === "tag_backfill" && j.status === "running");
  const aestheticBackfillJob = jobs.find((j) => j.kind === "aesthetic_backfill" && j.status === "running");
  const nsfwBackfillJob = jobs.find((j) => j.kind === "nsfw_backfill" && j.status === "running");
  const captionBackfillJob = jobs.find((j) => j.kind === "caption_backfill" && j.status === "running");
  const thumbsJob = jobs.find((j) => j.kind === "thumbnail_backfill" && j.status === "running");
  const llmDownloadJob = jobs.find((j) => j.kind === "download_llm_model" && j.status === "running");

  const startDownload = async () => {
    setDownloading(true);
    try {
      await downloadAiModels();
      refreshAiStatus();
    } catch (cause) {
      setFailure(String(cause));
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
    } catch (cause) {
      setFailure(String(cause));
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
    } catch (cause) {
      setFailure(String(cause));
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

  const startLlmDownload = async () => {
    setLlmDownloading(true);
    try {
      await downloadLlmModel();
      refreshAiStatus();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setLlmDownloading(false);
    }
  };

  const startTagBackfill = async () => {
    setTagBackfilling(true);
    try {
      await backfillTags();
      refreshAiStatus();
    } finally {
      setTagBackfilling(false);
    }
  };

  const startAestheticBackfill = async () => {
    setAestheticBackfilling(true);
    try {
      await backfillAesthetic();
      refreshAiStatus();
    } finally {
      setAestheticBackfilling(false);
    }
  };

  const startNsfwDownload = async () => {
    setNsfwDownloading(true);
    try {
      await downloadNsfwModel();
      refreshAiStatus();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setNsfwDownloading(false);
    }
  };

  const startNsfwBackfill = async () => {
    setNsfwBackfilling(true);
    try {
      await backfillNsfw();
      refreshAiStatus();
    } finally {
      setNsfwBackfilling(false);
    }
  };

  const startCaptionDownload = async () => {
    setCaptionDownloading(true);
    try {
      await downloadCaptionModel();
      refreshAiStatus();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setCaptionDownloading(false);
    }
  };

  const startCaptionBackfill = async () => {
    setCaptionBackfilling(true);
    try {
      await backfillCaptions();
      refreshAiStatus();
    } finally {
      setCaptionBackfilling(false);
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
      {failure && (
        <div className="mt-6 flex max-w-4xl items-start justify-between gap-4 rounded-2xl border border-red-500/30 bg-red-500/5 px-4 py-3">
          <div className="flex min-w-0 items-start gap-2.5">
            <AlertTriangle size={15} className="mt-0.5 shrink-0 text-red-600" />
            <div className="min-w-0">
              <p className="text-xs font-extrabold text-red-600">That didn’t work</p>
              <p className="mt-0.5 break-words text-[11px] text-red-600/90">{failure}</p>
            </div>
          </div>
          <button
            onClick={() => setFailure(null)}
            className="shrink-0 rounded-lg px-2 py-1 text-[11px] font-bold text-red-600 transition hover:bg-red-500/10"
          >
            Dismiss
          </button>
        </div>
      )}

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

          {!aiStatus?.llmModelsReady ? (
            <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="min-w-0 flex items-center gap-3">
                <Sparkles size={16} className="shrink-0 text-honey-deep" />
                <div>
                  <p className="text-xs font-bold text-ink">Gallery chat</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    {llmDownloadJob
                      ? `Downloading… ${formatBytes(llmDownloadJob.current)} / ${formatBytes(llmDownloadJob.total)}`
                      : "~1.1 GB, a small local model for asking questions about your photos."}
                  </p>
                </div>
              </div>
              <Button
                variant="secondary"
                icon={<Download size={14} />}
                disabled={llmDownloading || !!llmDownloadJob}
                onClick={startLlmDownload}
                className="shrink-0"
              >
                {llmDownloadJob ? "Downloading…" : "Download"}
              </Button>
            </div>
          ) : (
            <div className="mt-3 flex items-center gap-3 rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <Sparkles size={16} className="shrink-0 text-honey-deep" />
              <div>
                <p className="text-xs font-bold text-ink">Gallery chat is ready</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">Ask questions about your library from the Search page.</p>
              </div>
            </div>
          )}

          <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="min-w-0 flex items-center gap-3">
              <Tag size={16} className="shrink-0 text-honey-deep" />
              <div>
                <p className="text-xs font-bold text-ink">Auto-Tagging (CLIP)</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {tagBackfillJob
                    ? `Tagging photos… ${tagBackfillJob.current}/${tagBackfillJob.total}`
                    : "Automatically tags photos with categories (nature, dogs, food, etc.)."}
                </p>
              </div>
            </div>
            <Button
              variant="secondary"
              disabled={tagBackfilling || !!tagBackfillJob}
              onClick={startTagBackfill}
              className="shrink-0"
            >
              {tagBackfillJob ? "Tagging…" : "Run Auto-Tagging"}
            </Button>
          </div>

          <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="min-w-0 flex items-center gap-3">
              <Star size={16} className="shrink-0 text-honey-deep" />
              <div>
                <p className="text-xs font-bold text-ink">Aesthetic Ranking (LAION)</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {aestheticBackfillJob
                    ? `Scoring photos… ${aestheticBackfillJob.current}/${aestheticBackfillJob.total}`
                    : "Nothing to download — scores are read from the CLIP embeddings you already have."}
                </p>
              </div>
            </div>
            <div className="flex gap-2 shrink-0">
              <Button
                variant="secondary"
                disabled={aestheticBackfilling || !!aestheticBackfillJob}
                onClick={startAestheticBackfill}
              >
                {aestheticBackfillJob ? "Scoring…" : "Score photos"}
              </Button>
            </div>
          </div>

          <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="min-w-0 flex items-center gap-3">
              <Eye size={16} className="shrink-0 text-honey-deep" />
              <div>
                <p className="text-xs font-bold text-ink">Sensitive Content Detection (NSFW)</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {nsfwBackfillJob
                    ? `Scanning sensitive content… ${nsfwBackfillJob.current}/${nsfwBackfillJob.total}`
                    : "~10 MB model, automatically flags and hides sensitive images."}
                </p>
              </div>
            </div>
            <div className="flex gap-2 shrink-0">
              {!aiStatus?.nsfwModelsReady && (
                <Button
                  variant="secondary"
                  icon={<Download size={14} />}
                  disabled={nsfwDownloading}
                  onClick={startNsfwDownload}
                >
                  {nsfwProgress ? `${nsfwProgress.percent}%` : "Download"}
                </Button>
              )}
              <Button
                variant="secondary"
                disabled={nsfwBackfilling || !!nsfwBackfillJob}
                onClick={startNsfwBackfill}
              >
                {nsfwBackfillJob ? "Scanning…" : "Scan library"}
              </Button>
            </div>
          </div>

          <div className="mt-3 flex items-center justify-between gap-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="min-w-0 flex items-center gap-3">
              <FileText size={16} className="shrink-0 text-honey-deep" />
              <div>
                <p className="text-xs font-bold text-ink">Image Captions (ViT-GPT2)</p>
                <p className="mt-0.5 text-[11px] text-ink-muted">
                  {captionBackfillJob
                    ? `Generating captions… ${captionBackfillJob.current}/${captionBackfillJob.total}`
                    : "Generates natural text descriptions of your photos."}
                </p>
              </div>
            </div>
            <div className="flex gap-2 shrink-0">
              {!aiStatus?.captionModelsReady && (
                <Button
                  variant="secondary"
                  icon={<Download size={14} />}
                  disabled={captionDownloading}
                  onClick={startCaptionDownload}
                >
                  {captionProgress ? `${captionProgress.percent}%` : "Download"}
                </Button>
              )}
              <Button
                variant="secondary"
                disabled={captionBackfilling || !!captionBackfillJob}
                onClick={startCaptionBackfill}
              >
                {captionBackfillJob ? "Generating…" : "Generate captions"}
              </Button>
            </div>
          </div>
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

          <div className="mt-4 rounded-2xl border border-ink/[.08] bg-canvas p-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-xs font-extrabold text-ink">File sensitive photos away</p>
                <p className="mt-1 max-w-xl text-[11px] leading-relaxed text-ink-muted">
                  Photos the model scores above the threshold are always covered in the grid, and one
                  click reveals them. Turning this on also moves them out of the library into Hidden.
                  That is not a display choice — a wrongly flagged photo disappears from view, and no
                  classifier is right often enough for that to happen unasked, so it is off by default.
                </p>
              </div>
              <button
                role="switch"
                aria-checked={nsfwPolicy.autoHide}
                onClick={() => void saveNsfwPolicy(nsfwPolicy.threshold, !nsfwPolicy.autoHide)}
                className={cn(
                  "relative mt-0.5 h-6 w-11 shrink-0 rounded-full transition",
                  nsfwPolicy.autoHide ? "bg-honey" : "bg-ink/20",
                )}
              >
                <span
                  className={cn(
                    "absolute top-0.5 size-5 rounded-full bg-white shadow transition-all",
                    nsfwPolicy.autoHide ? "left-[22px]" : "left-0.5",
                  )}
                />
              </button>
            </div>

            <div className="mt-4 flex flex-wrap items-center gap-3">
              <span className="text-[11px] font-bold text-ink-muted">Sensitive above</span>
              {[0.5, 0.7, 0.85].map((value) => (
                <button
                  key={value}
                  onClick={() => void saveNsfwPolicy(value, nsfwPolicy.autoHide)}
                  className={cn(
                    "rounded-xl border px-3 py-1.5 text-[11px] font-bold transition",
                    Math.abs(nsfwPolicy.threshold - value) < 0.001
                      ? "border-honey/50 bg-honey/15 text-honey-deep"
                      : "border-ink/[.08] bg-panel text-ink-soft hover:border-honey/40",
                  )}
                >
                  {value === 0.5 ? "50% — strict" : value === 0.7 ? "70% — balanced" : "85% — lenient"}
                </button>
              ))}
            </div>
            <p className="mt-3 text-[10px] font-bold text-ink-muted">
              Changing the threshold re-covers the grid straight away; it does not rescan, and it
              never un-hides a photo that was already filed away.
            </p>
          </div>
        </Card>
      </div>
    </div>
  );
}
