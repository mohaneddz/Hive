import { Heart } from "lucide-react";

import { artworks } from "@/data/gallery";
import { ArtworkCard } from "@/components/gallery/ArtworkCard";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { useGalleryStore } from "@/hooks/useGalleryStore";

export function SavedPage() {
  const { savedIds } = useGalleryStore();
  const saved = artworks.filter((artwork) => savedIds.includes(artwork.id));
  return <div><GalleryPageHeader eyebrow="Your selection" title="Saved works." description="A growing personal collection of pieces you want to return to." action={<span className="inline-flex items-center gap-2 rounded-xl bg-cream px-3 py-2 text-xs font-extrabold text-honey-deep"><Heart size={14} className="fill-honey text-honey" /> {saved.length} saved</span>} /><div className="mt-7 grid grid-cols-3 gap-5">{saved.map((artwork) => <ArtworkCard key={artwork.id} artwork={artwork} />)}</div></div>;
}
