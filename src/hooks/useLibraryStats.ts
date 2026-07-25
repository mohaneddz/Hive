import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getLibraryStats, isTauri } from "@/lib/tauri";
import type { LibraryStats } from "@/types/media";

/**
 * Library counters, kept live.
 *
 * Listens to two events: `media:changed` when files are added or removed, and
 * `media:flagged` when only a flag moved — favorites, hidden, archived. The
 * second one exists so a heart click refreshes these totals without making
 * grids refetch a whole page.
 */
export function useLibraryStats() {
  const [stats, setStats] = useState<LibraryStats | null>(null);

  const refreshStats = useCallback(async () => {
    if (!isTauri()) return;
    setStats(await getLibraryStats());
  }, []);

  useEffect(() => {
    void refreshStats();
  }, [refreshStats]);

  useEffect(() => {
    if (!isTauri()) return;
    const subscriptions = ["media:changed", "media:flagged"].map((event) =>
      listen(event, () => void refreshStats()),
    );
    return () => {
      subscriptions.forEach((subscription) => void subscription.then((dispose) => dispose()));
    };
  }, [refreshStats]);

  return { stats, refreshStats };
}
