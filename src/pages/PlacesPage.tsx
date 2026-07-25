import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { MapPin } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { MediaThumb } from "@/components/media/MediaThumb";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { getPlaces, isTauri } from "@/lib/tauri";
import type { PlaceCluster } from "@/types/media";

function formatCoord(lat: number, lon: number) {
  const latLabel = `${Math.abs(lat).toFixed(1)}°${lat >= 0 ? "N" : "S"}`;
  const lonLabel = `${Math.abs(lon).toFixed(1)}°${lon >= 0 ? "E" : "W"}`;
  return `${latLabel}, ${lonLabel}`;
}

export function PlacesPage() {
  const [places, setPlaces] = useState<PlaceCluster[] | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setPlaces([]);
      return;
    }
    getPlaces().then(setPlaces);
  }, []);

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Places"
        title="Where your photos were taken."
        description="Grouped from GPS data embedded in your photos. Nothing leaves your device."
      />

      {places === null ? (
        <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
      ) : places.length === 0 ? (
        <Card className="mt-8 flex flex-col items-center gap-3 p-16 text-center">
          <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
            <MapPin size={22} />
          </div>
          <p className="text-sm font-extrabold text-ink">No location data yet</p>
          <p className="max-w-sm text-xs text-ink-muted">
            Photos with GPS coordinates in their EXIF metadata will show up here, grouped by
            location.
          </p>
        </Card>
      ) : (
        <div className="mt-7 grid grid-cols-4 gap-4">
          {places.map((place) => (
            <Link
              key={`${place.lat},${place.lon}`}
              to={`/media/${place.coverMediaId}`}
              className="group block"
            >
              <div className="artwork-frame aspect-square">
                <MediaThumb
                  mediaId={place.coverMediaId}
                  alt={formatCoord(place.lat, place.lon)}
                  className="size-full object-cover transition duration-500 group-hover:scale-[1.03]"
                />
                <div className="absolute inset-x-0 bottom-0 flex items-center gap-1.5 bg-gradient-to-t from-black/70 to-transparent p-3 text-white">
                  <MapPin size={12} />
                  <span className="text-xs font-bold">{formatCoord(place.lat, place.lon)}</span>
                </div>
              </div>
              <p className="mt-2 text-[11px] text-ink-muted">
                {place.count} item{place.count === 1 ? "" : "s"}
              </p>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
