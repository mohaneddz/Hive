import { Search } from "lucide-react";


export function DiscoverHeader() {
  return (
    <header className="flex items-start justify-between gap-6">
      <div>
        <p className="eyebrow">Curated for you</p>
        <h1 className="mt-2 text-[32px] font-extrabold tracking-[-.04em] text-ink">Art that stays with you.</h1>
        <p className="mt-2 text-sm text-ink-muted">A quiet selection of contemporary work from around the world.</p>
      </div>
      <div className="flex items-center gap-3">
        <label className="relative hidden w-[250px] lg:block">
          <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-muted" size={16} />
          <span className="sr-only">Search art and artists</span>
          <input className="search-input" placeholder="Search art & artists" />
        </label>
      </div>
    </header>
  );
}
