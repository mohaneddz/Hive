import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Download, Image, Layers, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { MediaGrid } from "@/components/media/MediaGrid";
import {
  BestPhotoButton,
  BestPhotoHero,
  RankBadge,
  useBestPhoto,
} from "@/components/media/BestPhoto";
import {
  exportMedia,
  getAlbum,
  getMediaPage,
  isTauri,
  removeMediaFromAlbum,
  setAlbumCover,
} from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { routes } from "@/config/routes";
import type { Album, MediaItem } from "@/types/media";
import { formatCount } from "@/utils/format";

export function AlbumDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [album, setAlbum] = useState<Album | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const best = useBestPhoto();

  const load = useCallback(async () => {
    if (!id || !isTauri()) return;
    setLoading(true);
    try {
      const [albumDetail, page] = await Promise.all([
        getAlbum(id),
        getMediaPage({ limit: 500, offset: 0, albumId: id, scope: "all" }),
      ]);
      setAlbum(albumDetail);
      setItems(page.items);
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    void load();
  }, [load]);

  const applyFavorite = useCallback((mediaId: string, favorite: boolean) => {
    setItems((prev) =>
      prev.map((item) => (item.id === mediaId ? { ...item, isFavorite: favorite } : item)),
    );
  }, []);

  const toggleSelect = (mediaId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(mediaId)) next.delete(mediaId);
      else next.add(mediaId);
      return next;
    });
  };

  const removeSelected = async () => {
    if (!id || selected.size === 0) return;
    const confirmed = await confirm(
      `Remove ${formatCount(selected.size, "item")} from this album? The files stay in your library.`,
      { title: "Remove from album", kind: "warning" },
    );
    if (!confirmed) return;

    for (const mediaId of selected) {
      await removeMediaFromAlbum(id, mediaId);
    }
    setItems((prev) => prev.filter((item) => !selected.has(item.id)));
    setSelected(new Set());
    setSelecting(false);
  };

  const exportSelected = async () => {
    if (selected.size === 0) return;
    const destination = await open({
      directory: true,
      multiple: false,
      title: "Export selected items to…",
    });
    if (typeof destination !== "string") return;

    const report = await exportMedia([...selected], destination);
    await confirm(
      `${report.exported} exported, ${report.skipped} skipped.`,
      { title: "Export finished", kind: "info" },
    );
  };

  /**
   * Ranks photos on sharpness, looks, and how representative each one is of the
   * set, then puts the winner first and marks it. Nothing is deleted — this only
   * ever reorders and highlights.
   *
   * A selection wins over the album: "which of these eight shots do I keep" is
   * the question this answers well, and it is rarely a whole album that is being
   * asked about.
   */
  const findBestPhoto = async () => {
    const pool = selected.size >= 2 ? items.filter((item) => selected.has(item.id)) : items;
    const winnerId = await best.rank(pool);
    if (!winnerId) return;

    setItems((prev) => {
      const winner = prev.find((item) => item.id === winnerId);
      if (!winner) return prev;
      return [winner, ...prev.filter((item) => item.id !== winnerId)];
    });
    // The badges live in the hover overlay, which selection mode replaces, so
    // step out of it — otherwise the result would be invisible.
    setSelecting(false);
    setSelected(new Set());
  };

  const rankButton = (
    <BestPhotoButton
      picking={best.picking}
      disabled={selecting && selected.size < 2}
      label={selecting ? "Best of selection" : "Best photo"}
      onClick={findBestPhoto}
    />
  );

  const makeCover = async (mediaId: string) => {
    if (!id) return;
    await setAlbumCover(id, mediaId);
    setAlbum((prev) => (prev ? { ...prev, coverMediaId: mediaId } : prev));
  };

  if (loading && !album) {
    return <div className="grid h-full place-items-center text-sm text-ink-muted">Loading…</div>;
  }

  if (!album) {
    return (
      <EmptyState
        icon={<Layers size={22} />}
        title="Album not found"
        description="It may have been deleted."
        action={<Button onClick={() => navigate(routes.collections.path)}>Back to collections</Button>}
      />
    );
  }

  return (
    <div>
      <button
        onClick={() => navigate(routes.collections.path)}
        className="mb-5 inline-flex items-center gap-1.5 text-xs font-bold text-ink-muted transition hover:text-ink"
      >
        <ArrowLeft size={14} /> All collections
      </button>

      <GalleryPageHeader
        eyebrow="Album"
        title={`${album.name}.`}
        description={album.description ?? `${formatCount(album.itemCount, "item")} in this album.`}
        action={
          items.length > 0 ? (
            selecting ? (
              <div className="flex items-center gap-2">
                {items.length > 1 && rankButton}
                <Button variant="secondary" icon={<Download size={15} />} onClick={exportSelected} disabled={selected.size === 0}>
                  Export
                </Button>
                <Button variant="secondary" icon={<Trash2 size={15} />} onClick={removeSelected} disabled={selected.size === 0}>
                  Remove
                </Button>
                <Button
                  variant="ghost"
                  icon={<X size={15} />}
                  onClick={() => {
                    setSelecting(false);
                    setSelected(new Set());
                  }}
                >
                  Done
                </Button>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                {items.length > 1 && rankButton}
                <Button variant="secondary" onClick={() => setSelecting(true)}>
                  Select
                </Button>
              </div>
            )
          ) : undefined
        }
      />

      {!selecting && (
        <BestPhotoHero
          ranking={best.ranking}
          scope={best.ranking.length < items.length ? "you selected" : "in this album"}
          onDismiss={best.clear}
        />
      )}

      {selecting && (
        <p className="mt-5 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
          {selected.size === 0
            ? "Tap items to select them."
            : `${formatCount(selected.size, "item")} selected.`}
        </p>
      )}

      {items.length === 0 && !loading ? (
        <EmptyState
          icon={<Layers size={22} />}
          title="This album is empty"
          description="Open any photo from the gallery and use “Add to album” to file it here."
          action={<Button onClick={() => navigate(routes.gallery.path)}>Browse the gallery</Button>}
        />
      ) : (
        <MediaGrid
          items={items}
          className="mt-7"
          onFavoriteChange={applyFavorite}
          selected={selecting ? selected : undefined}
          onToggleSelect={selecting ? toggleSelect : undefined}
          renderOverlay={
            selecting
              ? undefined
              : (item) => {
                  const ranked = best.byMedia.get(item.id);
                  return (
                    <>
                      {ranked && <RankBadge entry={ranked} />}
                      <button
                        onClick={(event) => {
                          event.preventDefault();
                          void makeCover(item.id);
                        }}
                        className="icon-button bg-white/15 text-white backdrop-blur-md"
                        aria-label="Use as album cover"
                        title="Use as album cover"
                      >
                        <Image size={14} />
                      </button>
                    </>
                  );
                }
          }
        />
      )}
    </div>
  );
}
