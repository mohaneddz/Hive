import { useCallback, useEffect, useMemo, useState } from "react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, FileType2, Loader2, Minimize2, PenLine } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import {
  batchRename,
  compressImages,
  convertImages,
  getMediaPage,
  isTauri,
  listAlbums,
  listFolders,
  previewBatchRename,
} from "@/lib/tauri";
import type {
  Album,
  BatchReport,
  ConvertFormat,
  Folder,
  MediaItem,
  RenamePreview,
} from "@/types/media";
import { cn } from "@/utils/cn";
import { formatBytes, formatCount } from "@/utils/format";

type Operation = "rename" | "compress" | "convert";

const OPERATIONS: { key: Operation; label: string; icon: typeof PenLine }[] = [
  { key: "rename", label: "Rename", icon: PenLine },
  { key: "compress", label: "Compress", icon: Minimize2 },
  { key: "convert", label: "Convert", icon: FileType2 },
];

const QUALITY_PRESETS = [
  { label: "Small", quality: 60, maxDimension: 2000 },
  { label: "Balanced", quality: 80, maxDimension: 3000 },
  { label: "High", quality: 92, maxDimension: 0 },
];

const FORMATS: { value: ConvertFormat; label: string; hint: string }[] = [
  { value: "jpg", label: "JPEG", hint: "Smallest, no transparency" },
  { value: "png", label: "PNG", hint: "Lossless, keeps transparency" },
  { value: "webp", label: "WebP", hint: "Modern, small and lossless" },
];

function ReportBanner({ report }: { report: BatchReport }) {
  const saved = report.bytesBefore - report.bytesAfter;
  return (
    <div className="mt-5 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
      <p>
        {report.processed} done · {report.skipped} skipped · {report.failed} failed
        {report.bytesBefore > 0 &&
          ` · ${formatBytes(report.bytesBefore)} → ${formatBytes(report.bytesAfter)} (${
            saved >= 0 ? "saved" : "grew by"
          } ${formatBytes(Math.abs(saved))})`}
      </p>
      {report.destination && <p className="mt-1 font-normal">Written to {report.destination}</p>}
      {report.firstError && (
        <p className="mt-1 font-normal text-red-600">First error: {report.firstError}</p>
      )}
    </div>
  );
}

export function BatchPanel() {
  const [albums, setAlbums] = useState<Album[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [source, setSource] = useState("library");
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loadingSource, setLoadingSource] = useState(false);

  const [operation, setOperation] = useState<Operation>("rename");
  const [pattern, setPattern] = useState("{date}-{name}");
  const [startIndex, setStartIndex] = useState(1);
  const [previews, setPreviews] = useState<RenamePreview[]>([]);
  const [preset, setPreset] = useState(1);
  const [format, setFormat] = useState<ConvertFormat>("webp");
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<BatchReport | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void listAlbums().then(setAlbums);
    void listFolders().then(setFolders);
  }, []);

  const loadSource = useCallback(async () => {
    if (!isTauri()) return;
    setLoadingSource(true);
    setReport(null);
    try {
      const page = await getMediaPage({
        limit: 5000,
        offset: 0,
        favoritesOnly: source === "favorites" || undefined,
        albumId: source.startsWith("album:") ? source.slice(6) : undefined,
        folderId: source.startsWith("folder:") ? source.slice(7) : undefined,
        scope: source.startsWith("album:") ? "all" : undefined,
        sort: "taken",
      });
      setItems(page.items);
    } finally {
      setLoadingSource(false);
    }
  }, [source]);

  useEffect(() => {
    void loadSource();
  }, [loadSource]);

  const mediaIds = useMemo(() => items.map((item) => item.id), [items]);
  // Only images can be re-encoded; a video in the selection is simply passed over.
  const imageIds = useMemo(
    () => items.filter((item) => item.mediaType === "image").map((item) => item.id),
    [items],
  );

  useEffect(() => {
    if (operation !== "rename" || mediaIds.length === 0) {
      setPreviews([]);
      return;
    }
    let cancelled = false;
    void previewBatchRename(mediaIds.slice(0, 200), pattern, startIndex).then((result) => {
      if (!cancelled) setPreviews(result);
    });
    return () => {
      cancelled = true;
    };
  }, [operation, mediaIds, pattern, startIndex]);

  const conflicts = previews.filter((preview) => preview.conflict).length;

  const pickDestination = async () => {
    const destination = await open({ directory: true, multiple: false, title: "Write results to…" });
    return typeof destination === "string" ? destination : null;
  };

  const runRename = async () => {
    const confirmed = await confirm(
      `Rename ${formatCount(mediaIds.length, "file")} on disk? Hive follows them in the index, and ${conflicts} conflicting name${conflicts === 1 ? "" : "s"} will be skipped.`,
      { title: "Batch rename", kind: "warning" },
    );
    if (!confirmed) return;

    setBusy(true);
    try {
      setReport(await batchRename(mediaIds, pattern, startIndex));
      await loadSource();
    } finally {
      setBusy(false);
    }
  };

  const runCompress = async () => {
    const destination = await pickDestination();
    if (!destination) return;

    setBusy(true);
    try {
      const { quality, maxDimension } = QUALITY_PRESETS[preset];
      setReport(
        await compressImages(imageIds, quality, maxDimension === 0 ? undefined : maxDimension, destination),
      );
    } finally {
      setBusy(false);
    }
  };

  const runConvert = async () => {
    const destination = await pickDestination();
    if (!destination) return;

    setBusy(true);
    try {
      setReport(await convertImages(imageIds, format, destination));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Card className="mt-6 p-6">
        <h2 className="text-base font-extrabold text-ink">1. Choose what to work on</h2>
        <p className="mt-0.5 text-xs text-ink-muted">
          Every batch operation runs on this selection.
        </p>
        <div className="mt-4 flex flex-wrap items-center gap-3">
          <select
            value={source}
            onChange={(event) => setSource(event.target.value)}
            className="h-10 min-w-[240px] rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink outline-none"
          >
            <option value="library">Whole library</option>
            <option value="favorites">Favorites only</option>
            {folders.length > 0 && (
              <optgroup label="Watched folders">
                {folders.map((folder) => (
                  <option key={folder.id} value={`folder:${folder.id}`}>
                    {folder.name}
                  </option>
                ))}
              </optgroup>
            )}
            {albums.length > 0 && (
              <optgroup label="Albums">
                {albums.map((album) => (
                  <option key={album.id} value={`album:${album.id}`}>
                    {album.name}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
          <span className="text-xs font-bold text-ink-muted">
            {loadingSource
              ? "Loading…"
              : `${formatCount(items.length, "item")} · ${imageIds.length} image${imageIds.length === 1 ? "" : "s"}`}
          </span>
        </div>
      </Card>

      <Card className="mt-4 p-6">
        <h2 className="text-base font-extrabold text-ink">2. Pick an operation</h2>
        <div className="mt-4 flex items-center gap-1 rounded-xl border border-ink/[.12] bg-canvas p-1">
          {OPERATIONS.map((entry) => (
            <button
              key={entry.key}
              onClick={() => {
                setOperation(entry.key);
                setReport(null);
              }}
              className={cn(
                "inline-flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-bold text-ink-muted transition",
                operation === entry.key && "bg-cream text-honey-deep",
              )}
            >
              <entry.icon size={14} />
              {entry.label}
            </button>
          ))}
        </div>

        {/* ------------------------------------------------------- rename -- */}
        {operation === "rename" && (
          <>
            <div className="mt-5 flex flex-wrap items-end gap-3">
              <label className="min-w-[260px] flex-1">
                <span className="mb-1.5 block text-[11px] font-bold text-ink-muted">Pattern</span>
                <input
                  value={pattern}
                  onChange={(event) => setPattern(event.target.value)}
                  className="search-input !pl-3.5"
                  placeholder="{date}-{name}"
                />
              </label>
              <label className="w-28">
                <span className="mb-1.5 block text-[11px] font-bold text-ink-muted">Start at</span>
                <input
                  type="number"
                  min={0}
                  value={startIndex}
                  onChange={(event) => setStartIndex(Number(event.target.value) || 0)}
                  className="search-input !pl-3.5"
                />
              </label>
            </div>
            <p className="mt-2.5 text-[11px] text-ink-muted">
              <code className="rounded bg-shell px-1.5 py-0.5 font-bold">{"{name}"}</code> original name ·{" "}
              <code className="rounded bg-shell px-1.5 py-0.5 font-bold">{"{n}"}</code> number ·{" "}
              <code className="rounded bg-shell px-1.5 py-0.5 font-bold">{"{date}"}</code> capture day.
              The extension is always kept.
            </p>

            {conflicts > 0 && (
              <p className="mt-4 flex items-center gap-2 rounded-2xl border border-red-500/30 bg-red-500/5 px-4 py-3 text-xs font-semibold text-red-600">
                <AlertTriangle size={14} />
                {formatCount(conflicts, "name")} would collide and will be skipped.
              </p>
            )}

            {previews.length > 0 && (
              <div className="mt-4 max-h-64 overflow-y-auto rounded-2xl border border-ink/[.08]">
                {previews.slice(0, 50).map((preview) => (
                  <div
                    key={preview.mediaId}
                    className={cn(
                      "flex items-center gap-3 border-b border-ink/[.05] px-4 py-2 text-[11px] last:border-b-0",
                      preview.conflict && "bg-red-500/5",
                    )}
                  >
                    <span className="min-w-0 flex-1 truncate text-ink-muted">{preview.from}</span>
                    <span className="shrink-0 text-ink-muted">→</span>
                    <span
                      className={cn(
                        "min-w-0 flex-1 truncate font-bold",
                        preview.conflict ? "text-red-600" : "text-ink",
                      )}
                    >
                      {preview.to}
                    </span>
                  </div>
                ))}
                {previews.length > 50 && (
                  <p className="px-4 py-2 text-[11px] text-ink-muted">
                    …and {previews.length - 50} more
                  </p>
                )}
              </div>
            )}

            <Button
              className="mt-5"
              icon={busy ? <Loader2 size={15} className="animate-spin" /> : <PenLine size={15} />}
              onClick={runRename}
              disabled={busy || mediaIds.length === 0}
            >
              Rename {formatCount(mediaIds.length, "file")}
            </Button>
            <p className="mt-2.5 text-[11px] text-ink-muted">
              This is the one batch operation that changes files on your disk. Hive updates the index
              first, so a renamed photo keeps its favorites, albums and place.
            </p>
          </>
        )}

        {/* ----------------------------------------------------- compress -- */}
        {operation === "compress" && (
          <>
            <div className="mt-5 grid grid-cols-3 gap-3">
              {QUALITY_PRESETS.map((entry, index) => (
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
                  <p className="mt-0.5 text-[11px] text-ink-muted">
                    Quality {entry.quality}
                    {entry.maxDimension > 0 ? ` · max ${entry.maxDimension}px` : " · full size"}
                  </p>
                </button>
              ))}
            </div>
            <Button
              className="mt-5"
              icon={busy ? <Loader2 size={15} className="animate-spin" /> : <Minimize2 size={15} />}
              onClick={runCompress}
              disabled={busy || imageIds.length === 0}
            >
              Compress {formatCount(imageIds.length, "image")}…
            </Button>
            <p className="mt-2.5 text-[11px] text-ink-muted">
              Compressed copies are written to a folder you choose. Your originals are never opened
              for writing. Videos in the selection are skipped.
            </p>
          </>
        )}

        {/* ------------------------------------------------------ convert -- */}
        {operation === "convert" && (
          <>
            <div className="mt-5 grid grid-cols-3 gap-3">
              {FORMATS.map((entry) => (
                <button
                  key={entry.value}
                  onClick={() => setFormat(entry.value)}
                  className={cn(
                    "rounded-2xl border p-4 text-left transition",
                    format === entry.value
                      ? "border-honey bg-cream/55"
                      : "border-ink/[.08] bg-canvas hover:border-honey/40",
                  )}
                >
                  <p className="text-sm font-extrabold text-ink">{entry.label}</p>
                  <p className="mt-0.5 text-[11px] text-ink-muted">{entry.hint}</p>
                </button>
              ))}
            </div>
            <Button
              className="mt-5"
              icon={busy ? <Loader2 size={15} className="animate-spin" /> : <FileType2 size={15} />}
              onClick={runConvert}
              disabled={busy || imageIds.length === 0}
            >
              Convert {formatCount(imageIds.length, "image")}…
            </Button>
            <p className="mt-2.5 text-[11px] text-ink-muted">
              Converted copies go to a folder you choose. Converting to JPEG flattens transparency,
              since JPEG has no alpha channel.
            </p>
          </>
        )}

        {report && <ReportBanner report={report} />}
      </Card>
    </>
  );
}
