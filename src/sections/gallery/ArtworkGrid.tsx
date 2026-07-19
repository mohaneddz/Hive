import { ArrowRight } from "lucide-react";

import { ArtworkCard } from "@/components/gallery/ArtworkCard";
import type { Artwork } from "@/types/gallery";

export function ArtworkGrid({ artworks, title = "New arrivals" }: { artworks: Artwork[]; title?: string }) {
  return (
    <section>
      <div className="mb-4 flex items-end justify-between">
        <div><p className="eyebrow">Selected works</p><h2 className="mt-1 text-xl font-extrabold tracking-[-.025em] text-ink">{title}</h2></div>
        <button className="flex items-center gap-1.5 text-xs font-extrabold text-honey-deep transition hover:text-ink">View all <ArrowRight size={14} /></button>
      </div>
      <div className="grid grid-cols-3 gap-5">{artworks.map((artwork) => <ArtworkCard key={artwork.id} artwork={artwork} />)}</div>
    </section>
  );
}
