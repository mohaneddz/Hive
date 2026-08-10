import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Cpu,
  Eraser,
  Loader2,
  MousePointerClick,
  Scissors,
  Settings,
  Sparkles,
  Wand2,
  Zap,
} from "lucide-react";

import { Button } from "@/components/ui/Button";
import { useJobProgress } from "@/hooks/useJobProgress";
import {
  getAiEditorStatus,
  isTauri,
  previewErase,
  previewGenerate,
  previewRemoveBackground,
  previewUpscale,
  selectObject,
  warmSelection,
} from "@/lib/tauri";
import { routes } from "@/config/routes";
import type { AiEditorStatus, AiEditorTool, AiPreview, MediaItem, SelectPoint } from "@/types/media";
import { cn } from "@/utils/cn";

const readyOf = (status: AiEditorStatus | null, tool: AiEditorTool) =>
  status?.[`${tool}Ready` as const] ?? false;

/**
 * Roughly how long each tool takes on the processor, measured on a 480×360
 * picture. The graphics card is several times quicker.
 *
 * Shown before the button is pressed, not after. Twenty-five seconds of spinner
 * with no idea whether that is normal is what makes people think the app has
 * hung — and painting, at a dozen seconds a step, is far past that.
 */
const CPU_SECONDS = { cutout: 4, inpaint: 25, generatePerStep: 12 };
const GPU_SPEEDUP = 5;

function estimate(seconds: number, onGpu: boolean): string {
  const value = onGpu ? seconds / GPU_SPEEDUP : seconds;
  if (value < 10) return "a few seconds";
  if (value < 90) return `about ${Math.round(value / 5) * 5} seconds`;
  return `about ${Math.round(value / 60)} minute${value >= 90 ? "s" : ""}`;
}

/** How many denoising steps to run. Fewer is rougher, and much faster. */
const QUALITY = [
  { steps: 8, label: "Quick" },
  { steps: 20, label: "Better" },
];

/** Shown in place of a tool whose model has not been downloaded yet. */
function NeedsDownload({ label }: { label: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-ink/[.14] p-4 text-center">
      <p className="text-[11px] font-bold text-ink-muted">{label} is not downloaded yet.</p>
      <Link
        to={routes.settings.path}
        className="mt-2 inline-flex items-center gap-1.5 text-[11px] font-extrabold text-honey-deep hover:underline"
      >
        <Settings size={12} /> Get it in Settings
      </Link>
    </div>
  );
}

function ToolHeading({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof Zap;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-start gap-2.5">
      <span className="grid size-8 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
        <Icon size={15} />
      </span>
      <div className="min-w-0">
        <p className="text-xs font-extrabold text-ink">{title}</p>
        <p className="mt-1 text-[11px] leading-relaxed text-ink-muted">{children}</p>
      </div>
    </div>
  );
}

/** A numbered step in the erase flow, dimmed until its turn comes. */
function Step({
  number,
  done,
  active,
  children,
}: {
  number: number;
  done: boolean;
  active: boolean;
  children: React.ReactNode;
}) {
  return (
    <li className={cn("flex gap-2.5", !active && !done && "opacity-45")}>
      <span
        className={cn(
          "grid size-5 shrink-0 place-items-center rounded-full text-[10px] font-extrabold",
          done
            ? "bg-honey text-[#3b2900]"
            : active
              ? "bg-honey/25 text-honey-deep"
              : "bg-ink/10 text-ink-muted",
        )}
      >
        {done ? "âœ“" : number}
      </span>
      <span className="text-[11px] leading-relaxed text-ink-soft">{children}</span>
    </li>
  );
}

/**
 * The AI half of the editor.
 *
 * Nothing here saves. Each tool hands its result back to the page, which draws
 * it on the canvas exactly as it draws a brightness change — and the editor's
 * one Save button is what writes a file. That is why this panel has no
 * "keep the original?" choice of its own: there is only one save, and it asks.
 *
 * Downloading lives in Settings with every other model.
 */
export function AiToolsPanel({
  item,
  pending,
  selection,
  onSelectionChange,
  onPreview,
}: {
  item: MediaItem;
  /** What is already staged for this photo, if anything. */
  pending: AiPreview | null;
  /** Clicks collected on the canvas, in the photo's own pixels. */
  selection: SelectPoint[];
  onSelectionChange: (points: SelectPoint[]) => void;
  /** Hands a fresh result to the page, which shows it and enables Save. */
  onPreview: (preview: AiPreview) => void;
}) {
  const [status, setStatus] = useState<AiEditorStatus | null>(null);
  const [busy, setBusy] = useState<AiEditorTool | null>(null);
  const [maskUrl, setMaskUrl] = useState<string | null>(null);
  const [maskPng, setMaskPng] = useState<Uint8Array | null>(null);
  const [prompt, setPrompt] = useState("");
  const [steps, setSteps] = useState(QUALITY[0].steps);
  const [failure, setFailure] = useState<string | null>(null);
  const onGpu = Boolean(status?.gpuBackend);

  // A spinner alone reads as "nothing is happening" on anything that takes more
  // than a few seconds, and on the processor these take minutes. Both slow tools
  // report a real count through the job feed.
  const jobs = useJobProgress();
  const generateJob = jobs.find((job) => job.kind === "generate");
  const upscaleJob = jobs.find((job) => job.kind === "upscale");

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    setStatus(await getAiEditorStatus().catch(() => null));
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Encoding for click-to-select takes about seven seconds and every click after
  // it a tenth of one. Started here, while the photo is still being looked at,
  // rather than on the first click where the whole wait would land at once.
  useEffect(() => {
    if (!isTauri() || !readyOf(status, "segment")) return;
    void warmSelection(item.id).catch(() => {});
  }, [item.id, status, pending?.steps.length]);

  // A mask describes a particular set of pixels; a new photo, or a tool that
  // just changed them, makes it meaningless.
  useEffect(() => {
    setMaskPng(null);
    setMaskUrl((previous) => {
      if (previous) URL.revokeObjectURL(previous);
      return null;
    });
  }, [item.id, pending?.steps.length]);

  const run = async (tool: AiEditorTool, work: () => Promise<void>) => {
    setBusy(tool);
    setFailure(null);
    try {
      await work();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setBusy(null);
    }
  };

  const enlarge = () =>
    run("upscale", async () => onPreview(await previewUpscale(item.id)));

  const cutOut = (withBackground: boolean) =>
    run("cutout", async () => {
      let background: string | undefined;
      if (withBackground) {
        const picked = await open({
          multiple: false,
          title: "Pick a background",
          filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] }],
        });
        if (typeof picked !== "string") return;
        background = picked;
      }
      onPreview(await previewRemoveBackground(item.id, background));
    });

  const select = () =>
    run("segment", async () => {
      const png = await selectObject(item.id, selection);
      setMaskPng(png);
      setMaskUrl((previous) => {
        if (previous) URL.revokeObjectURL(previous);
        return URL.createObjectURL(new Blob([png as BlobPart], { type: "image/png" }));
      });
    });

  const erase = () =>
    run("inpaint", async () => {
      if (!maskPng) return;
      onPreview(await previewErase(item.id, maskPng));
      onSelectionChange([]);
    });

  const paint = () =>
    run("generate", async () => {
      if (!maskPng || !prompt.trim()) return;
      onPreview(await previewGenerate(item.id, maskPng, prompt.trim(), steps));
      onSelectionChange([]);
    });

  const anyReady =
    readyOf(status, "upscale") ||
    readyOf(status, "cutout") ||
    readyOf(status, "segment") ||
    readyOf(status, "inpaint");

  return (
    <div className="flex flex-1 flex-col gap-4 p-5">
      {!anyReady && (
        <div className="rounded-2xl border border-honey/30 bg-cream/45 p-4">
          <p className="text-xs font-extrabold text-ink">No AI tool downloaded yet</p>
          <p className="mt-1.5 text-[11px] leading-relaxed text-ink-muted">
            Each tool is a separate download, so you only fetch what you use. The smallest is
            Enlarge, at under 5 MB.
          </p>
          <Link
            to={routes.settings.path}
            className="mt-3 inline-flex items-center gap-1.5 rounded-xl bg-honey px-3 py-2 text-[11px] font-extrabold text-[#3b2900] transition hover:bg-honey-dark"
          >
            <Settings size={13} /> Open Settings
          </Link>
        </div>
      )}

      <p className="text-[11px] leading-relaxed text-ink-muted">
        Nothing here is saved until you press <b>Save</b> at the top. Try things freely â€”
        <b> Reset</b> puts the photo back exactly as it was.
      </p>

      {/* Says why it is slow, rather than leaving a spinner to imply a freeze. */}
      {status && !status.gpuBackend && (
        <p className="flex items-start gap-1.5 rounded-xl border border-ink/[.08] bg-canvas p-3 text-[11px] leading-relaxed text-ink-muted">
          <Cpu size={12} className="mt-0.5 shrink-0 text-honey-deep" />
          <span>
            Running on the processor, so these take minutes rather than seconds. Turning the
            graphics card on in{" "}
            <Link to={routes.settings.path} className="font-extrabold text-honey-deep hover:underline">
              Settings
            </Link>{" "}
            makes them several times faster.
          </span>
        </p>
      )}

      {/* ------------------------------------------------------- enlarge -- */}
      <div className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
        <ToolHeading icon={Zap} title="Enlarge Ã—4">
          Makes the photo four times wider and taller, inventing detail instead of stretching
          pixels. Use it on small or old photos.
        </ToolHeading>
        {readyOf(status, "upscale") ? (
          <Button
            className="mt-3 !h-8 w-full !text-[11px]"
            icon={busy === "upscale" ? <Loader2 size={13} className="animate-spin" /> : undefined}
            onClick={enlarge}
            disabled={busy !== null}
          >
            {busy === "upscale"
              ? upscaleJob && upscaleJob.total > 0
                ? `Enlargingâ€¦ ${Math.round((upscaleJob.current / upscaleJob.total) * 100)}%`
                : "Startingâ€¦"
              : "Enlarge"}
          </Button>
        ) : (
          <div className="mt-3">
            <NeedsDownload label="Enlarge" />
          </div>
        )}
      </div>

      {/* ---------------------------------------------------- background -- */}
      <div className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
        <ToolHeading icon={Scissors} title="Remove the background">
          Keeps the subject and drops everything behind it. <b>Make transparent</b> gives a PNG you
          can paste anywhere; <b>Put another photo behind</b> lets you pick a replacement.
        </ToolHeading>
        {readyOf(status, "cutout") ? (
          <div className="mt-3 grid gap-2">
            <Button
              variant="secondary"
              className="!h-8 !text-[11px]"
              onClick={() => cutOut(false)}
              disabled={busy !== null}
            >
              {busy === "cutout"
                ? "Workingâ€¦"
                : `Make transparent Â· ${estimate(CPU_SECONDS.cutout, onGpu)}`}
            </Button>
            <Button
              variant="secondary"
              className="!h-8 !text-[11px]"
              onClick={() => cutOut(true)}
              disabled={busy !== null}
            >
              Put another photo behind
            </Button>
          </div>
        ) : (
          <div className="mt-3">
            <NeedsDownload label="Remove background" />
          </div>
        )}
      </div>

      {/* --------------------------------------------------------- erase -- */}
      <div className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
        <ToolHeading icon={Eraser} title="Erase something">
          Make a person, a car or a passer-by disappear. The background behind them is invented to
          match.
        </ToolHeading>

        {readyOf(status, "segment") && readyOf(status, "inpaint") ? (
          <>
            <ol className="mt-3.5 space-y-2.5">
              <Step number={1} done={selection.length > 0} active={selection.length === 0}>
                <b>Click the thing</b> on the photo, to the left. A gold dot appears.
                <br />
                <span className="text-ink-muted">
                  Shift-click adds a red dot meaning &ldquo;not this part&rdquo;.
                </span>
              </Step>
              <Step
                number={2}
                done={maskPng !== null}
                active={selection.length > 0 && maskPng === null}
              >
                <b>Show selection.</b> The white area is what will disappear.
              </Step>
              <Step number={3} done={false} active={maskPng !== null}>
                <b>Erase it.</b> The result appears on the canvas.
              </Step>
            </ol>

            <div className="mt-3.5 grid grid-cols-2 gap-2">
              <Button
                variant="secondary"
                className="!h-8 !text-[11px]"
                icon={
                  busy === "segment" ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <MousePointerClick size={13} />
                  )
                }
                onClick={select}
                disabled={busy !== null || selection.length === 0}
              >
                {busy === "segment" ? "Lookingâ€¦" : "Show selection"}
              </Button>
              <Button
                variant="ghost"
                className="!h-8 !text-[11px]"
                onClick={() => onSelectionChange([])}
                disabled={selection.length === 0}
              >
                Clear dots
              </Button>
            </div>

            {maskUrl && (
              <div className="artwork-frame mt-3 aspect-video bg-black/85">
                <img
                  src={maskUrl}
                  alt="White is what will be erased"
                  className="size-full object-contain"
                />
              </div>
            )}

            <Button
              className="mt-3 !h-8 w-full !text-[11px]"
              icon={
                busy === "inpaint" ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Sparkles size={13} />
                )
              }
              onClick={erase}
              disabled={busy !== null || !maskPng}
            >
              {busy === "inpaint"
                ? "Erasingâ€¦"
                : `Erase it Â· ${estimate(CPU_SECONDS.inpaint, onGpu)}`}
            </Button>

          </>
        ) : (
          <div className="mt-3">
            <NeedsDownload label="Erasing (it needs two models)" />
          </div>
        )}
      </div>

      {/* ------------------------------------------------------ generate -- */}
      {/* Its own card, and always on screen. It was tucked inside the erase
          block and hidden until downloaded, which meant the one feature nobody
          would guess at was also the one nobody could see. */}
      <div className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
        <ToolHeading icon={Wand2} title="Paint something new">
          Select an area, describe what belongs there, and it is painted in â€” a hat, a vase of
          flowers, a different sky.
        </ToolHeading>

        {readyOf(status, "generate") ? (
          readyOf(status, "segment") ? (
            <>
              <p className="mt-3 text-[11px] font-bold text-honey-deep">
                {maskPng
                  ? "Selection ready. Describe what should replace it."
                  : "First pick an area with Show selection above."}
              </p>
              <input
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && maskPng && prompt.trim()) void paint();
                }}
                placeholder="a vase of white flowers"
                className="select-input mt-2.5 w-full"
              />
              <div className="mt-2.5 flex items-center gap-2">
                <span className="text-[10px] font-extrabold uppercase tracking-[.1em] text-ink-muted">
                  Quality
                </span>
                {QUALITY.map((option) => (
                  <button
                    key={option.steps}
                    onClick={() => setSteps(option.steps)}
                    className={cn(
                      "rounded-lg border px-2.5 py-1 text-[10px] font-bold transition",
                      steps === option.steps
                        ? "border-honey/50 bg-honey/15 text-honey-deep"
                        : "border-ink/[.08] bg-panel text-ink-soft hover:border-honey/40",
                    )}
                  >
                    {option.label}
                  </button>
                ))}
                <span className="ml-auto text-[10px] font-bold text-ink-muted">
                  ~{estimate(steps * CPU_SECONDS.generatePerStep, onGpu)}
                </span>
              </div>
              <Button
                className="mt-2 !h-8 w-full !text-[11px]"
                icon={
                  busy === "generate" ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <Wand2 size={13} />
                  )
                }
                onClick={paint}
                disabled={busy !== null || !maskPng || !prompt.trim()}
              >
                {busy === "generate"
                  ? generateJob && generateJob.total > 0
                    ? `Paintingâ€¦ ${generateJob.current}/${generateJob.total}`
                    : "Startingâ€¦"
                  : "Paint it in"}
              </Button>
              <p className="mt-2 text-[10px] leading-relaxed text-ink-muted">
                Press again for another attempt â€” every try starts from fresh noise, so no two are
                alike.
              </p>
            </>
          ) : (
            <div className="mt-3">
              <NeedsDownload label="Painting (it also needs Click to select)" />
            </div>
          )
        ) : (
          <div className="mt-3">
            <NeedsDownload label="Painting from a description (2.1 GB)" />
          </div>
        )}
      </div>

      {failure && (
        <p className="rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-[11px] leading-relaxed text-red-600">
          {failure}
        </p>
      )}
    </div>
  );
}
