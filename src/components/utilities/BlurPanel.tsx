import { useState } from "react";
import { Focus, Loader2, Sparkles, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaThumb } from "@/components/media/MediaThumb";
import { scanBlur, setTrashed } from "@/lib/tauri";
import type { BlurReport } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCount } from "@/utils/format";

/** Sharpness cut-offs, from "only the worst" to "anything slightly soft". */
const SENSITIVITY = [
  { label: "Very soft only", threshold: 40 },
  { label: "Balanced", threshold: 100 },
  { label: "Strict", threshold: 200 },
];

export function BlurPanel() {
  const [preset, setPreset] = useState(1);
  const [report, setReport] = useState<BlurReport | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (rescan = false) => {
    setBusy(true);
    try {
      setReport(await scanBlur(SENSITIVITY[preset].threshold, rescan));
    } finally {
      setBusy(false);
    }
  };

  const trashItem = async (mediaId: string) => {
    await setTrashed(mediaId, true);
    setReport((prev) =>
      prev ? { ...prev, items: prev.items.filter((entry) => entry.item.id !== mediaId) } : prev,
    );
  };

  return (
    <>
      <Card className="mt-6 p-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-base font-extrabold text-ink">Out-of-focus photos</h2>
            <p className="mt-0.5 max-w-2xl text-xs text-ink-muted">
              Hive measures sharpness with the variance of the Laplacian: it runs an edge-detection
              filter over each photo and looks at how much the result varies. Crisp photos have
              strong, varied edges and score high; smeared ones score low. It is arithmetic on
              pixels — nothing is guessed, and no model is involved.
            </p>
          </div>
          <Button
            icon={busy ? <Loader2 size={15} className="animate-spin" /> : <Focus size={15} />}
            onClick={() => run(false)}
            disabled={busy}
          >
            {report ? "Scan again" : "Scan"}
          </Button>
        </div>

        <div className="mt-5 grid grid-cols-3 gap-3">
          {SENSITIVITY.map((entry, index) => (
            <button
              key={entry.label}
              onClick={() => setPreset(index)}
              className={cn(
                "rounded-2xl border p-4 text-left transition",
                preset === index
                  ? "border-honey bg-cream/55"
                  : "border-ink/[.08] bg-canvas hover:border-honey/40",
              )}
            >
              <p className="text-sm font-extrabold text-ink">{entry.label}</p>
              <p className="mt-0.5 text-[11px] text-ink-muted">Flags below {entry.threshold}</p>
            </button>
          ))}
        </div>

        <p className="mt-3 text-[11px] text-ink-muted">
          Scores are measured once and kept, so changing the sensitivity is instant. Only a full
          rescan re-reads your photos.
        </p>
      </Card>

      {report && (
        <>
          <p className="mt-4 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
            {formatCount(report.measured, "photo")} measured
            {report.scanned > 0 && ` (${report.scanned} this run)`} ·{" "}
            {formatCount(report.items.length, "flagged as soft", "flagged as soft")}
          </p>

          {report.items.length === 0 ? (
            <EmptyState
              icon={<Sparkles size={22} />}
              title="Nothing looks soft"
              description="Every measured photo scores above the threshold. Try a stricter setting to see borderline ones."
            />
          ) : (
            <div className="mt-5 grid grid-cols-4 gap-4">
              {report.items.map((entry) => (
                <article key={entry.item.id} className="group relative">
                  <div className="artwork-frame block aspect-square">
                    <MediaThumb
                      mediaId={entry.item.id}
                      alt={entry.item.filename}
                      className="size-full object-cover"
                    />
                    <span className="absolute left-3 top-3 rounded-full bg-black/60 px-2.5 py-1 text-[11px] font-extrabold text-white">
                      {entry.score.toFixed(0)}
                    </span>
                    <div className="absolute inset-x-0 bottom-0 flex justify-center bg-gradient-to-t from-black/70 to-transparent p-4 opacity-0 transition group-hover:opacity-100">
                      <button
                        onClick={() => trashItem(entry.item.id)}
                        className="inline-flex items-center gap-1.5 rounded-lg bg-red-600/90 px-3 py-1.5 text-xs font-bold text-white transition hover:bg-red-600"
                      >
                        <Trash2 size={13} />
                        Move to trash
                      </button>
                    </div>
                  </div>
                  <p className="mt-2 truncate text-xs font-bold text-ink">{entry.item.filename}</p>
                  <p className="text-[11px] text-ink-muted">
                    Sharpness {entry.score.toFixed(0)} · threshold {report.threshold}
                  </p>
                </article>
              ))}
            </div>
          )}
        </>
      )}

      {!report && !busy && (
        <EmptyState
          icon={<Focus size={22} />}
          title="Nothing measured yet"
          description="Run a scan to score every photo in your library. The first pass reads each 800px preview; after that it is instant."
        />
      )}
    </>
  );
}
