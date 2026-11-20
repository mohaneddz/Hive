import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  CheckCircle2,
  FolderPlus,
  Heart,
  Images,
  Layers,
  Loader2,
  MapPin,
  Sparkles,
  Star,
} from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaCard } from "@/components/media/MediaCard";
import { MediaThumb } from "@/components/media/MediaThumb";
import { StatTile } from "@/components/ui/StatTile";
import { PageError, PageSkeleton } from "@/components/ui/AsyncState";
import { addWatchedFolder, getAestheticRanking, getMediaPage, getOnThisDay, isTauri, scanFolder } from "@/lib/tauri";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { useJobProgress } from "@/hooks/useJobProgress";
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
        <div className="grid size-9 place-items-center rounded-xl bg-cream dark:bg-honey/15 text-honey-deep">{icon}</div>
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

function MediaRail({ items }: { items: MediaItem[] }) {
  return (
    <div className="mt-5 flex snap-x gap-4 overflow-x-auto pb-2 [scrollbar-width:thin]">
      {items.map((item) => (
        <div key={item.id} className="w-44 shrink-0 snap-start xl:w-52">
          <MediaCard item={item} />
        </div>
      ))}
    </div>
  );
}

export function HomePage() {
  const { stats } = useLibraryStats();
  const jobs = useJobProgress();
  const [recent, setRecent] = useState<MediaItem[]>([]);
  const [favorites, setFavorites] = useState<MediaItem[]>([]);
  const [memories, setMemories] = useState<MediaItem[]>([]);
  const [continueViewing, setContinueViewing] = useState<MediaItem[]>([]);
  const [topAesthetic, setTopAesthetic] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [failure, setFailure] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!isTauri()) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setFailure(null);
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
      // nothing opened yet would still return rows — filter them out here.
      setContinueViewing(viewedPage.items.filter((item) => item.lastViewedAt));
      setTopAesthetic(aestheticRanked.map((r) => r.item));
    } catch (cause) {
      setFailure(String(cause));
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
  const activeJobs = jobs.filter((job) => job.status === "running");

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Home"
        title="Welcome back."
        description={
          stats
            ? `${formatCount(stats.totalItems, "item")} · ${formatBytes(stats.totalBytes)} across ${formatCount(stats.folderCount, "folder")}.`
            : loading
              ? "Loading your library…"
              : "Add a folder to begin your private, local library."
        }
        action={
          <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
            Add folder
          </Button>
        }
      />

      {failure && <PageError message={failure} onRetry={() => void load()} />}
      {loading && !failure && <PageSkeleton cards={8} />}

      {isEmpty && !failure && (
        <div>
          <EmptyState
            icon={<Images size={22} />}
            title="Build your private photo library"
            description="Point Hive at a folder of photos or videos. Everything is indexed locally — nothing ever leaves your machine."
            action={
              <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
                Add your first folder
              </Button>
            }
          />
          <div className="mt-5 grid gap-3 md:grid-cols-3">
            {[
              ["1", "Choose folders", "Keep files where they are; Hive watches them in place."],
              ["2", "Let Hive organize", "Thumbnails, dates, people, and search are prepared locally."],
              ["3", "Explore privately", "Browse, edit, and rediscover memories without uploading them."],
            ].map(([number, title, copy]) => (
              <Card key={number} className="flex gap-3 p-4">
                <span className="grid size-8 shrink-0 place-items-center rounded-xl bg-honey/15 text-xs font-extrabold text-honey-deep">{number}</span>
                <div><p className="text-xs font-extrabold text-ink">{title}</p><p className="mt-1 text-[11px] leading-relaxed text-ink-muted">{copy}</p></div>
              </Card>
            ))}
          </div>
        </div>
      )}

      {stats && !isEmpty && (
        <>
          {continueViewing[0] && (
            <Link
              to={`/media/${continueViewing[0].id}`}
              className="group relative mt-7 block min-h-64 overflow-hidden rounded-[24px] border border-ink/[.08] bg-panel shadow-[0_18px_60px_rgba(0,0,0,.16)]"
            >
              <MediaThumb
                mediaId={continueViewing[0].id}
                alt={continueViewing[0].filename}
                className="absolute inset-0 size-full object-cover transition duration-700 group-hover:scale-[1.02]"
              />
              <div className="absolute inset-0 bg-gradient-to-r from-black/85 via-black/45 to-transparent" />
              <div className="relative flex min-h-64 max-w-xl flex-col justify-center p-8 text-white">
                <p className="text-[10px] font-extrabold uppercase tracking-[.18em] text-honey">Continue where you left off</p>
                <h2 className="mt-3 truncate text-2xl font-extrabold">{continueViewing[0].filename}</h2>
                <p className="mt-2 text-xs text-white/70">Return to the last photo you opened.</p>
                <span className="mt-6 inline-flex w-fit items-center gap-2 rounded-xl border border-honey/70 bg-black/20 px-4 py-2 text-xs font-extrabold text-honey backdrop-blur">
                  Open photo <ArrowRight size={14} />
                </span>
              </div>
            </Link>
          )}

          <div className="mt-7 grid grid-cols-2 gap-4 xl:grid-cols-4">
            <StatTile
              icon={<Images size={17} />}
              label="Photos & videos"
              value={stats.totalItems.toLocaleString()}
              hint={`${stats.imageCount.toLocaleString()} photos · ${stats.videoCount.toLocaleString()} videos`}
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

          <section className="mt-7 rounded-[20px] border border-ink/[.08] bg-panel p-5 shadow-[0_12px_40px_rgba(75,52,10,.055)]" aria-labelledby="background-activity-title">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h2 id="background-activity-title" className="text-sm font-extrabold text-ink">Background activity</h2>
                <p className="mt-0.5 text-[11px] text-ink-muted">Indexing and local AI work stay visible while you browse.</p>
              </div>
              {activeJobs.length > 0 ? <Loader2 size={18} className="animate-spin text-honey-deep" /> : <CheckCircle2 size={18} className="text-emerald-500" />}
            </div>
            {activeJobs.length === 0 ? (
              <p className="mt-4 rounded-xl bg-canvas px-4 py-3 text-xs font-semibold text-ink-muted">Everything is up to date. Hive is idle.</p>
            ) : (
              <div className="mt-4 grid gap-2 md:grid-cols-2">
                {activeJobs.map((job) => {
                  const percent = job.total > 0 ? Math.round((job.current / job.total) * 100) : 0;
                  return (
                    <div key={job.id} className="rounded-xl bg-canvas px-4 py-3">
                      <div className="flex items-center justify-between gap-3 text-xs font-bold text-ink">
                        <span className="truncate">{job.message || job.kind.replace(/_/g, " ")}</span>
                        <span className="text-honey-deep">{percent}%</span>
                      </div>
                      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-ink/10">
                        <div className="h-full rounded-full bg-honey transition-[width]" style={{ width: `${percent}%` }} />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </section>

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
              <MediaRail items={memories} />
            </>
          )}

          {topAesthetic.length > 0 && (
            <>
              <SectionHeader
                icon={<Star size={17} />}
                title="Top Aesthetic Photos"
                subtitle="Highest quality photos scored by the LAION Aesthetic Predictor"
              />
              <MediaRail items={topAesthetic} />
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
              <MediaRail items={favorites} />
            </>
          )}

          <SectionHeader
            icon={<Images size={17} />}
            title="Recently added"
            subtitle="The newest arrivals in your library"
            to={routes.gallery.path}
          />
          <MediaRail items={recent} />
        </>
      )}
    </div>
  );
}
