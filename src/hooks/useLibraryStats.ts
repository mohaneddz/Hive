import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getLibraryStats, isTauri } from "@/lib/tauri";
import type { LibraryStats } from "@/types/media";

export function useLibraryStats() {
  const [stats, setStats] = useState<LibraryStats | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const refresh = () => {
      void getLibraryStats().then(setStats);
    };
    refresh();
    const unlisten = listen("media:changed", refresh);
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  return stats;
}
