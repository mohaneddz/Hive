import { useCallback, useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { confirm } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  ArrowLeft,
  CalendarRange,
  EyeOff,
  Heart,
  Layers,
  Pencil,
  Plane,
  Plus,
  Sparkle,
  Trash2,
} from "lucide-react";

import { EventsView, TimelineView } from "@/components/collections/AutoGroups";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaGrid } from "@/components/media/MediaGrid";
import { MediaThumb } from "@/components/media/MediaThumb";
import { routes } from "@/config/routes";
import { createAlbum, deleteAlbum, getMediaPage, isTauri, listAlbums, updateAlbum } from "@/lib/tauri";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { Album, MediaItem, MediaScope } from "@/types/media";
import { cn } from "@/utils/cn";
import { formatCount, relativeTime } from "@/utils/format";

const SMART_VIEWS = {
  favorites: {
    label: "Favorites",
    icon: Heart,
    description: "Everything you starred.",
    scope: "library" as MediaScope,
    favoritesOnly: true,
  },
  hidden: {
    label: "Hidden",
    icon: EyeOff,
    description: "Kept out of the library, search and albums.",
    scope: "hidden" as MediaScope,
    favoritesOnly: false,
  },
  archive: {
    label: "Archive",
    icon: Archive,
    description: "Out of the timeline, still searchable.",
    scope: "archive" as MediaScope,
    favoritesOnly: false,
  },
};

type SmartViewKey = keyof typeof SMART_VIEWS;

function isSmartView(value: string | null): value is SmartViewKey {
  return value !== null && value in SMART_VIEWS;
}

/**
 * Groupings Hive works out on its own. Despite sitting under "AI" in the task
 * split, none of them uses a model: they are dates, gaps between dates, and
 * distances between coordinates.
 */
const AUTO_VIEWS = {
  timeline: {
    label: "Timeline",
    icon: CalendarRange,
    description: "Your library by year, month or day.",
  },
  events: {
    label: "Events",
    icon: Sparkle,
    description: "Bursts of photos with a quiet gap on either side.",
  },
  trips: {
    label: "Trips",
    icon: Plane,
    description: "Runs of photos taken away from home, for at least a night.",
  },
};

type AutoViewKey = keyof typeof AUTO_VIEWS;

function isAutoView(value: string | null): value is AutoViewKey {
  return value !== null && value in AUTO_VIEWS;
}

export function CollectionsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const view = searchParams.get("view");

  const [albums, setAlbums] = useState<Album[]>([]);
  const { stats } = useLibraryStats();
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState("");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<Album | null>(null);
  const [editName, setEditName] = useState("");

  const loadOverview = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    try {
      setAlbums(await listAlbums());
    } finally {
      setLoading(false);
    }
  }, []);

  const loadSmartView = useCallback(async (key: SmartViewKey) => {
    if (!isTauri()) return;
    setLoading(true);
    try {
      const config = SMART_VIEWS[key];
      const page = await getMediaPage({
        limit: 500,
        offset: 0,
        scope: config.scope,
        favoritesOnly: config.favoritesOnly || undefined,
      });
      setItems(page.items);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isSmartView(view)) {
      void loadSmartView(view);
    } else {
      void loadOverview();
    }
  }, [view, loadSmartView, loadOverview]);

  const applyFavorite = useCallback((mediaId: string, favorite: boolean) => {
    setItems((prev) =>
      prev.map((item) => (item.id === mediaId ? { ...item, isFavorite: favorite } : item)),
    );
  }, []);

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    const album = await createAlbum(name);
    setAlbums((prev) => [album, ...prev]);
    setNewName("");
    setCreating(false);
  };

  const rename = async () => {
    if (!editing) return;
    const name = editName.trim();
    if (!name) return;
    await updateAlbum(editing.id, name, editing.description ?? undefined);
    setAlbums((prev) =>
      prev.map((album) => (album.id === editing.id ? { ...album, name } : album)),
    );
    setEditing(null);
  };

  const remove = async (album: Album) => {
    const confirmed = await confirm(
      `Delete the album "${album.name}"? The photos inside stay in your library.`,
      { title: "Delete album", kind: "warning" },
    );
    if (!confirmed) return;
    await deleteAlbum(album.id);
    setAlbums((prev) => prev.filter((entry) => entry.id !== album.id));
  };

  /* ------------------------------------------- one automatic grouping open -- */

  if (isAutoView(view)) {
    const config = AUTO_VIEWS[view];
    return (
      <div>
        <button
          onClick={() => setSearchParams({})}
          className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <ArrowLeft size={14} /> All collections
        </button>
        <GalleryPageHeader
          eyebrow="Collections"
          title={`${config.label}.`}
          description={config.description}
        />
        <div className="mt-7">
          {view === "timeline" ? <TimelineView /> : <EventsView trips={view === "trips"} />}
        </div>
      </div>
    );
  }

  /* ------------------------------------------------- one smart view opened -- */

  if (isSmartView(view)) {
    const config = SMART_VIEWS[view];
    return (
      <div>
        <button
          onClick={() => setSearchParams({})}
          className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
        >
          <ArrowLeft size={14} /> All collections
        </button>
        <GalleryPageHeader
          eyebrow="Collections"
          title={`${config.label}.`}
          description={
            loading ? config.description : `${formatCount(items.length, "item")}. ${config.description}`
          }
        />
        {loading ? (
          <div className="mt-12 text-center text-sm text-ink-muted">Loading…</div>
        ) : items.length === 0 ? (
          <EmptyState
            icon={<config.icon size={22} />}
            title={`Nothing in ${config.label.toLowerCase()} yet`}
            description={config.description}
          />
        ) : (
          <MediaGrid items={items} className="mt-7" onFavoriteChange={applyFavorite} />
        )}
      </div>
    );
  }

  /* ------------------------------------------------------------- overview -- */

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Collections"
        title="Albums and smart views."
        description="Group photos by hand, or jump straight to what Hive already tracks for you."
        action={
          !creating ? (
            <Button icon={<Plus size={16} />} onClick={() => setCreating(true)}>
              New album
            </Button>
          ) : undefined
        }
      />

      {creating && (
        <div className="mt-6 flex items-center gap-2">
          <input
            autoFocus
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void create();
              if (event.key === "Escape") setCreating(false);
            }}
            placeholder="Album name"
            className="search-input max-w-sm !pl-3.5"
          />
          <Button onClick={create} disabled={!newName.trim()}>
            Create
          </Button>
          <Button variant="ghost" onClick={() => setCreating(false)}>
            Cancel
          </Button>
        </div>
      )}

      <div className="mt-7 grid grid-cols-4 gap-4">
        {(Object.keys(SMART_VIEWS) as SmartViewKey[]).map((key) => {
          const config = SMART_VIEWS[key];
          const count =
            key === "favorites"
              ? stats?.favorites
              : key === "hidden"
                ? stats?.hiddenCount
                : stats?.archivedCount;
          return (
            <button
              key={key}
              onClick={() => setSearchParams({ view: key })}
              className="flex items-center gap-3.5 rounded-[18px] border border-ink/[.07] bg-panel p-4 text-left shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
            >
              <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
                <config.icon size={17} />
              </div>
              <div className="min-w-0">
                <p className="text-[22px] font-extrabold leading-none tracking-[-.03em] text-ink">
                  {(count ?? 0).toLocaleString()}
                </p>
                <p className="mt-1 text-[11px] font-bold text-ink-muted">{config.label}</p>
              </div>
            </button>
          );
        })}
        <Link
          to={routes.trash.path}
          className="flex items-center gap-3.5 rounded-[18px] border border-ink/[.07] bg-panel p-4 shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
        >
          <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
            <Trash2 size={17} />
          </div>
          <div className="min-w-0">
            <p className="text-[22px] font-extrabold leading-none tracking-[-.03em] text-ink">
              {(stats?.trashed ?? 0).toLocaleString()}
            </p>
            <p className="mt-1 text-[11px] font-bold text-ink-muted">Trash</p>
          </div>
        </Link>
      </div>

      <div className="mt-10 flex items-center gap-3">
        <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
          <Sparkle size={17} />
        </div>
        <div>
          <h2 className="text-base font-extrabold text-ink">Grouped automatically</h2>
          <p className="text-xs text-ink-muted">
            Worked out from dates and coordinates alone — no model involved
          </p>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-3 gap-4">
        {(Object.keys(AUTO_VIEWS) as AutoViewKey[]).map((key) => {
          const config = AUTO_VIEWS[key];
          return (
            <button
              key={key}
              onClick={() => setSearchParams({ view: key })}
              className="flex items-start gap-3.5 rounded-[18px] border border-ink/[.07] bg-panel p-4 text-left shadow-[0_12px_40px_rgba(75,52,10,.055)] transition hover:-translate-y-px hover:border-honey/40"
            >
              <div className="grid size-9 shrink-0 place-items-center rounded-xl bg-cream text-honey-deep">
                <config.icon size={17} />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-extrabold text-ink">{config.label}</p>
                <p className="mt-0.5 text-[11px] leading-relaxed text-ink-muted">
                  {config.description}
                </p>
              </div>
            </button>
          );
        })}
      </div>

      <div className="mt-10 flex items-center gap-3">
        <div className="grid size-9 place-items-center rounded-xl bg-cream text-honey-deep">
          <Layers size={17} />
        </div>
        <div>
          <h2 className="text-base font-extrabold text-ink">Albums</h2>
          <p className="text-xs text-ink-muted">
            {albums.length === 0 ? "None yet" : formatCount(albums.length, "album")}
          </p>
        </div>
      </div>

      {!loading && albums.length === 0 && (
        <EmptyState
          icon={<Layers size={22} />}
          title="No albums yet"
          description="Albums are yours to arrange by hand. Open any photo and use “Add to album”, or create an empty one to fill later."
          action={
            <Button icon={<Plus size={16} />} onClick={() => setCreating(true)}>
              New album
            </Button>
          }
        />
      )}

      {albums.length > 0 && (
        <div className="mt-5 grid grid-cols-4 gap-4">
          {albums.map((album) => (
            <Card key={album.id} className="group overflow-hidden">
              <Link to={`/albums/${album.id}`} className="block">
                <div className="relative aspect-[4/3] bg-shell">
                  {album.coverMediaId ? (
                    <MediaThumb
                      mediaId={album.coverMediaId}
                      alt={album.name}
                      className="size-full object-cover transition duration-700 group-hover:scale-[1.04]"
                    />
                  ) : (
                    <div className="grid size-full place-items-center text-ink-muted">
                      <Layers size={26} />
                    </div>
                  )}
                </div>
              </Link>
              <div className="flex items-start justify-between gap-2 p-4">
                <Link to={`/albums/${album.id}`} className="min-w-0 flex-1">
                  {editing?.id === album.id ? (
                    <input
                      autoFocus
                      value={editName}
                      onClick={(event) => event.preventDefault()}
                      onChange={(event) => setEditName(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") void rename();
                        if (event.key === "Escape") setEditing(null);
                      }}
                      className="w-full rounded-lg border border-honey/50 bg-canvas px-2 py-1 text-xs font-extrabold text-ink outline-none"
                    />
                  ) : (
                    <p className="truncate text-xs font-extrabold text-ink">{album.name}</p>
                  )}
                  <p className="mt-0.5 truncate text-[11px] text-ink-muted">
                    {formatCount(album.itemCount, "item")} · {relativeTime(album.updatedAt)}
                  </p>
                </Link>
                <div className={cn("flex shrink-0 gap-1", editing?.id === album.id && "hidden")}>
                  <button
                    onClick={() => {
                      setEditing(album);
                      setEditName(album.name);
                    }}
                    className="icon-button !h-7 !w-7"
                    aria-label={`Rename ${album.name}`}
                  >
                    <Pencil size={12} />
                  </button>
                  <button
                    onClick={() => remove(album)}
                    className="icon-button !h-7 !w-7"
                    aria-label={`Delete ${album.name}`}
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
