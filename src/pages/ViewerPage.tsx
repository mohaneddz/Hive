import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  ChevronLeft,
  ChevronRight,
  Copy,
  Heart,
  Info,
  Maximize2,
  Minimize2,
  Moon,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  Sun,
  Trash2,
  X,
  ZoomIn,
  ZoomOut,
  Grid,
  Check,
  Sparkles,
} from "lucide-react";

import { MediaThumb } from "@/components/media/MediaThumb";
import { MediaViewerContextMenu } from "@/components/media/MediaViewerContextMenu";
import { MediaViewerFilmstrip } from "@/components/media/MediaViewerFilmstrip";
import { MediaViewerInfoDrawer } from "@/components/media/MediaViewerInfoDrawer";
import { getMediaDetail, getMediaPage, readMediaUrl, setFavorite, setTrashed } from "@/lib/tauri";
import type { MediaItem } from "@/types/media";
import { cn } from "@/utils/cn";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

const NAV_WINDOW = 500;
type StageBgMode = "adaptive" | "dark" | "checkerboard";

export function ViewerPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const [neighbors, setNeighbors] = useState<MediaItem[]>([]);
  const [item, setItem] = useState<MediaItem | null>(null);

  // Viewport & Stage states
  const [stageBg, setStageBg] = useState<StageBgMode>("adaptive");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [rotation, setRotation] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });

  // UI Drawer & Menu states
  const [isInfoOpen, setIsInfoOpen] = useState(false);
  const [isPlayingSlideshow, setIsPlayingSlideshow] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  // Right-click context menu
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);

  // Toast notification timer
  const showToast = useCallback((msg: string) => {
    setToastMessage(msg);
    setTimeout(() => setToastMessage(null), 2400);
  }, []);

  // Fetch list of neighbor items for navigation
  useEffect(() => {
    let cancelled = false;
    getMediaPage({ limit: NAV_WINDOW, offset: 0 }).then((page) => {
      if (!cancelled) setNeighbors(page.items);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Fetch current media item details
  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    
    // Reset canvas transformation when changing images
    setZoom(1);
    setPan({ x: 0, y: 0 });
    setRotation(0);
    setContextMenu(null);

    const fromWindow = neighbors.find((entry) => entry.id === id);
    if (fromWindow) {
      setItem(fromWindow);
      return;
    }
    getMediaDetail(id).then((detail) => {
      if (!cancelled) setItem(detail);
    });
    return () => {
      cancelled = true;
    };
  }, [id, neighbors]);

  const index = neighbors.findIndex((entry) => entry.id === id);
  const prevItem = index > 0 ? neighbors[index - 1] : null;
  const nextItem = index >= 0 && index < neighbors.length - 1 ? neighbors[index + 1] : null;

  const goTo = useCallback(
    (target: MediaItem | null) => {
      if (target) navigate(`/media/${target.id}`);
    },
    [navigate],
  );

  // Auto-advance slideshow
  useEffect(() => {
    if (!isPlayingSlideshow) return;
    const interval = setInterval(() => {
      if (nextItem) {
        goTo(nextItem);
      } else if (neighbors.length > 0) {
        goTo(neighbors[0]); // Loop back to start
      }
    }, 3500);
    return () => clearInterval(interval);
  }, [isPlayingSlideshow, nextItem, neighbors, goTo]);

  // Keyboard Shortcuts
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") goTo(prevItem);
      if (event.key === "ArrowRight") goTo(nextItem);
      if (event.key === "Escape") {
        if (isInfoOpen) setIsInfoOpen(false);
        else if (contextMenu) setContextMenu(null);
        else navigate("/");
      }
      if (event.key === "+" || event.key === "=") handleZoomIn();
      if (event.key === "-") handleZoomOut();
      if (event.key === "0") handleResetZoom();
      if (event.key.toLowerCase() === "r") handleRotateCw();
      if (event.key.toLowerCase() === "i") setIsInfoOpen((prev) => !prev);
      if (event.key === " ") {
        event.preventDefault();
        setIsPlayingSlideshow((prev) => !prev);
      }
      if (event.key.toLowerCase() === "f") toggleFullscreen();
      if (event.key === "Delete") trash();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goTo, prevItem, nextItem, navigate, isInfoOpen, contextMenu]);

  // Fullscreen toggle
  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(() => {});
      setIsFullscreen(true);
    } else {
      document.exitFullscreen().catch(() => {});
      setIsFullscreen(false);
    }
  };

  // Zoom / Pan handlers
  const handleZoomIn = () => {
    setZoom((prev) => Math.min(prev + 0.25, 4.0));
  };

  const handleZoomOut = () => {
    setZoom((prev) => {
      const next = Math.max(prev - 0.25, 0.5);
      if (next <= 1) setPan({ x: 0, y: 0 });
      return next;
    });
  };

  const handleResetZoom = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  const handleDoubleTap = () => {
    if (zoom === 1) {
      setZoom(2);
    } else {
      handleResetZoom();
    }
  };

  const handleRotateCw = () => {
    setRotation((prev) => (prev + 90) % 360);
  };

  const handleRotateCcw = () => {
    setRotation((prev) => (prev - 90 + 360) % 360);
  };

  // Mouse wheel zoom
  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    if (e.deltaY < 0) {
      handleZoomIn();
    } else {
      handleZoomOut();
    }
  };

  // Mouse drag panning
  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return; // Only left click for dragging
    if (zoom > 1) {
      setIsDragging(true);
      setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y });
    }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (isDragging && zoom > 1) {
      setPan({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      });
    }
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  // Context menu trigger
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  // Actions
  const toggleFavorite = async () => {
    if (!item) return;
    const next = !item.isFavorite;
    await setFavorite(item.id, next);
    setItem({ ...item, isFavorite: next });
    showToast(next ? "Added to Favorites" : "Removed from Favorites");
  };

  const trash = async () => {
    if (!item) return;
    await setTrashed(item.id, true);
    showToast("Moved to Trash");
    navigate("/");
  };

  const handleCopyPath = async () => {
    if (!item) return;
    try {
      await writeText(item.path);
      showToast("File path copied to clipboard");
    } catch {
      await navigator.clipboard.writeText(item.path);
      showToast("File path copied to clipboard");
    }
  };

  const handleCopyImage = async () => {
    if (!item) return;
    try {
      const url = await readMediaUrl(item.id, "original");
      const resp = await fetch(url);
      const blob = await resp.blob();
      await navigator.clipboard.write([
        new ClipboardItem({ [blob.type || "image/png"]: blob }),
      ]);
      showToast("Image copied to clipboard");
    } catch (err) {
      showToast("Copied file path to clipboard");
      await handleCopyPath();
    }
  };

  const handleOpenFolder = async () => {
    if (!item) return;
    try {
      await revealItemInDir(item.path);
    } catch {
      await openPath(item.path);
    }
  };

  const cycleStageBg = () => {
    if (stageBg === "adaptive") setStageBg("dark");
    else if (stageBg === "dark") setStageBg("checkerboard");
    else setStageBg("adaptive");
  };

  if (!item) {
    return (
      <div className="fixed inset-0 z-50 grid place-items-center bg-canvas text-sm font-medium text-ink-muted">
        <div className="flex flex-col items-center gap-3">
          <div className="size-8 rounded-full border-2 border-honey border-t-transparent animate-spin" />
          <span>Loading image…</span>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={cn(
        "fixed inset-0 z-50 flex flex-col overflow-hidden select-none transition-colors duration-200",
        stageBg === "adaptive" && "bg-canvas/95 text-ink backdrop-blur-2xl",
        stageBg === "dark" && "bg-[#09090b] text-white backdrop-blur-2xl",
        stageBg === "checkerboard" && "bg-canvas text-ink"
      )}
      style={
        stageBg === "checkerboard"
          ? {
              backgroundImage:
                "radial-gradient(#888 1px, transparent 0), radial-gradient(#888 1px, #e5e5e5 0)",
              backgroundPosition: "0 0, 10px 10px",
              backgroundSize: "20px 20px",
            }
          : undefined
      }
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      {/* Toast Notification Banner */}
      {toastMessage && (
        <div className="absolute top-16 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 rounded-full border border-honey/40 bg-panel px-4 py-2 text-xs font-semibold text-ink shadow-xl backdrop-blur-md animate-in fade-in slide-in-from-top-2 duration-150">
          <Check size={14} className="text-honey" />
          <span>{toastMessage}</span>
        </div>
      )}

      {/* Top Floating Glass Bar */}
      <header className="relative z-30 flex h-16 items-center justify-between px-6 border-b border-border/20 bg-panel/40 dark:bg-panel/40 backdrop-blur-md">
        {/* Left: Back & Filename */}
        <div className="flex items-center gap-3 min-w-0">
          <button
            onClick={() => navigate("/")}
            className="flex items-center gap-2 rounded-xl border border-border/30 bg-panel/60 px-3 py-1.5 text-xs font-semibold hover:border-honey/50 hover:bg-honey/15 hover:text-honey transition"
            aria-label="Back to Gallery"
          >
            <X size={15} />
            <span>Close</span>
          </button>

          <div className="min-w-0 border-l border-border/30 pl-3">
            <h1 className="text-xs font-bold truncate max-w-[280px] sm:max-w-md">
              {item.filename}
            </h1>
            <p className="text-[11px] text-ink-muted truncate">
              {item.takenAt ?? item.createdAt}{" "}
              {item.width && item.height ? `· ${item.width} × ${item.height}` : ""}
            </p>
          </div>
        </div>

        {/* Center: Status indicators */}
        {isPlayingSlideshow && (
          <div className="hidden sm:flex items-center gap-1.5 rounded-full border border-honey/40 bg-honey/10 px-3 py-1 text-[11px] font-bold text-honey animate-pulse">
            <Sparkles size={13} />
            <span>Slideshow Playing</span>
          </div>
        )}

        {/* Right: Actions */}
        <div className="flex items-center gap-1.5">
          <button
            onClick={toggleFavorite}
            className="rounded-xl border border-border/30 bg-panel/60 p-2 text-ink-soft hover:text-honey hover:border-honey/40 transition"
            title="Toggle Favorite"
            aria-label="Toggle favorite"
          >
            <Heart
              size={17}
              className={cn(item.isFavorite && "fill-honey text-honey")}
            />
          </button>

          <button
            onClick={handleCopyPath}
            className="rounded-xl border border-border/30 bg-panel/60 p-2 text-ink-soft hover:text-honey hover:border-honey/40 transition"
            title="Copy Path"
            aria-label="Copy Path"
          >
            <Copy size={17} />
          </button>

          <button
            onClick={cycleStageBg}
            className="rounded-xl border border-border/30 bg-panel/60 p-2 text-ink-soft hover:text-honey hover:border-honey/40 transition"
            title={`Stage Theme: ${stageBg}`}
            aria-label="Cycle stage background"
          >
            {stageBg === "adaptive" ? (
              <Sun size={17} />
            ) : stageBg === "dark" ? (
              <Moon size={17} />
            ) : (
              <Grid size={17} />
            )}
          </button>

          <button
            onClick={() => setIsInfoOpen((prev) => !prev)}
            className={cn(
              "rounded-xl border border-border/30 bg-panel/60 p-2 text-ink-soft hover:text-honey hover:border-honey/40 transition",
              isInfoOpen && "border-honey text-honey bg-honey/15"
            )}
            title="Toggle Info Panel (I)"
            aria-label="Toggle info panel"
          >
            <Info size={17} />
          </button>

          <button
            onClick={trash}
            className="rounded-xl border border-border/30 bg-panel/60 p-2 text-red-500 hover:bg-red-500/10 transition"
            title="Move to Trash"
            aria-label="Move to trash"
          >
            <Trash2 size={17} />
          </button>
        </div>
      </header>

      {/* Main Image Stage Container */}
      <div
        className="relative flex flex-1 items-center justify-center overflow-hidden p-6 cursor-grab active:cursor-grabbing"
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onDoubleClick={handleDoubleTap}
        onContextMenu={handleContextMenu}
      >
        {/* Navigation Arrow Left */}
        {prevItem && (
          <button
            onClick={() => goTo(prevItem)}
            className="absolute left-6 top-1/2 z-20 grid size-12 -translate-y-1/2 place-items-center rounded-2xl border border-white/20 dark:border-white/10 bg-panel/75 dark:bg-panel/75 text-ink shadow-xl backdrop-blur-xl transition hover:scale-110 hover:border-honey hover:text-honey active:scale-95"
            aria-label="Previous image"
          >
            <ChevronLeft size={24} />
          </button>
        )}

        {/* The Image Preview Canvas */}
        <div
          className="flex h-full w-full items-center justify-center transition-transform duration-75 ease-out"
          style={{
            transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom}) rotate(${rotation}deg)`,
            transformOrigin: "center center",
          }}
        >
          <MediaThumb
            mediaId={item.id}
            variant="original"
            alt={item.filename}
            className="max-h-full max-w-full rounded-2xl object-contain shadow-2xl transition-all duration-150 pointer-events-none"
          />
        </div>

        {/* Navigation Arrow Right */}
        {nextItem && (
          <button
            onClick={() => goTo(nextItem)}
            className="absolute right-6 top-1/2 z-20 grid size-12 -translate-y-1/2 place-items-center rounded-2xl border border-white/20 dark:border-white/10 bg-panel/75 dark:bg-panel/75 text-ink shadow-xl backdrop-blur-xl transition hover:scale-110 hover:border-honey hover:text-honey active:scale-95"
            aria-label="Next image"
          >
            <ChevronRight size={24} />
          </button>
        )}
      </div>

      {/* Floating Bottom Control Dock */}
      <div className="relative z-30 flex flex-col items-center gap-2 pb-4">
        {/* Filmstrip Carousel */}
        <MediaViewerFilmstrip
          items={neighbors}
          currentId={item.id}
          onSelect={goTo}
        />

        {/* Control Buttons Pill */}
        <div className="flex items-center gap-1.5 rounded-2xl border border-white/20 dark:border-white/10 bg-panel/80 dark:bg-panel/80 p-1.5 shadow-2xl backdrop-blur-2xl text-ink text-xs">
          <button
            onClick={handleZoomOut}
            className="rounded-xl p-2 hover:bg-honey/15 hover:text-honey transition"
            title="Zoom Out (-)"
          >
            <ZoomOut size={16} />
          </button>

          <button
            onClick={handleResetZoom}
            className="rounded-xl px-2.5 py-1 font-bold text-[11px] hover:bg-honey/15 hover:text-honey transition"
            title="Reset Zoom / Fit to Screen"
          >
            {zoom === 1 ? "Fit" : `${Math.round(zoom * 100)}%`}
          </button>

          <button
            onClick={handleZoomIn}
            className="rounded-xl p-2 hover:bg-honey/15 hover:text-honey transition"
            title="Zoom In (+)"
          >
            <ZoomIn size={16} />
          </button>

          <div className="h-4 w-px bg-border/40 my-auto mx-0.5" />

          <button
            onClick={handleRotateCcw}
            className="rounded-xl p-2 hover:bg-honey/15 hover:text-honey transition"
            title="Rotate Left (Shift+R)"
          >
            <RotateCcw size={16} />
          </button>

          <button
            onClick={handleRotateCw}
            className="rounded-xl p-2 hover:bg-honey/15 hover:text-honey transition"
            title="Rotate Right (R)"
          >
            <RotateCw size={16} />
          </button>

          <div className="h-4 w-px bg-border/40 my-auto mx-0.5" />

          <button
            onClick={() => setIsPlayingSlideshow((prev) => !prev)}
            className={cn(
              "rounded-xl p-2 transition",
              isPlayingSlideshow
                ? "bg-honey text-white"
                : "hover:bg-honey/15 hover:text-honey"
            )}
            title="Slideshow (Space)"
          >
            {isPlayingSlideshow ? <Pause size={16} /> : <Play size={16} />}
          </button>

          <button
            onClick={toggleFullscreen}
            className="rounded-xl p-2 hover:bg-honey/15 hover:text-honey transition"
            title="Fullscreen (F)"
          >
            {isFullscreen ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
        </div>
      </div>

      {/* Info & EXIF Drawer */}
      <MediaViewerInfoDrawer
        item={item}
        isOpen={isInfoOpen}
        onClose={() => setIsInfoOpen(false)}
        onCopyPath={handleCopyPath}
        onOpenFolder={handleOpenFolder}
      />

      {/* Right Click Context Menu */}
      {contextMenu && (
        <MediaViewerContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          item={item}
          onZoomIn={handleZoomIn}
          onZoomOut={handleZoomOut}
          onResetZoom={handleResetZoom}
          onRotate={handleRotateCw}
          onToggleFavorite={toggleFavorite}
          onToggleInfo={() => setIsInfoOpen((prev) => !prev)}
          onTrash={trash}
          onCopyPath={handleCopyPath}
          onCopyImage={handleCopyImage}
          onOpenFolder={handleOpenFolder}
        />
      )}
    </div>
  );
}
