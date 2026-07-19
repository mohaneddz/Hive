import { artists, artworks } from "@/data/gallery";
import { ArtistCard } from "@/components/gallery/ArtistCard";
import { ArtworkCard } from "@/components/gallery/ArtworkCard";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function ArtistsPage() {
  return <div><GalleryPageHeader eyebrow="The artists" title="Meet the makers." description="Follow the people, practices, and perspectives behind the work." /><section className="mt-7 grid grid-cols-3 gap-4">{artists.map((artist) => <ArtistCard key={artist.id} artist={artist} />)}</section><section className="mt-10"><p className="eyebrow">From their studios</p><h2 className="mt-1 text-xl font-extrabold tracking-[-.025em] text-ink">Artist highlights</h2><div className="mt-5 grid grid-cols-3 gap-5">{artworks.slice(3).map((artwork) => <ArtworkCard key={artwork.id} artwork={artwork} />)}</div></section></div>;
}
