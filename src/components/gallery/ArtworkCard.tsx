import { Heart, Maximize2 } from "lucide-react";

import type { Artwork } from "@/types/gallery";
import { cn } from "@/utils/cn";

export function ArtworkCard({ artwork, featured = false }: { artwork: Artwork; featured?: boolean }) {
  return (
    <article className={cn("group relative", featured && "md:col-span-2")}>
      <div className={cn("artwork-frame", featured ? "aspect-[1.74/1]" : "aspect-[.84/1]")}>
        <div className={cn("absolute inset-0 bg-gradient-to-br", artwork.palette)} />
        <img
          src={artwork.image}
          alt={`${artwork.title} by ${artwork.artist}`}
          className="relative size-full object-cover transition duration-700 group-hover:scale-[1.035]"
        />
        <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/55 to-transparent opacity-0 transition group-hover:opacity-100" />
        <div className="absolute inset-x-0 bottom-0 flex translate-y-2 items-end justify-between p-4 opacity-0 transition duration-300 group-hover:translate-y-0 group-hover:opacity-100">
          <p className="text-xs font-bold text-white">View artwork</p>
          <button className="icon-button bg-white/15 text-white backdrop-blur-md" aria-label={`View ${artwork.title}`}>
            <Maximize2 size={15} />
          </button>
        </div>
        <button className="absolute right-3 top-3 grid size-9 place-items-center rounded-full bg-white/86 text-ink shadow-sm backdrop-blur transition hover:scale-105" aria-label={`Save ${artwork.title}`}>
          <Heart size={16} className={cn(artwork.saved && "fill-honey text-honey")} />
        </button>
      </div>
      <div className="flex items-start justify-between gap-3 px-1 pt-3">
        <div>
          <h3 className="text-sm font-extrabold tracking-[-.01em] text-ink">{artwork.title}</h3>
          <p className="mt-0.5 text-xs text-ink-muted">{artwork.artist} <span className="mx-1 text-ink-muted/50">•</span> {artwork.year}</p>
        </div>
        <span className="pt-0.5 text-[10px] font-bold uppercase tracking-[.11em] text-ink-muted">{artwork.medium.split(" ")[0]}</span>
      </div>
    </article>
  );
}
