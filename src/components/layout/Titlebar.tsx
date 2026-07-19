import { Minus, Square, X } from "lucide-react";

import { appConfig } from "@/config/app";
import { useWindowControls } from "@/hooks/useWindowControls";
import { HiveMark } from "@/components/brand/HiveMark";

export function Titlebar() {
  const { close, isMaximized, minimize, toggleMaximize } = useWindowControls();

  return (
    <header
      data-tauri-drag-region
      className="titlebar flex h-10 shrink-0 select-none items-center border-b border-ink/[.07] bg-shell pl-3"
    >
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center gap-2"
      >
        <HiveMark className="size-6 rounded-lg shadow-none" />
        <span
          data-tauri-drag-region
          className="text-xs font-extrabold tracking-[.08em] text-ink"
        >
          {appConfig.name}
        </span>
      </div>

      <div className="flex h-full">
        <button
          className="window-control"
          onClick={minimize}
          aria-label="Minimize window"
        >
          <Minus size={15} strokeWidth={1.7} />
        </button>
        <button
          className="window-control"
          onClick={toggleMaximize}
          aria-label={isMaximized ? "Restore window" : "Maximize window"}
        >
          <Square size={isMaximized ? 12 : 11} strokeWidth={1.7} />
        </button>
        <button
          className="window-control window-control-close"
          onClick={close}
          aria-label="Close window"
        >
          <X size={16} strokeWidth={1.7} />
        </button>
      </div>
    </header>
  );
}
