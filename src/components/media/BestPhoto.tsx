import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Loader2, Trophy, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { MediaThumb } from "@/components/media/MediaThumb";
import { selectBestPhoto } from "@/lib/tauri";
import type { MediaItem, PhotoRanking } from "@/types/media";
import { formatCount } from "@/utils/format";

const percent = (value: number) => `${Math.round(value * 100)}%`;

/**
 * The three signals in words. Every score is relative to the photos that were
 * ranked together, so "88%" means "near the best of this set", not "good".
 */
export const breakdown = (scores: PhotoRanking) =>
  `Sharpness ${percent(scores.sharpnessScore)} · Looks ${percent(scores.aestheticScore)}` +
  ` · Fits the set ${percent(scores.representativenessScore)}`;

export interface RankedEntry {
  position: number;
  scores: PhotoRanking;
}

/**
 * Ranking state for one grid. Kept apart from any page so albums, smart albums
 * and anything else that shows a grid can offer the same thing without copying
 * the wiring — the earlier version lived inside the album page, which is why a
 * smart album had no way to ask.
 */
export function useBestPhoto() {
  const [ranking, setRanking] = useState<PhotoRanking[]>([]);
  const [picking, setPicking] = useState(false);

  const byMedia = useMemo(() => {
    const map = new Map<string, RankedEntry>();
    ranking.forEach((scores, index) => map.set(scores.mediaId, { position: index + 1, scores }));
    return map;
  }, [ranking]);

  /** Ranks `pool`, returning the winner's id, or null if it could not run. */
  const rank = useCallback(async (pool: MediaItem[]): Promise<string | null> => {
    if (pool.length < 2) return null;
    setPicking(true);
    try {
      const result = await selectBestPhoto(pool.map((item) => item.id));
      setRanking(result.rankings);
      return result.bestMediaId;
    } catch (cause) {
      await confirm(String(cause), { title: "Could not rank these photos", kind: "error" });
      return null;
    } finally {
      setPicking(false);
    }
  }, []);

  const clear = useCallback(() => setRanking([]), []);

  return { ranking, byMedia, winner: ranking[0] ?? null, picking, rank, clear };
}

export function BestPhotoButton({
  picking,
  disabled,
  label,
  onClick,
}: {
  picking: boolean;
  disabled?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      variant="secondary"
      icon={picking ? <Loader2 size={15} className="animate-spin" /> : <Trophy size={15} />}
      onClick={onClick}
      disabled={picking || disabled}
    >
      {picking ? "Ranking…" : label}
    </Button>
  );
}

/** One signal as a labelled bar, so the numbers can be compared at a glance. */
function ScoreBar({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-24 shrink-0 text-[11px] font-bold text-ink-soft">{label}</span>
      <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-ink/10">
        <span
          className="block h-full rounded-full bg-honey"
          style={{ width: `${Math.max(2, Math.round(value * 100))}%` }}
        />
      </span>
      <span className="w-9 shrink-0 text-right text-[11px] font-extrabold text-ink">
        {percent(value)}
      </span>
    </div>
  );
}

/**
 * The winner, shown large with its reasoning.
 *
 * Moving it to the front of a grid of identical tiles was too quiet an answer to
 * "which one is best" — the badge was easy to miss entirely. Here the photo is
 * the size of the claim being made about it, and the three bars say why it won
 * without anyone having to hover.
 *
 * `scope` finishes the sentence "Best of 8 photos …", e.g. "you selected".
 */
export function BestPhotoHero({
  ranking,
  scope,
  onDismiss,
}: {
  ranking: PhotoRanking[];
  scope: string;
  onDismiss: () => void;
}) {
  const winner = ranking[0];
  if (!winner) return null;

  return (
    <section className="mt-6 overflow-hidden rounded-[22px] border border-honey/30 bg-cream/45">
      <div className="flex flex-wrap items-start gap-6 p-5">
        <Link
          to={`/media/${winner.mediaId}`}
          className="artwork-frame block size-44 shrink-0"
          title="Open this photo"
        >
          <MediaThumb
            mediaId={winner.mediaId}
            variant="md"
            alt="Best photo"
            className="size-full object-cover"
          />
        </Link>

        <div className="min-w-56 flex-1">
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-2 text-honey-deep">
              <span className="grid size-7 place-items-center rounded-full bg-honey text-[#3b2900]">
                <Trophy size={14} />
              </span>
              <div>
                <p className="text-sm font-extrabold text-ink">Best photo</p>
                <p className="text-[11px] font-bold text-ink-muted">
                  of {formatCount(ranking.length, "photo")} {scope}
                </p>
              </div>
            </div>
            <button onClick={onDismiss} className="icon-button !h-7 !w-7" aria-label="Clear ranking">
              <X size={13} />
            </button>
          </div>

          <div className="mt-4 space-y-2">
            <ScoreBar label="Sharpness" value={winner.sharpnessScore} />
            <ScoreBar label="Looks" value={winner.aestheticScore} />
            <ScoreBar label="Fits the set" value={winner.representativenessScore} />
          </div>

          <p className="mt-3.5 text-[11px] leading-relaxed text-ink-muted">
            Scores compare these photos to each other, not to your library — the best of a poor set
            still scores high. The rest are ranked below; hover any of them for its own numbers.
          </p>
        </div>
      </div>
    </section>
  );
}

/** The pill shown on a ranked photo: gold for the winner, plain for the rest. */
export function RankBadge({ entry }: { entry: RankedEntry }) {
  const first = entry.position === 1;
  return (
    <span
      className={
        first
          ? "inline-flex h-9 items-center gap-1.5 rounded-full bg-honey px-3 text-[11px] font-extrabold text-[#3b2900]"
          : "inline-flex h-9 items-center rounded-full bg-black/55 px-3 text-[11px] font-extrabold text-white backdrop-blur-md"
      }
      title={breakdown(entry.scores)}
    >
      {first && <Trophy size={13} />}
      {first
        ? percent(entry.scores.totalScore)
        : `#${entry.position} · ${percent(entry.scores.totalScore)}`}
    </span>
  );
}
