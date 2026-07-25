import { useEffect, useState } from "react";
import { AlertTriangle, Copy, FileCheck2, Loader2, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import type { SaveMode } from "@/types/media";
import { cn } from "@/utils/cn";

const CHOICES: {
  mode: SaveMode;
  icon: typeof Copy;
  label: string;
  summary: string;
  detail: string;
  recommended?: boolean;
}[] = [
  {
    mode: "copy",
    icon: Copy,
    label: "Keep the original",
    summary: "Saves as a new photo beside it",
    detail:
      "Your original keeps its pixels and its camera data. The edited version joins your library as a separate item, inheriting the title and description.",
    recommended: true,
  },
  {
    mode: "overwrite",
    icon: FileCheck2,
    label: "Replace the original",
    summary: "Writes over the file on your disk",
    detail:
      "Favorites, albums, hidden and archived flags all carry over — it stays the same photo to Hive. But the original pixels are gone, and the file loses its embedded camera data.",
  },
];

export function SaveEditDialog({
  filename,
  saving,
  onCancel,
  onConfirm,
}: {
  filename: string;
  saving: boolean;
  onCancel: () => void;
  onConfirm: (mode: SaveMode) => void;
}) {
  // The safe answer is pre-selected: replacing is a deliberate act, not a default.
  const [mode, setMode] = useState<SaveMode>("copy");

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel, saving]);

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6 backdrop-blur-sm"
      onClick={() => !saving && onCancel()}
    >
      <div
        className="w-full max-w-lg rounded-[22px] border border-ink/[.08] bg-panel p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="eyebrow">Save edits</p>
            <h2 className="mt-1.5 text-lg font-extrabold text-ink">Keep the original?</h2>
            <p className="mt-0.5 truncate text-xs text-ink-muted">{filename}</p>
          </div>
          <button onClick={onCancel} className="icon-button" aria-label="Close" disabled={saving}>
            <X size={15} />
          </button>
        </div>

        <div className="mt-5 space-y-2.5">
          {CHOICES.map((choice) => (
            <button
              key={choice.mode}
              onClick={() => setMode(choice.mode)}
              aria-pressed={mode === choice.mode}
              className={cn(
                "flex w-full gap-3.5 rounded-2xl border p-4 text-left transition",
                mode === choice.mode
                  ? "border-honey bg-cream/55"
                  : "border-ink/[.08] bg-canvas hover:border-honey/40",
              )}
            >
              <span
                className={cn(
                  "mt-0.5 grid size-9 shrink-0 place-items-center rounded-xl",
                  mode === choice.mode ? "bg-honey text-[#3b2900]" : "bg-cream text-honey-deep",
                )}
              >
                <choice.icon size={17} />
              </span>
              <span className="min-w-0">
                <span className="flex items-center gap-2">
                  <span className="text-sm font-extrabold text-ink">{choice.label}</span>
                  {choice.recommended && (
                    <span className="rounded-full bg-honey/20 px-2 py-0.5 text-[10px] font-extrabold text-honey-deep">
                      Recommended
                    </span>
                  )}
                </span>
                <span className="mt-0.5 block text-[11px] font-bold text-ink-muted">
                  {choice.summary}
                </span>
                <span className="mt-1.5 block text-[11px] leading-relaxed text-ink-muted">
                  {choice.detail}
                </span>
              </span>
            </button>
          ))}
        </div>

        {mode === "overwrite" && (
          <p className="mt-4 flex items-start gap-2.5 rounded-2xl border border-red-500/30 bg-red-500/5 px-4 py-3 text-[11px] font-semibold leading-relaxed text-red-600">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <span>
              This cannot be undone. Hive keeps the camera details it already read, so Places and your
              timeline still work — but the file itself will no longer carry them for other programs.
            </span>
          </p>
        )}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel} disabled={saving}>
            Cancel
          </Button>
          <Button
            icon={saving ? <Loader2 size={15} className="animate-spin" /> : undefined}
            onClick={() => onConfirm(mode)}
            disabled={saving}
          >
            {saving ? "Saving…" : mode === "copy" ? "Save a copy" : "Replace original"}
          </Button>
        </div>
      </div>
    </div>
  );
}
