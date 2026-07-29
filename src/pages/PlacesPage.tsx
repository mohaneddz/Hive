import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  ExternalLink,
  Globe,
  Loader2,
  LocateFixed,
  Maximize,
  MapPin,
  Minimize,
  Minus,
  Plus,
} from "lucide-react";

import { Card } from "@/components/ui/Card";
import { MediaGrid } from "@/components/media/MediaGrid";
import { MediaThumb } from "@/components/media/MediaThumb";
import {
  getCachedPlaceNames,
  getGeocodingEnabled,
  isTauri,
  listMediaAtPlace,
  listPlaces,
  lookupPlaceNames,
} from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem, PlaceGroup } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCoordinates, formatCount, formatDate } from "@/utils/format";

/** Grouping granularity, in decimal places of latitude/longitude. */
const ZOOM_LEVELS = [
  { label: "Region", precision: 0 },
  { label: "City", precision: 1 },
  { label: "Area", precision: 2 },
  { label: "Street", precision: 3 },
];

const MIN_MAP_ZOOM = 1;
const MAX_MAP_ZOOM = 6;

/**
 * Plots the pins on an equirectangular projection: longitude maps linearly to x,
 * latitude to y. No tiles are fetched — a local-first gallery should not need the
 * network to show you where your photos were taken. Zoom and pan are real,
 * client-side transforms of that same projection, not a tile viewer.
 */
function MiniMap({
  places,
  activeId,
  onSelect,
}: {
  places: PlaceGroup[];
  activeId: string | null;
  onSelect: (place: PlaceGroup) => void;
}) {
  const frameRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [isFullscreen, setIsFullscreen] = useState(false);

  useEffect(() => {
    const onChange = () => setIsFullscreen(document.fullscreenElement === frameRef.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const maxCount = Math.max(...places.map((place) => place.count), 1);

  const zoomIn = () => setZoom((z) => Math.min(MAX_MAP_ZOOM, z + 0.75));
  const zoomOut = () =>
    setZoom((z) => {
      const next = Math.max(MIN_MAP_ZOOM, z - 0.75);
      if (next === MIN_MAP_ZOOM) setPan({ x: 0, y: 0 });
      return next;
    });
  const recenter = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };

  const handleWheel = (event: React.WheelEvent) => {
    event.preventDefault();
    if (event.deltaY < 0) zoomIn();
    else zoomOut();
  };

  const handleMouseDown = (event: React.MouseEvent) => {
    if (event.button !== 0 || zoom <= MIN_MAP_ZOOM) return;
    setDragging(true);
    setDragStart({ x: event.clientX - pan.x, y: event.clientY - pan.y });
  };
  const handleMouseMove = (event: React.MouseEvent) => {
    if (!dragging) return;
    setPan({ x: event.clientX - dragStart.x, y: event.clientY - dragStart.y });
  };
  const stopDragging = () => setDragging(false);

  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      frameRef.current?.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  };

  return (
    <Card className="mt-7 overflow-hidden">
      <div
        ref={frameRef}
        className={cn("relative bg-shell", isFullscreen && "flex h-screen flex-col justify-center")}
      >
        <svg
          viewBox="0 0 360 180"
          role="img"
          aria-label="Photo locations"
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={stopDragging}
          onMouseLeave={stopDragging}
          className={cn("block h-auto w-full select-none", zoom > 1 && (dragging ? "cursor-grabbing" : "cursor-grab"))}
        >
          <g
            style={{
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
              transformOrigin: "180px 90px",
              transition: dragging ? "none" : "transform 150ms ease-out",
            }}
          >
            {/* Graticule every 30° gives a sense of scale without any map data. */}
            {[30, 60, 90, 120, 150, 180, 210, 240, 270, 300, 330].map((x) => (
              <line key={`v${x}`} x1={x} y1={0} x2={x} y2={180} stroke="currentColor" strokeWidth={0.3} className="text-ink-muted/25" />
            ))}
            {[30, 60, 90, 120, 150].map((y) => (
              <line key={`h${y}`} x1={0} y1={y} x2={360} y2={y} stroke="currentColor" strokeWidth={0.3} className="text-ink-muted/25" />
            ))}
            <line x1={0} y1={90} x2={360} y2={90} stroke="currentColor" strokeWidth={0.7} className="text-ink-muted/45" />

            {places.map((place) => {
              const x = place.lon + 180;
              const y = 90 - place.lat;
              const radius = 2 + (place.count / maxCount) * 5;
              const isActive = place.id === activeId;
              return (
                <g key={place.id} onClick={() => onSelect(place)} className="cursor-pointer">
                  <circle cx={x} cy={y} r={radius + 3} className="fill-honey/20" />
                  <circle
                    cx={x}
                    cy={y}
                    r={radius}
                    className={cn("fill-honey stroke-[0.6]", isActive ? "stroke-ink" : "stroke-transparent")}
                  />
                </g>
              );
            })}
          </g>
        </svg>

        {places.length === 0 && (
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 text-center">
            <MapPin size={22} className="text-ink-muted" />
            <p className="text-xs font-bold text-ink-muted">No geotagged photos yet</p>
          </div>
        )}

        <div className="absolute bottom-3 left-3 flex flex-col gap-1 rounded-xl border border-ink/[.1] bg-panel/90 p-1 shadow-lg backdrop-blur">
          <button
            onClick={zoomIn}
            disabled={zoom >= MAX_MAP_ZOOM}
            className="icon-button !size-8 border-0 disabled:opacity-30"
            aria-label="Zoom in"
            title="Zoom in"
          >
            <Plus size={14} />
          </button>
          <button
            onClick={zoomOut}
            disabled={zoom <= MIN_MAP_ZOOM}
            className="icon-button !size-8 border-0 disabled:opacity-30"
            aria-label="Zoom out"
            title="Zoom out"
          >
            <Minus size={14} />
          </button>
          <button onClick={recenter} className="icon-button !size-8 border-0" aria-label="Recenter" title="Recenter">
            <LocateFixed size={14} />
          </button>
          <button
            onClick={toggleFullscreen}
            className="icon-button !size-8 border-0"
            aria-label={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
          >
            {isFullscreen ? <Minimize size={14} /> : <Maximize size={14} />}
          </button>
        </div>
      </div>
    </Card>
  );
}

export function PlacesPage() {
  const [precision, setPrecision] = useState(1);
  const [places, setPlaces] = useState<PlaceGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [active, setActive] = useState<PlaceGroup | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);
  /** Cached place names, keyed the same way the backend caches them. */
  const [names, setNames] = useState<Map<string, string>>(new Map());
  const [geocoding, setGeocoding] = useState(false);
  const [naming, setNaming] = useState(false);

  const nameKey = (lat: number, lon: number) => `${lat.toFixed(2)},${lon.toFixed(2)}`;
  const nameFor = (place: PlaceGroup) => names.get(nameKey(place.lat, place.lon));

  const absorbNames = (entries: [number, number, string][]) =>
    setNames((prev) => {
      const next = new Map(prev);
      entries.forEach(([lat, lon, name]) => next.set(nameKey(lat, lon), name));
      return next;
    });

  useEffect(() => {
    if (!isTauri()) return;
    void getGeocodingEnabled().then(setGeocoding);
    // Cached names cost nothing and work offline.
    void getCachedPlaceNames().then(absorbNames);
  }, []);

  const lookupNames = async () => {
    setNaming(true);
    try {
      absorbNames(await lookupPlaceNames(places.map((place) => [place.lat, place.lon])));
    } finally {
      setNaming(false);
    }
  };

  const load = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    try {
      setPlaces(await listPlaces(precision));
    } finally {
      setLoading(false);
    }
  }, [precision]);

  useEffect(() => {
    void load();
  }, [load]);

  const openPlace = async (place: PlaceGroup) => {
    setActive(place);
    setItems(await listMediaAtPlace(place.lat, place.lon, precision));
  };

  const applyFavorite = useCallback((mediaId: string, favorite: boolean) => {
    setItems((prev) =>
      prev.map((item) => (item.id === mediaId ? { ...item, isFavorite: favorite } : item)),
    );
  }, []);

  /* --------------------------------------------------------- one place -- */

  if (active) {
    return (
      <div>
        <button
          onClick={() => setActive(null)}
          className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <ArrowLeft size={14} /> All places
        </button>
        <GalleryPageHeader
          eyebrow="Place"
          title={nameFor(active) ?? formatCoordinates(active.lat, active.lon)}
          description={`${formatCount(active.count, "photo")} taken here${
            active.earliest ? ` · ${formatDate(active.earliest)} → ${formatDate(active.latest)}` : ""
          }`}
          action={
            <button
              onClick={() =>
                void openUrl(
                  `https://www.openstreetmap.org/?mlat=${active.lat}&mlon=${active.lon}#map=13/${active.lat}/${active.lon}`,
                )
              }
              className="inline-flex h-10 items-center gap-2 rounded-xl border border-ink/10 bg-panel px-4 text-sm font-bold text-ink transition hover:border-honey/40"
            >
              <ExternalLink size={15} /> Open in maps
            </button>
          }
        />
        <MediaGrid items={items} className="mt-7" onFavoriteChange={applyFavorite} />
      </div>
    );
  }

  /* ----------------------------------------------------------- overview -- */

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Places"
        title="Where your photos were taken."
        description="Built from the GPS coordinates already stored in your photos. Nothing is sent anywhere to work this out."
        action={
          <div className="flex items-center gap-1 rounded-xl border border-ink/[.12] bg-panel p-1">
            {ZOOM_LEVELS.map((level) => (
              <button
                key={level.precision}
                onClick={() => setPrecision(level.precision)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-xs font-bold text-ink-muted transition",
                  precision === level.precision && "bg-cream text-honey-deep",
                )}
              >
                {level.label}
              </button>
            ))}
          </div>
        }
      />

      {loading && places.length === 0 ? (
        <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
      ) : (
        <MiniMap places={places} activeId={null} onSelect={openPlace} />
      )}

      {!loading && places.length === 0 && (
        <p className="mt-4 text-center text-xs text-ink-muted">
          Hive reads GPS coordinates straight from EXIF. Photos taken with location services off
          carry none, so nothing shows up on the map.
        </p>
      )}

      {places.length > 0 && geocoding && names.size < places.length && (
        <div className="mt-6 flex items-center justify-between gap-4 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3">
          <p className="text-xs font-semibold text-honey-deep">
            {names.size === 0
              ? "Place names are enabled but nothing has been looked up yet."
              : `${places.length - names.size} pin${places.length - names.size === 1 ? "" : "s"} still unnamed.`}{" "}
            <span className="font-normal">
              Looking them up sends coordinates to OpenStreetMap, one per second.
            </span>
          </p>
          <button
            onClick={lookupNames}
            disabled={naming}
            className="inline-flex shrink-0 items-center gap-2 rounded-xl bg-honey px-4 py-2 text-xs font-extrabold text-[#3b2900] transition hover:bg-honey-dark disabled:opacity-50"
          >
            {naming ? <Loader2 size={14} className="animate-spin" /> : <Globe size={14} />}
            {naming ? "Looking up…" : "Look up names"}
          </button>
        </div>
      )}

      {places.length > 0 && (
        <div className="mt-7 grid grid-cols-4 gap-4">
          {places.map((place) => (
            <button
              key={place.id}
              onClick={() => openPlace(place)}
              className="group overflow-hidden rounded-[22px] border border-ink/[.07] bg-panel text-left shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
            >
              <div className="relative aspect-[4/3]">
                <MediaThumb
                  mediaId={place.coverMediaId}
                  alt={formatCoordinates(place.lat, place.lon)}
                  className="size-full object-cover transition duration-700 group-hover:scale-[1.04]"
                />
                <span className="absolute left-3 top-3 inline-flex items-center gap-1.5 rounded-full bg-black/55 px-2.5 py-1 text-[11px] font-bold text-white">
                  <MapPin size={11} />
                  {formatCount(place.count, "photo")}
                </span>
              </div>
              <div className="p-4">
                <p className="truncate text-xs font-extrabold text-ink">
                  {nameFor(place) ?? formatCoordinates(place.lat, place.lon)}
                </p>
                <p className="mt-0.5 truncate text-[11px] text-ink-muted">
                  {nameFor(place)
                    ? formatCoordinates(place.lat, place.lon)
                    : place.earliest
                      ? formatDate(place.earliest)
                      : "Undated"}
                </p>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
