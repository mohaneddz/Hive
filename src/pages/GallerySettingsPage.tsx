import { Download, FolderOpen, Image, ImageOff, Monitor, Moon, Palette, Plus, RefreshCw, ScanText, Sparkles, Sun, Trash2, Users } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

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
  downloadFaceModels,
  downloadOcrModels,
} from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { formatBytes } from "@/utils/format";
import { cn } from "@/utils/cn";

const choices: { value: Theme; label: string; icon: typeof Sun; caption: string }[] = [
  { value: "light", label: "Light", icon: Sun, caption: "Warm gallery walls" },
  { value: "dark", label: "Dark", icon: Moon, caption: "A quieter viewing room" },
];

export function GallerySettingsPage() {
  const { theme, setTheme } = useTheme();
  const { folders, addFolder, removeFolder, rescan } = useMediaLibrary();
  const { status: aiStatus, refresh: refreshAiStatus } = useAiStatus();
  const jobs = useJobProgress();
  const [downloading, setDownloading] = useState(false);
  const [backfilling, setBackfilling] = useState(false);
  const [ocrDownloading, setOcrDownloading] = useState(false);
  const [ocrBackfilling, setOcrBackfilling] = useState(false);
  const [faceDownloading, setFaceDownloading] = useState(false);
  const [faceBackfilling, setFaceBackfilling] = useState(false);
  const [thumbsRebuilding, setThumbsRebuilding] = useState(false);

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
                <Image size={16} className="shrink-0 text-honey-deep" />
                <span className="min-w-0 flex-1 truncate text-xs font-semibold text-ink">{folder.path}</span>
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
      </div>
    </div>
  );
}
