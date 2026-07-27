import { useEffect, useRef } from "react";
import {
  Copy,
  FolderOpen,
  Heart,
  Info,
  Maximize2,
  RotateCw,
  Trash2,
  ZoomIn,
  ZoomOut,
  ImageIcon,
} from "lucide-react";
import type { MediaItem } from "@/types/media";

interface MediaViewerContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  item: MediaItem;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onRotate: () => void;
  onToggleFavorite: () => void;
  onToggleInfo: () => void;
  onTrash: () => void;
  onCopyPath: () => void;
  onCopyImage: () => void;
  onOpenFolder: () => void;
}

export function MediaViewerContextMenu({
  x,
  y,
  onClose,
  item,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onRotate,
  onToggleFavorite,
  onToggleInfo,
  onTrash,
  onCopyPath,
  onCopyImage,
  onOpenFolder,
}: MediaViewerContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  // Adjust coordinates so context menu doesn't flow off-screen
  const width = 230;
  const height = 360;
  const adjustedX = Math.min(x, window.innerWidth - width - 12);
  const adjustedY = Math.min(y, window.innerHeight - height - 12);

  return (
    <div
      ref={menuRef}
      style={{ left: adjustedX, top: adjustedY }}
      className="fixed z-50 w-56 rounded-2xl border border-white/20 dark:border-white/10 bg-panel/90 p-1.5 shadow-2xl backdrop-blur-xl animate-in fade-in zoom-in-95 duration-100 text-ink text-xs select-none"
    >
      <div className="px-2.5 py-1.5 border-b border-ink/[.4] text-[11px] font-medium text-ink-muted truncate">
        {item.filename}
      </div>

      <div className="py-1">
        <button
          onClick={() => {
            onCopyImage();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <ImageIcon size={14} className="text-honey" />
          <span>Copy Image</span>
        </button>

        <button
          onClick={() => {
            onCopyPath();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <Copy size={14} />
          <span>Copy File Path</span>
        </button>

        <button
          onClick={() => {
            onOpenFolder();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <FolderOpen size={14} />
          <span>Show in File Explorer</span>
        </button>
      </div>

      <div className="my-1 border-t border-ink/[.4]" />

      <div className="py-1">
        <button
          onClick={() => {
            onZoomIn();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <ZoomIn size={14} />
          <span>Zoom In (+25%)</span>
        </button>

        <button
          onClick={() => {
            onZoomOut();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <ZoomOut size={14} />
          <span>Zoom Out (-25%)</span>
        </button>

        <button
          onClick={() => {
            onResetZoom();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <Maximize2 size={14} />
          <span>Fit to Screen</span>
        </button>

        <button
          onClick={() => {
            onRotate();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <RotateCw size={14} />
          <span>Rotate Right (90°)</span>
        </button>
      </div>

      <div className="my-1 border-t border-ink/[.4]" />

      <div className="py-1">
        <button
          onClick={() => {
            onToggleFavorite();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <Heart
            size={14}
            className={item.isFavorite ? "fill-honey text-honey" : ""}
          />
          <span>{item.isFavorite ? "Remove from Favorites" : "Add to Favorites"}</span>
        </button>

        <button
          onClick={() => {
            onToggleInfo();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <Info size={14} />
          <span>View Info & EXIF</span>
        </button>
      </div>

      <div className="my-1 border-t border-ink/[.4]" />

      <div className="py-1">
        <button
          onClick={() => {
            onTrash();
            onClose();
          }}
          className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-1.5 font-medium hover:bg-honey/15 hover:text-honey transition"
        >
          <Trash2 size={14} />
          <span>Move to Trash</span>
        </button>
      </div>
    </div>
  );
}
