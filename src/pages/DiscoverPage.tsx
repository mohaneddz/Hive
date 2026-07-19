import { artworks } from "@/data/gallery";
import { ArtworkGrid } from "@/sections/gallery/ArtworkGrid";
import { DiscoverHeader } from "@/sections/gallery/DiscoverHeader";
import { ExhibitionFeature } from "@/sections/gallery/ExhibitionFeature";

export function DiscoverPage() {
  return <div className="space-y-7"><DiscoverHeader /><ExhibitionFeature /><ArtworkGrid artworks={artworks.slice(0, 3)} /></div>;
}
