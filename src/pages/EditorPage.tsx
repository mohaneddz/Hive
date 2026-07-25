import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { confirm } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  Check,
  Crop,
  FlipHorizontal,
  FlipVertical,
  Info,
  RotateCcw,
  RotateCw,
  Save,
  Sliders,
  Tag,
  Wand2,
} from "lucide-react";

import { Button } from "@/components/ui/Button";
import { EditorCanvas } from "@/components/editor/EditorCanvas";
import { SaveEditDialog } from "@/components/editor/SaveEditDialog";
import { applyEdits, getMediaDetail, readMediaUrl, updateMediaMetadata } from "@/lib/tauri";
import {
  FILTER_PRESETS,
  NEUTRAL_EDIT_OPS,
  type CropRect,
  type EditOps,
  type MediaItem,
  type SaveMode,
} from "@/types/media";
import { cn } from "@/utils/cn";
import { formatDate } from "@/utils/format";

type Tab = "filters" | "adjust" | "crop" | "metadata";

const TABS: { key: Tab; label: string; icon: typeof Sliders }[] = [
  { key: "filters", label: "Filters", icon: Wand2 },
  { key: "adjust", label: "Adjust", icon: Sliders },
  { key: "crop", label: "Frame", icon: Crop },
  { key: "metadata", label: "Details", icon: Tag },
];

const SLIDERS: { key: keyof EditOps; label: string; min: number; max: number; neutral: number }[] = [
  { key: "brightness", label: "Brightness", min: 0, max: 2, neutral: 1 },
  { key: "contrast", label: "Contrast", min: 0, max: 2, neutral: 1 },
  { key: "saturation", label: "Saturation", min: 0, max: 2, neutral: 1 },
  { key: "temperature", label: "Temperature", min: -1, max: 1, neutral: 0 },
  { key: "grayscale", label: "Black & white", min: 0, max: 1, neutral: 0 },
  { key: "sepia", label: "Sepia", min: 0, max: 1, neutral: 0 },
];

const CROP_PRESETS: { label: string; ratio: number | null }[] = [
  { label: "Free", ratio: null },
  { label: "Square", ratio: 1 },
  { label: "4:3", ratio: 4 / 3 },
  { label: "3:2", ratio: 3 / 2 },
  { label: "16:9", ratio: 16 / 9 },
];

/** The crop a preset produces: centred, and as large as the ratio allows. */
function centredCrop(ratio: number | null, frameRatio: number): CropRect | null {
  if (ratio === null) return null;
  const relative = ratio / frameRatio;
  return relative >= 1
    ? { x: 0, y: (1 - 1 / relative) / 2, width: 1, height: 1 / relative }
    : { x: (1 - relative) / 2, y: 0, width: relative, height: 1 };
}

function isNeutral(ops: EditOps): boolean {
  return (Object.keys(NEUTRAL_EDIT_OPS) as (keyof EditOps)[]).every(
    (key) => JSON.stringify(ops[key]) === JSON.stringify(NEUTRAL_EDIT_OPS[key]),
  );
}

export function EditorPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const [item, setItem] = useState<MediaItem | null>(null);
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const [ops, setOps] = useState<EditOps>(NEUTRAL_EDIT_OPS);
  const [tab, setTab] = useState<Tab>("filters");
  const [askingSave, setAskingSave] = useState(false);
  const [saving, setSaving] = useState(false);

  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [takenAtOverride, setTakenAtOverride] = useState("");
  const [savingMetadata, setSavingMetadata] = useState(false);

  // Crop dragging, in fractions of the displayed frame.
  const stageRef = useRef<HTMLDivElement>(null);
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    let objectUrl: string | null = null;

    void getMediaDetail(id).then((detail) => {
      if (cancelled) return;
      setItem(detail);
      setTitle(detail.title ?? "");
      setDescription(detail.description ?? "");
      setTakenAtOverride(detail.takenAtOverride?.slice(0, 10) ?? "");
    });

    // The 800px render is what the editor works against: a 50-megapixel original
    // would cost a hundred times more for a preview you cannot see the difference in.
    void readMediaUrl(id, "md")
      .then((url) => {
        if (cancelled) {
          URL.revokeObjectURL(url);
          return;
        }
        objectUrl = url;
        const element = new Image();
        element.onload = () => {
          if (!cancelled) setImage(element);
        };
        element.src = url;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [id]);

  // Rotating re-frames the picture, so a crop drawn on the old orientation no
  // longer means what it did.
  const rotate = (degrees: number) =>
    setOps((prev) => ({ ...prev, rotation: (prev.rotation + degrees + 360) % 360, crop: null }));

  const quarterTurned = ops.rotation === 90 || ops.rotation === 270;
  const naturalRatio = image ? image.naturalWidth / image.naturalHeight : 1;
  const frameRatio = quarterTurned ? 1 / naturalRatio : naturalRatio;

  const applyPreset = (ratio: number | null) =>
    setOps((prev) => ({ ...prev, crop: centredCrop(ratio, frameRatio) }));

  /* ------------------------------------------------------- crop by mouse -- */

  const pointFromEvent = useCallback((event: React.MouseEvent) => {
    const stage = stageRef.current;
    if (!stage) return null;
    const bounds = stage.getBoundingClientRect();
    return {
      x: Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width)),
      y: Math.min(1, Math.max(0, (event.clientY - bounds.top) / bounds.height)),
    };
  }, []);

  const beginCrop = (event: React.MouseEvent) => {
    if (tab !== "crop") return;
    const point = pointFromEvent(event);
    if (!point) return;
    setDragStart(point);
    setOps((prev) => ({ ...prev, crop: { x: point.x, y: point.y, width: 0, height: 0 } }));
  };

  const extendCrop = (event: React.MouseEvent) => {
    if (!dragStart) return;
    const point = pointFromEvent(event);
    if (!point) return;
    setOps((prev) => ({
      ...prev,
      crop: {
        x: Math.min(dragStart.x, point.x),
        y: Math.min(dragStart.y, point.y),
        width: Math.abs(point.x - dragStart.x),
        height: Math.abs(point.y - dragStart.y),
      },
    }));
  };

  const endCrop = () => {
    if (!dragStart) return;
    setDragStart(null);
    // A stray click should not leave a sliver of a crop behind.
    setOps((prev) =>
      prev.crop && (prev.crop.width < 0.02 || prev.crop.height < 0.02)
        ? { ...prev, crop: null }
        : prev,
    );
  };

  /* ------------------------------------------------------------- saving -- */

  const save = async (mode: SaveMode) => {
    if (!id) return;
    setSaving(true);
    try {
      const result = await applyEdits(id, ops, mode);
      setAskingSave(false);
      navigate(`/media/${result.id}`);
    } catch (cause) {
      await confirm(String(cause), { title: "Could not save", kind: "error" });
    } finally {
      setSaving(false);
    }
  };

  const saveMetadata = async () => {
    if (!id) return;
    setSavingMetadata(true);
    try {
      setItem(
        await updateMediaMetadata(
          id,
          title || null,
          description || null,
          takenAtOverride ? new Date(takenAtOverride).toISOString() : null,
        ),
      );
    } finally {
      setSavingMetadata(false);
    }
  };

  if (!item) {
    return <div className="grid h-full place-items-center text-sm text-ink-muted">Loading…</div>;
  }

  return (
    <div className="-mx-8 -my-7 flex h-[calc(100vh-40px)] flex-col bg-black/90">
      <div className="flex items-center justify-between gap-4 px-6 py-4 text-white">
        <button
          onClick={() => navigate(`/media/${item.id}`)}
          className="inline-flex items-center gap-1.5 text-xs font-bold text-white/70 transition hover:text-white"
        >
          <ArrowLeft size={14} /> Back to photo
        </button>
        <p className="min-w-0 truncate text-sm font-bold">{item.filename}</p>
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={() => setOps(NEUTRAL_EDIT_OPS)}
            disabled={isNeutral(ops)}
            className="inline-flex h-10 items-center gap-2 rounded-xl bg-white/10 px-4 text-sm font-bold text-white transition hover:bg-white/20 disabled:opacity-40"
          >
            <RotateCcw size={15} /> Reset
          </button>
          <Button icon={<Save size={15} />} onClick={() => setAskingSave(true)} disabled={isNeutral(ops)}>
            Save…
          </Button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <div className="relative flex min-h-0 min-w-0 flex-1 items-center justify-center overflow-hidden p-6">
          {/* The stage shrink-wraps the canvas, so the crop overlay's percentages
              line up with the pixels underneath whatever the window size. */}
          <div
            ref={stageRef}
            onMouseDown={beginCrop}
            onMouseMove={extendCrop}
            onMouseUp={endCrop}
            onMouseLeave={endCrop}
            className={cn("relative flex max-h-full max-w-full", tab === "crop" && "cursor-crosshair")}
          >
            <EditorCanvas
              image={image}
              ops={{ ...ops, crop: null }}
              className="block max-h-full max-w-full select-none rounded-lg object-contain shadow-2xl"
            />
            {ops.crop && ops.crop.width > 0 && (
              <div
                className="pointer-events-none absolute border-2 border-honey"
                style={{
                  left: `${ops.crop.x * 100}%`,
                  top: `${ops.crop.y * 100}%`,
                  width: `${ops.crop.width * 100}%`,
                  height: `${ops.crop.height * 100}%`,
                  // Everything outside the crop is dimmed rather than hidden, so
                  // you can still see what you are cutting away.
                  boxShadow: "0 0 0 9999px rgba(0,0,0,0.6)",
                }}
              />
            )}
          </div>
          {tab === "crop" && !ops.crop && (
            <p className="pointer-events-none absolute bottom-4 rounded-full bg-black/60 px-4 py-2 text-[11px] font-bold text-white/80">
              Drag across the photo to draw a crop
            </p>
          )}
        </div>

        <aside className="flex w-80 shrink-0 flex-col overflow-y-auto overflow-x-hidden border-l border-white/10 bg-panel">
          {/* A four-column grid rather than a flex row: equal cells that are free
              to shrink, so no label can push the panel wider than it is. */}
          <div className="sticky top-0 z-10 grid shrink-0 grid-cols-4 gap-1 border-b border-ink/[.08] bg-panel p-2.5">
            {TABS.map((entry) => (
              <button
                key={entry.key}
                onClick={() => setTab(entry.key)}
                className={cn(
                  "flex min-w-0 flex-col items-center gap-1 rounded-lg px-1 py-2 text-[10px] font-bold text-ink-muted transition",
                  tab === entry.key ? "bg-cream text-honey-deep" : "hover:bg-ink/5 hover:text-ink",
                )}
              >
                <entry.icon size={15} />
                <span className="w-full truncate text-center">{entry.label}</span>
              </button>
            ))}
          </div>

          {/* ---------------------------------------------------- filters -- */}
          {tab === "filters" && (
            <div className="flex flex-1 flex-col p-5">
              <p className="mb-3 text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
                Presets
              </p>
              <div className="grid grid-cols-2 gap-2">
                {FILTER_PRESETS.map((preset) => (
                  <button
                    key={preset.name}
                    onClick={() =>
                      setOps((prev) => ({
                        // A preset is a look, not a reset: framing is preserved.
                        ...prev,
                        brightness: 1,
                        contrast: 1,
                        saturation: 1,
                        grayscale: 0,
                        sepia: 0,
                        temperature: 0,
                        ...preset.ops,
                      }))
                    }
                    className="rounded-xl border border-ink/[.1] bg-canvas px-3 py-2.5 text-[11px] font-bold text-ink transition hover:border-honey/50 hover:bg-cream/40"
                  >
                    {preset.name}
                  </button>
                ))}
              </div>
              {/* Pushed to the foot of the panel so the tab does not trail off
                  into dead space under the buttons. */}
              <p className="mt-auto pt-6 text-[10px] leading-relaxed text-ink-muted">
                A preset only sets the colour sliders — your rotation and crop stay as they are.
                Fine-tune anything afterwards in the Adjust tab.
              </p>
            </div>
          )}

          {/* ----------------------------------------------------- adjust -- */}
          {tab === "adjust" && (
            <div className="space-y-6 p-5">
              <div>
                <p className="mb-2.5 text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
                  Orientation
                </p>
                <div className="grid grid-cols-4 gap-2">
                  {[
                    { icon: RotateCcw, label: "Rotate left", onClick: () => rotate(-90) },
                    { icon: RotateCw, label: "Rotate right", onClick: () => rotate(90) },
                    {
                      icon: FlipHorizontal,
                      label: "Flip horizontal",
                      onClick: () => setOps((p) => ({ ...p, flipHorizontal: !p.flipHorizontal })),
                      active: ops.flipHorizontal,
                    },
                    {
                      icon: FlipVertical,
                      label: "Flip vertical",
                      onClick: () => setOps((p) => ({ ...p, flipVertical: !p.flipVertical })),
                      active: ops.flipVertical,
                    },
                  ].map((button) => (
                    <button
                      key={button.label}
                      onClick={button.onClick}
                      aria-label={button.label}
                      title={button.label}
                      className={cn(
                        "grid h-10 place-items-center rounded-xl border transition",
                        button.active
                          ? "border-honey bg-cream/55 text-honey-deep"
                          : "border-ink/[.1] bg-canvas text-ink hover:border-honey/40",
                      )}
                    >
                      <button.icon size={15} />
                    </button>
                  ))}
                </div>
                {ops.rotation !== 0 && (
                  <p className="mt-2 text-[11px] text-ink-muted">Rotated {ops.rotation}°</p>
                )}
              </div>

              <div className="space-y-4">
                <p className="text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
                  Light and colour
                </p>
                {SLIDERS.map((slider) => {
                  const value = ops[slider.key] as number;
                  return (
                    <label key={slider.key} className="block">
                      <span className="mb-1.5 flex items-center justify-between text-[11px] font-bold">
                        <span className="text-ink">{slider.label}</span>
                        <span className="text-ink-muted">
                          {slider.neutral === 0 ? value.toFixed(2) : `${Math.round(value * 100)}%`}
                        </span>
                      </span>
                      <input
                        type="range"
                        min={slider.min}
                        max={slider.max}
                        step={0.01}
                        value={value}
                        onChange={(event) =>
                          setOps((prev) => ({ ...prev, [slider.key]: Number(event.target.value) }))
                        }
                        onDoubleClick={() =>
                          setOps((prev) => ({ ...prev, [slider.key]: slider.neutral }))
                        }
                        className="w-full accent-[var(--color-honey)]"
                      />
                    </label>
                  );
                })}
                <p className="text-[10px] text-ink-muted">
                  Double-click a slider to send it back to neutral.
                </p>
              </div>
            </div>
          )}

          {/* ------------------------------------------------------- crop -- */}
          {tab === "crop" && (
            <div className="space-y-5 p-5">
              <div>
                <p className="mb-2.5 text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
                  Aspect ratio
                </p>
                <div className="grid grid-cols-3 gap-2">
                  {CROP_PRESETS.map((presetOption) => (
                    <button
                      key={presetOption.label}
                      onClick={() => applyPreset(presetOption.ratio)}
                      className="rounded-xl border border-ink/[.1] bg-canvas px-2 py-2 text-[11px] font-bold text-ink transition hover:border-honey/40"
                    >
                      {presetOption.label}
                    </button>
                  ))}
                </div>
              </div>

              {ops.crop ? (
                <div className="space-y-3.5">
                  <p className="text-[10px] font-extrabold uppercase tracking-[.13em] text-ink-muted">
                    Fine tune
                  </p>
                  {(
                    [
                      ["x", "Left"],
                      ["y", "Top"],
                      ["width", "Width"],
                      ["height", "Height"],
                    ] as const
                  ).map(([key, label]) => (
                    <label key={key} className="block">
                      <span className="mb-1 flex items-center justify-between text-[11px] font-bold">
                        <span className="text-ink">{label}</span>
                        <span className="text-ink-muted">
                          {Math.round((ops.crop?.[key] ?? 0) * 100)}%
                        </span>
                      </span>
                      <input
                        type="range"
                        min={0}
                        max={1}
                        step={0.01}
                        value={ops.crop?.[key] ?? 0}
                        onChange={(event) =>
                          setOps((prev) =>
                            prev.crop
                              ? { ...prev, crop: { ...prev.crop, [key]: Number(event.target.value) } }
                              : prev,
                          )
                        }
                        className="w-full accent-[var(--color-honey)]"
                      />
                    </label>
                  ))}
                  <button
                    onClick={() => setOps((prev) => ({ ...prev, crop: null }))}
                    className="text-[11px] font-bold text-honey-deep underline-offset-2 hover:underline"
                  >
                    Clear the crop
                  </button>
                </div>
              ) : (
                <p className="rounded-2xl border border-dashed border-ink/[.14] p-4 text-[11px] leading-relaxed text-ink-muted">
                  Drag across the photo to draw a crop, or pick a ratio above. The dimmed area is
                  what gets cut away.
                </p>
              )}
            </div>
          )}

          {/* --------------------------------------------------- metadata -- */}
          {tab === "metadata" && (
            <div className="space-y-4 p-5">
              <label className="block">
                <span className="mb-1.5 block text-[11px] font-bold text-ink">Title</span>
                <input
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                  placeholder="Untitled"
                  className="search-input !pl-3.5"
                />
              </label>

              <label className="block">
                <span className="mb-1.5 block text-[11px] font-bold text-ink">Description</span>
                <textarea
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  rows={4}
                  placeholder="What is happening in this photo?"
                  className="w-full rounded-xl border border-ink/[.12] bg-canvas p-3 text-xs text-ink outline-none focus:border-honey/65"
                />
              </label>

              <label className="block">
                <span className="mb-1.5 block text-[11px] font-bold text-ink">Capture date</span>
                <input
                  type="date"
                  value={takenAtOverride}
                  onChange={(event) => setTakenAtOverride(event.target.value)}
                  className="search-input !pl-3.5"
                />
                <span className="mt-1.5 block text-[10px] text-ink-muted">
                  {item.takenAt
                    ? `Read from the photo: ${formatDate(item.takenAt)}`
                    : "This photo carries no capture date."}
                  {" Leave empty to keep it."}
                </span>
              </label>

              <Button
                className="w-full"
                icon={savingMetadata ? undefined : <Check size={15} />}
                onClick={saveMetadata}
                disabled={savingMetadata}
              >
                {savingMetadata ? "Saving…" : "Save details"}
              </Button>

              <p className="flex items-start gap-2 rounded-2xl border border-ink/[.08] bg-canvas p-3 text-[10px] leading-relaxed text-ink-muted">
                <Info size={13} className="mt-0.5 shrink-0" />
                <span>
                  These are stored by Hive, not written into the photo file — so nothing is
                  re-encoded and your original stays byte-for-byte intact.
                </span>
              </p>
            </div>
          )}
        </aside>
      </div>

      {askingSave && (
        <SaveEditDialog
          filename={item.filename}
          saving={saving}
          onCancel={() => setAskingSave(false)}
          onConfirm={save}
        />
      )}
    </div>
  );
}
