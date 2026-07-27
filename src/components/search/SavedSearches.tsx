import { useEffect, useState } from "react";
import { LazyStore } from "@tauri-apps/plugin-store";
import { Bookmark, Plus, X } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { isTauri } from "@/lib/tauri";

export interface SavedSearch {
  id: string;
  label: string;
  query: string;
  mode: "exact" | "semantic";
}

const STORE_KEY = "searches";
let store: LazyStore | null = null;

function getStore(): LazyStore {
  store ??= new LazyStore("saved-searches.json");
  return store;
}

export function SavedSearchesPanel({
  current,
  onApply,
}: {
  /** The query+mode that "Save this search" would persist; null while there's nothing worth saving. */
  current: { query: string; mode: "exact" | "semantic" } | null;
  onApply: (search: SavedSearch) => void;
}) {
  const [searches, setSearches] = useState<SavedSearch[] | null>(null);
  const [naming, setNaming] = useState(false);
  const [label, setLabel] = useState("");

  useEffect(() => {
    if (!isTauri()) return;
    void getStore()
      .get<SavedSearch[]>(STORE_KEY)
      .then((saved) => setSearches(saved ?? []));
  }, []);

  const persist = async (next: SavedSearch[]) => {
    setSearches(next);
    const s = getStore();
    await s.set(STORE_KEY, next);
    await s.save();
  };

  const save = async () => {
    if (!current || !label.trim()) return;
    const entry: SavedSearch = {
      id: crypto.randomUUID(),
      label: label.trim(),
      query: current.query,
      mode: current.mode,
    };
    await persist([entry, ...(searches ?? [])]);
    setLabel("");
    setNaming(false);
  };

  const remove = async (id: string) => {
    await persist((searches ?? []).filter((s) => s.id !== id));
  };

  if (searches === null) return null;

  return (
    <Card className="p-4">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Bookmark size={14} className="text-honey-deep" />
          <h2 className="text-sm font-extrabold text-ink">Saved searches</h2>
        </div>
        {current && !naming && (
          <button
            onClick={() => setNaming(true)}
            className="inline-flex items-center gap-1 text-[11px] font-bold text-honey-deep hover:underline"
          >
            <Plus size={12} /> Save current
          </button>
        )}
      </div>

      {naming && (
        <div className="mt-3 flex items-center gap-1.5">
          <input
            autoFocus
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void save();
              if (event.key === "Escape") setNaming(false);
            }}
            placeholder="Name this search"
            className="h-8 flex-1 rounded-lg border border-ink/[.12] bg-canvas px-2.5 text-xs text-ink outline-none focus:border-honey/50"
          />
          <button
            onClick={save}
            disabled={!label.trim()}
            className="rounded-lg bg-honey px-2.5 py-1.5 text-[11px] font-extrabold text-[#3b2900] disabled:opacity-50"
          >
            Save
          </button>
        </div>
      )}

      {searches.length === 0 ? (
        <p className="mt-2 text-[11px] text-ink-muted">
          Run a search, then save it here to get back to it in one click.
        </p>
      ) : (
        <ul className="mt-2 space-y-1">
          {searches.map((search) => (
            <li key={search.id} className="group flex items-center gap-1.5">
              <button
                onClick={() => onApply(search)}
                className="min-w-0 flex-1 truncate rounded-lg px-2 py-1.5 text-left text-xs font-bold text-ink transition hover:bg-honey/10"
                title={search.query}
              >
                {search.label}
              </button>
              <button
                onClick={() => remove(search.id)}
                className="rounded-lg p-1 text-ink-muted opacity-0 transition hover:text-ink group-hover:opacity-100"
                aria-label={`Delete saved search "${search.label}"`}
              >
                <X size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
