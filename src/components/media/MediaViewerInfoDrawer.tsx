import {
  Calendar,
  Camera,
  Copy,
  FolderOpen,
  HardDrive,
  Info,
  MapPin,
  Maximize,
  X,
} from "lucide-react";
import type { MediaItem } from "@/types/media";

interface MediaViewerInfoDrawerProps {
  item: MediaItem;
  isOpen: boolean;
  onClose: () => void;
  onCopyPath: () => void;
  onOpenFolder: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Bytes";
  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function calculateMegapixels(width: number | null, height: number | null): string | null {
  if (!width || !height) return null;
  const mp = (width * height) / 1000000;
  return `${mp.toFixed(1)} MP`;
}

export function MediaViewerInfoDrawer({
  item,
  isOpen,
  onClose,
  onCopyPath,
  onOpenFolder,
}: MediaViewerInfoDrawerProps) {
  if (!isOpen) return null;

  const mp = calculateMegapixels(item.width, item.height);

  return (
    <aside className="absolute right-0 top-0 bottom-0 z-40 w-80 border-l border-border/40 bg-panel/95 dark:bg-panel/95 p-5 shadow-2xl backdrop-blur-xl animate-in slide-in-from-right duration-200 flex flex-col gap-6 text-ink overflow-y-auto">
      {/* Drawer Header */}
      <div className="flex items-center justify-between border-b border-border/40 pb-4">
        <div className="flex items-center gap-2 font-bold text-sm">
          <Info size={16} className="text-honey" />
          <span>Info & Metadata</span>
        </div>
        <button
          onClick={onClose}
          className="rounded-lg p-1.5 text-ink-muted hover:bg-honey/15 hover:text-honey transition"
          aria-label="Close drawer"
        >
          <X size={16} />
        </button>
      </div>

      {/* File Information Section */}
      <div className="flex flex-col gap-3">
        <h4 className="eyebrow">File Info</h4>
        
        <div className="rounded-xl border border-border/30 bg-shell/50 p-3.5 space-y-3 text-xs">
          <div>
            <div className="text-[10px] uppercase font-bold text-ink-muted mb-0.5">Filename</div>
            <div className="font-semibold text-ink break-all">{item.filename}</div>
          </div>

          <div>
            <div className="text-[10px] uppercase font-bold text-ink-muted mb-0.5">Location</div>
            <div className="text-ink-soft break-all font-mono text-[11px] mb-2">{item.path}</div>
            <div className="flex gap-2">
              <button
                onClick={onCopyPath}
                className="flex items-center gap-1.5 rounded-lg border border-border/40 bg-panel px-2.5 py-1 text-[11px] font-medium text-ink-soft hover:text-honey hover:border-honey/40 transition"
              >
                <Copy size={12} />
                <span>Copy Path</span>
              </button>
              <button
                onClick={onOpenFolder}
                className="flex items-center gap-1.5 rounded-lg border border-border/40 bg-panel px-2.5 py-1 text-[11px] font-medium text-ink-soft hover:text-honey hover:border-honey/40 transition"
              >
                <FolderOpen size={12} />
                <span>Reveal</span>
              </button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2 pt-1 border-t border-border/30 text-[11px]">
            <div>
              <div className="flex items-center gap-1 text-ink-muted">
                <HardDrive size={11} />
                <span>Size</span>
              </div>
              <div className="font-semibold text-ink mt-0.5">{formatBytes(item.size)}</div>
            </div>
            <div>
              <div className="flex items-center gap-1 text-ink-muted">
                <Maximize size={11} />
                <span>Dimensions</span>
              </div>
              <div className="font-semibold text-ink mt-0.5">
                {item.width && item.height ? `${item.width} × ${item.height}` : "Unknown"}
                {mp ? ` (${mp})` : ""}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Camera EXIF Details */}
      {item.exif ? (
        <div className="flex flex-col gap-3">
          <h4 className="eyebrow">Camera Specs</h4>
          <div className="rounded-xl border border-border/30 bg-shell/50 p-3.5 space-y-3 text-xs">
            {(item.exif.cameraMake || item.exif.cameraModel) && (
              <div className="flex items-start gap-2.5">
                <Camera size={15} className="text-honey shrink-0 mt-0.5" />
                <div>
                  <div className="text-[10px] uppercase font-bold text-ink-muted">Camera</div>
                  <div className="font-semibold text-ink">
                    {[item.exif.cameraMake, item.exif.cameraModel].filter(Boolean).join(" ")}
                  </div>
                </div>
              </div>
            )}

            {item.exif.lens && (
              <div>
                <div className="text-[10px] uppercase font-bold text-ink-muted mb-0.5">Lens</div>
                <div className="font-medium text-ink-soft">{item.exif.lens}</div>
              </div>
            )}

            <div className="grid grid-cols-2 gap-2 pt-2 border-t border-border/30 text-[11px]">
              {item.exif.focalLength && (
                <div>
                  <span className="text-ink-muted">Focal Length: </span>
                  <span className="font-semibold text-ink">{item.exif.focalLength} mm</span>
                </div>
              )}
              {item.exif.fNumber && (
                <div>
                  <span className="text-ink-muted">Aperture: </span>
                  <span className="font-semibold text-ink">f/{item.exif.fNumber}</span>
                </div>
              )}
              {item.exif.exposureTime && (
                <div>
                  <span className="text-ink-muted">Shutter: </span>
                  <span className="font-semibold text-ink">{item.exif.exposureTime} s</span>
                </div>
              )}
              {item.exif.iso && (
                <div>
                  <span className="text-ink-muted">ISO: </span>
                  <span className="font-semibold text-ink">{item.exif.iso}</span>
                </div>
              )}
            </div>

            {(item.exif.gpsLat !== null && item.exif.gpsLon !== null) && (
              <div className="pt-2 border-t border-border/30">
                <div className="flex items-center gap-1 text-[10px] uppercase font-bold text-ink-muted mb-1">
                  <MapPin size={11} className="text-honey" />
                  <span>GPS Location</span>
                </div>
                <div className="font-mono text-[11px] text-ink-soft">
                  {item.exif.gpsLat?.toFixed(5)}, {item.exif.gpsLon?.toFixed(5)}
                </div>
              </div>
            )}
          </div>
        </div>
      ) : null}

      {/* Date & Time Section */}
      <div className="flex flex-col gap-3">
        <h4 className="eyebrow">Dates & History</h4>
        <div className="rounded-xl border border-border/30 bg-shell/50 p-3.5 space-y-2 text-xs">
          <div className="flex items-center gap-2">
            <Calendar size={13} className="text-honey shrink-0" />
            <div>
              <span className="text-ink-muted text-[11px]">Taken: </span>
              <span className="font-semibold text-ink text-[11px]">
                {item.takenAt ?? item.createdAt}
              </span>
            </div>
          </div>

          <div className="text-[11px] text-ink-muted pt-1 border-t border-border/30">
            <div>Indexed: {new Date(item.indexedAt).toLocaleString()}</div>
          </div>
        </div>
      </div>
    </aside>
  );
}
