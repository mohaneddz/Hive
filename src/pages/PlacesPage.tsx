import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  ExternalLink,
  Globe,
  Loader2,
  Maximize,
  MapPin,
  Minimize,
} from "lucide-react";
import L from "leaflet";
import "leaflet/dist/leaflet.css";

import { Card } from "@/components/ui/Card";
import { PageError, PageSkeleton } from "@/components/ui/AsyncState";
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

const OSM_TILES = "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png";
const CARTO_DARK_TILES =
  "https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png";

function isDarkMode() {
  return document.documentElement.classList.contains("dark");
}

/**
 * Interactive Leaflet world map with OpenStreetMap / CartoDB dark tiles.
 * Renders CircleMarkers at each PlaceGroup's lat/lon. No custom icon images
 * are needed, so Vite asset handling is not a concern.
 */
function LeafletMap({
  places,
  activeId,
  onSelect,
}: {
  places: PlaceGroup[];
  activeId: string | null;
  onSelect: (place: PlaceGroup) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);
  const tileLayerRef = useRef<L.TileLayer | null>(null);
  const markersRef = useRef<Map<string, L.CircleMarker>>(new Map());
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Track fullscreen changes
  useEffect(() => {
    const onChange = () => setIsFullscreen(document.fullscreenElement === frameRef.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  const toggleFullscreen = () => {
    if (!document.fullscreenElement) {
      frameRef.current?.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  };

  // Initialise map once
  useEffect(() => {
    if (!containerRef.current) return;

    const map = L.map(containerRef.current, {
      zoomControl: true,
      attributionControl: true,
    });

    mapRef.current = map;

    const tileUrl = isDarkMode() ? CARTO_DARK_TILES : OSM_TILES;
    const tile = L.tileLayer(tileUrl, {
      attribution:
        isDarkMode()
          ? '&copy; <a href="https://carto.com/">CartoDB</a>'
          : '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
      maxZoom: 18,
    }).addTo(map);
    tileLayerRef.current = tile;

    map.setView([20, 0], 2);

    // Dark-mode observer — swap tiles when .dark is toggled on <html>
    const observer = new MutationObserver(() => {
      const dark = isDarkMode();
      const url = dark ? CARTO_DARK_TILES : OSM_TILES;
      const attribution = dark
        ? '&copy; <a href="https://carto.com/">CartoDB</a>'
        : '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors';

      if (tileLayerRef.current) {
        tileLayerRef.current.remove();
      }
      const newTile = L.tileLayer(url, { attribution, maxZoom: 18 }).addTo(map);
      tileLayerRef.current = newTile;
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      observer.disconnect();
      map.remove();
      mapRef.current = null;
      tileLayerRef.current = null;
      markersRef.current.clear();
    };
  }, []);

  // Add / refresh markers whenever places or activeId changes
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    // Remove existing markers
    markersRef.current.forEach((m) => m.remove());
    markersRef.current.clear();

    if (places.length === 0) return;

    const maxCount = Math.max(...places.map((p) => p.count), 1);
    const bounds: [number, number][] = [];

    places.forEach((place) => {
      const isActive = place.id === activeId;
      const radius = 5 + (place.count / maxCount) * 10;

      const marker = L.circleMarker([place.lat, place.lon], {
        radius,
        color: isActive ? "#fff" : "#f5a623",
        weight: isActive ? 2 : 1,
        fillColor: "#f5a623",
        fillOpacity: 0.8,
      }).addTo(map);

      const label = `${formatCoordinates(place.lat, place.lon)} · ${formatCount(place.count, "photo")}`;
      marker.bindTooltip(label, { direction: "top", offset: [0, -radius] });

      marker.on("click", () => onSelect(place));
      markersRef.current.set(place.id, marker);
      bounds.push([place.lat, place.lon]);
    });

    if (bounds.length > 0) {
      map.fitBounds(bounds as L.LatLngBoundsExpression, { padding: [40, 40], maxZoom: 10 });
    }
  }, [places, activeId, onSelect]);

  return (
    <Card className="mt-7 overflow-hidden">
      <div
        ref={frameRef}
        className={cn("relative", isFullscreen && "flex h-screen flex-col justify-center")}
      >
        {/* Leaflet map container */}
        <div
          ref={containerRef}
          style={{ height: isFullscreen ? "100vh" : "420px", width: "100%" }}
        />

        {places.length === 0 && (
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 text-center">
            <MapPin size={22} className="text-ink-muted" />
            <p className="text-xs font-bold text-ink-muted">No geotagged photos yet</p>
          </div>
        )}

        {/* Custom fullscreen toggle overlay */}
        <div className="absolute bottom-[52px] right-2 z-[1000] flex flex-col gap-1 rounded-xl border border-ink/[.1] bg-panel/90 p-1 shadow-lg backdrop-blur">
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
  const [failure, setFailure] = useState<string | null>(null);

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
    if (!isTauri()) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setFailure(null);
    try {
      setPlaces(await listPlaces(precision));
    } catch (cause) {
      setFailure(String(cause));
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
                  precision === level.precision && "bg-honey/20 text-honey-deep dark:text-honey",
                )}
              >
                {level.label}
              </button>
            ))}
          </div>
        }
      />

      {failure && <PageError message={failure} onRetry={() => void load()} />}

      {loading && places.length === 0 && !failure ? (
        <PageSkeleton cards={4} />
      ) : (
        !failure && <LeafletMap places={places} activeId={null} onSelect={openPlace} />
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
