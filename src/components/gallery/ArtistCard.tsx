import { ArrowUpRight } from "lucide-react";

import type { Artist } from "@/types/gallery";

export function ArtistCard({ artist }: { artist: Artist }) {
  return (
    <article className="group flex items-center gap-4 rounded-[20px] border border-ink/[.08] bg-panel p-3 transition hover:-translate-y-0.5 hover:border-honey/40 hover:shadow-[0_12px_30px_rgba(75,52,10,.08)]">
      <img src={artist.portrait} alt={artist.name} className="size-14 rounded-2xl object-cover" />
      <div className="min-w-0 flex-1">
        <h3 className="truncate text-sm font-extrabold text-ink">{artist.name}</h3>
        <p className="mt-0.5 text-[11px] text-ink-muted">{artist.discipline} · {artist.location}</p>
        <p className="mt-1.5 text-[10px] font-bold uppercase tracking-[.1em] text-honey-deep">{artist.artworkCount} works</p>
      </div>
      <ArrowUpRight size={17} className="text-ink-muted transition group-hover:text-ink" />
    </article>
  );
}
