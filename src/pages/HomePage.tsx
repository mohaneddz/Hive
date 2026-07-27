import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  Clock,
  FolderPlus,
  Heart,
  Images,
  Layers,
  MapPin,
  Sparkles,
  Star,
} from "lucide-react";

import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaGrid } from "@/components/media/MediaGrid";
import { StatTile } from "@/components/ui/StatTile";
import { addWatchedFolder, getAestheticRanking, getMediaPage, getOnThisDay, isTauri, scanFolder } from "@/lib/tauri";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { routes } from "@/config/routes";
import type { MediaItem } from "@/types/media";
import { formatBytes, formatCount, yearsAgo } from "@/utils/format";

const RECENT_COUNT = 12;
const RAIL_COUNT = 8;

function SectionHeader({ icon, title, subtitle, to }: { icon: React.ReactNode; title: string; subtitle: string; to?: string }) {
  return (
    <div className="mt-10 flex items-end justify-between gap-4">
      <div className="flex items-center gap-3">
        <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">{icon}</div>
        <div>
          <h2 className="text-base font-extrabold text-ink">{title}</h2>
          <p className="text-xs text-ink-muted">{subtitle}</p>
        </div>
      </div>
      {to && (
        <Link
          to={to}
          className="inline-flex items-center gap-1.5 text-xs font-bold text-honey-deep transition hover:gap-2.5"
        >
          See all <ArrowRight size={14} />
        </Link>
      )}
    </div>
  );
}

export function HomePage() {
  const { stats } = useLibraryStats();
  const [recent, setRecent] = useState<MediaItem[]>([]);
  const [favorites, setFavorites] = useState<MediaItem[]>([]);
  const [memories, setMemories] = useState<MediaItem[]>([]);
  const [continueViewing, setContinueViewing] = useState<MediaItem[]>([]);
  const [topAesthetic, setTopAesthetic] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    try {
      const [recentPage, favoritePage, onThisDay, viewedPage, aestheticRanked] = await Promise.all([
        getMediaPage({ limit: RECENT_COUNT, offset: 0, sort: "added" }),
        getMediaPage({ limit: RAIL_COUNT, offset: 0, favoritesOnly: true }),
        getOnThisDay(RAIL_COUNT),
        getMediaPage({ limit: RAIL_COUNT, offset: 0, sort: "viewed" }),
        getAestheticRanking(RAIL_COUNT).catch(() => []),
      ]);
      setRecent(recentPage.items);
      setFavorites(favoritePage.items);
      setMemories(onThisDay);
      // "viewed" sorts NULLs last in SQLite DESC ordering, but a library with
      // nothing opened yet would still return rows â€” filter them out here.
      setContinueViewing(viewedPage.items.filter((item) => item.lastViewedAt));
      setTopAesthetic(aestheticRanked.map((r) => r.item));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("media:changed", () => void load());
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [load]);

  const chooseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Add a media folder" });
      if (typeof selected !== "string") return;
      const folder = await addWatchedFolder(selected);
      void scanFolder(folder.id);
      await load();
    } catch {
      /* No native folder picker in the web preview. */
    }
  };

  const isEmpty = !loading && (stats?.totalItems ?? 0) === 0;

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Home"
        title="Welcome back."
        description={
          stats
            ? `${formatCount(stats.totalItems, "item")} Â· ${formatBytes(stats.totalBytes)} across ${formatCount(stats.folderCount, "folder")}.`
            : "Loading your libraryâ€¦"
        }
        action={
          <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
            Add folder
          </Button>
        }
      />

      {isEmpty && (
        <EmptyState
          icon={<Images size={22} />}
          title="Your library is empty"
          description="Point Hive at a folder of photos or videos. Everything is indexed locally â€” nothing ever leaves your machine."
          action={
            <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
              Add folder
            </Button>
          }
        />
      )}

      {stats && !isEmpty && (
        <>
          <div className="mt-7 grid grid-cols-4 gap-4">
            <StatTile
              icon={<Images size={17} />}
              label="Photos & videos"
              value={stats.totalItems.toLocaleString()}
              hint={`${stats.imageCount.toLocaleString()} photos Â· ${stats.videoCount.toLocaleString()} videos`}
              to={routes.gallery.path}
            />
            <StatTile
              icon={<Heart size={17} />}
              label="Favorites"
              value={stats.favorites.toLocaleString()}
              to={`${routes.collections.path}?view=favorites`}
            />
            <StatTile
              icon={<Layers size={17} />}
              label="Albums"
              value={stats.albumCount.toLocaleString()}
              to={routes.collections.path}
            />
            <StatTile
              icon={<MapPin size={17} />}
              label="Geotagged"
              value={stats.placeCount.toLocaleString()}
              hint="Photos carrying GPS"
              to={routes.places.path}
            />
          </div>

          {memories.length > 0 && (
            <>
              <SectionHeader
                icon={<Sparkles size={17} />}
                title="On this day"
                subtitle={
                  yearsAgo(memories[0].takenAt)
                    ? `From ${yearsAgo(memories[0].takenAt)} year${yearsAgo(memories[0].takenAt) === 1 ? "" : "s"} ago and earlier`
                    : "From earlier years"
                }
              />
              <MediaGrid items={memories} className="mt-5" />
            </>
          )}

          {continueViewing.length > 0 && (
            <>
              <SectionHeader
                icon={<Clock size={17} />}
                title="Continue where you left off"
                subtitle="The last things you opened"
              />
              <MediaGrid items={continueViewing} className="mt-5" />
            </>
          )}

          {topAesthetic.length > 0 && (
            <>
              <SectionHeader
                icon={<Star size={17} />}
                title="Top Aesthetic Photos"
                subtitle="Highest quality photos scored by NIMA AI"
              />
              <MediaGrid items={topAesthetic} className="mt-5" />
            </>
          )}

          {favorites.length > 0 && (
            <>
              <SectionHeader
                icon={<Heart size={17} />}
                title="Favorites"
                subtitle="Everything you starred"
                to={`${routes.collections.path}?view=favorites`}
              />
              <MediaGrid items={favorites} className="mt-5" />
            </>
          )}

          <SectionHeader
            icon={<Images size={17} />}
            title="Recently added"
            subtitle="The newest arrivals in your library"
            to={routes.gallery.path}
          />
          <MediaGrid items={recent} className="mt-5" />
        </>
      )}
    </div>
  );
}
