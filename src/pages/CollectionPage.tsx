import { SlidersHorizontal } from "lucide-react";

import { artworks } from "@/data/gallery";
import { ArtworkCard } from "@/components/gallery/ArtworkCard";
import { Button } from "@/components/ui/Button";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function CollectionPage() {
  return <div><GalleryPageHeader eyebrow="The collection" title="Every work, in one place." description="Browse paintings, photography, sculpture, and works on paper." action={<Button variant="secondary" icon={<SlidersHorizontal size={15} />}>Filter works</Button>} /><div className="mt-7 grid grid-cols-3 gap-x-5 gap-y-8">{artworks.map((artwork) => <ArtworkCard key={artwork.id} artwork={artwork} />)}</div></div>;
}
