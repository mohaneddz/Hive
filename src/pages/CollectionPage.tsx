import { Search, SlidersHorizontal, SortAsc, X } from "lucide-react";
import { useMemo, useState } from "react";

import { artworks } from "@/data/gallery";
import { ArtworkCard } from "@/components/gallery/ArtworkCard";
import { Button } from "@/components/ui/Button";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function CollectionPage() {
  const [query, setQuery] = useState(""); const [sort, setSort] = useState("recent"); const [medium, setMedium] = useState("All mediums");
  const mediums = ["All mediums", ...new Set(artworks.map((item) => item.medium.split(" ")[0]))];
  const visible = useMemo(() => artworks.filter((item) => `${item.title} ${item.artist} ${item.medium}`.toLowerCase().includes(query.toLowerCase())).filter((item) => medium === "All mediums" || item.medium.startsWith(medium)).sort((a,b) => sort === "title" ? a.title.localeCompare(b.title) : b.year.localeCompare(a.year)), [query, sort, medium]);
  return <div><GalleryPageHeader eyebrow="The collection" title="Every work, in one place." description="Browse paintings, photography, sculpture, and works on paper." action={<Button variant="secondary" icon={<SlidersHorizontal size={15} />}>{visible.length} works</Button>} /><div className="mt-7 flex flex-wrap items-center gap-3"><label className="relative min-w-[240px] flex-1"><Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-muted" /><input value={query} onChange={(e) => setQuery(e.target.value)} className="search-input" placeholder="Search title, artist, or medium" /></label><select value={medium} onChange={(e) => setMedium(e.target.value)} className="h-[42px] rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink outline-none">{mediums.map((item) => <option key={item}>{item}</option>)}</select><button onClick={() => setSort(sort === "recent" ? "title" : "recent")} className="inline-flex h-[42px] items-center gap-2 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs font-bold text-ink"><SortAsc size={15} /> {sort === "recent" ? "Newest" : "A–Z"}</button>{(query || medium !== "All mediums") && <button onClick={() => { setQuery(""); setMedium("All mediums"); }} className="icon-button" aria-label="Clear filters"><X size={15} /></button>}</div><div className="mt-7 grid grid-cols-3 gap-x-5 gap-y-8">{visible.map((artwork) => <ArtworkCard key={artwork.id} artwork={artwork} />)}</div>{visible.length === 0 && <div className="mt-12 rounded-3xl border border-dashed border-ink/[.15] p-12 text-center text-sm text-ink-muted">No works match those filters.</div>}</div>;
}
