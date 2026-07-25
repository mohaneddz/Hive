import { Download, FolderOpen, Image, Monitor, Moon, Palette, Plus, RefreshCw, Sparkles, Sun, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useJobProgress } from "@/hooks/useJobProgress";
import { useTheme, type Theme } from "@/hooks/useTheme";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import { backfillEmbeddings, downloadAiModels } from "@/lib/tauri";
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

  const downloadJob = jobs.find((j) => j.kind === "download_models" && j.status === "running");
  const backfillJob = jobs.find((j) => j.kind === "embed_backfill" && j.status === "running");

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
          <button
            onClick={chooseFolder}
            className="mt-4 inline-flex items-center gap-2 rounded-xl bg-honey px-4 py-2.5 text-xs font-extrabold text-[#3b2900] transition hover:bg-honey-dark"
          >
            <Plus size={15} /> Add folder
          </button>
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
                  theme === choice.value ? "border-honey bg-cream/55" : "border-ink/[.08] bg-canvas hover:border-honey/40",
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
