import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { Bell, HelpCircle, Minus, Search, Square, X } from "lucide-react";

import { appConfig } from "@/config/app";
import { useWindowControls } from "@/hooks/useWindowControls";
import { HiveMark } from "@/components/brand/HiveMark";
import { JobIndicator } from "@/components/layout/JobIndicator";

export function Titlebar() {
  const { close, isMaximized, minimize, toggleMaximize } = useWindowControls();
  const navigate = useNavigate();
  const [query, setQuery] = useState("");

  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    if (query.trim()) navigate(`/search?q=${encodeURIComponent(query.trim())}`);
  };

  return (
    <header
      data-tauri-drag-region
      className="titlebar flex h-10 shrink-0 select-none items-center gap-4 border-b border-ink/[.07] bg-shell pl-3 pr-2"
    >
      <div data-tauri-drag-region className="flex shrink-0 items-center gap-2">
        <HiveMark className="size-5" />
        <span className="text-xs font-extrabold tracking-[.08em] text-ink">{appConfig.name}</span>
      </div>

      <form onSubmit={submitSearch} className="min-w-0 max-w-[360px] flex-1">
        <label className="relative block">
          <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-muted" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search your hive"
            className="h-7 w-full rounded-lg border border-ink/[.1] bg-panel/70 pl-8 pr-2 text-[11px] text-ink outline-none placeholder:text-ink-muted focus:border-honey/50"
          />
        </label>
      </form>

      <JobIndicator />

      <div data-tauri-drag-region className="flex-1" />

      <div className="flex items-center gap-1 text-ink-muted">
        <button className="grid size-7 place-items-center rounded-lg transition hover:bg-ink/5 hover:text-ink" title="Notifications">
          <Bell size={14} />
        </button>
        <button className="grid size-7 place-items-center rounded-lg transition hover:bg-ink/5 hover:text-ink" title="Help">
          <HelpCircle size={14} />
        </button>
      </div>

      <div className="flex h-full">
        <button className="window-control" onClick={minimize} aria-label="Minimize window">
          <Minus size={15} strokeWidth={1.7} />
        </button>
        <button
          className="window-control"
          onClick={toggleMaximize}
          aria-label={isMaximized ? "Restore window" : "Maximize window"}
        >
          <Square size={isMaximized ? 12 : 11} strokeWidth={1.7} />
        </button>
        <button className="window-control window-control-close" onClick={close} aria-label="Close window">
          <X size={16} strokeWidth={1.7} />
        </button>
      </div>
    </header>
  );
}
