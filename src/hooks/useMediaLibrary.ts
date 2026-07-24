import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "@/lib/tauri";
import type { Folder, JobProgress, MediaItem } from "@/types/media";

const PAGE_SIZE = 60;

export function useMediaLibrary() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [jobs, setJobs] = useState<Record<string, JobProgress>>({});

  const refreshFolders = useCallback(async () => {
    if (!api.isTauri()) return;
    setFolders(await api.listFolders());
  }, []);

  const loadPage = useCallback(
    async (offset = 0, options?: { mediaType?: string; favoritesOnly?: boolean }) => {
      if (!api.isTauri()) return;
      setLoading(true);
      try {
        const page = await api.getMediaPage({
          limit: PAGE_SIZE,
          offset,
          mediaType: options?.mediaType,
          favoritesOnly: options?.favoritesOnly,
        });
        setItems((prev) => (offset === 0 ? page.items : [...prev, ...page.items]));
        setTotal(page.total);
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  const addFolder = useCallback(
    async (path: string) => {
      const folder = await api.addWatchedFolder(path);
      await refreshFolders();
      const result = await api.scanFolder(folder.id);
      await loadPage(0);
      return result;
    },
    [refreshFolders, loadPage],
  );

  const removeFolder = useCallback(
    async (folderId: string) => {
      await api.removeWatchedFolder(folderId);
      await refreshFolders();
      await loadPage(0);
    },
    [refreshFolders, loadPage],
  );

  const rescan = useCallback(
    async (folderId: string) => {
      await api.scanFolder(folderId);
      await loadPage(0);
    },
    [loadPage],
  );

  useEffect(() => {
    void refreshFolders();
    void loadPage(0);
  }, [refreshFolders, loadPage]);

  useEffect(() => {
    if (!api.isTauri()) return;
    const unlistenProgress = listen<JobProgress>("job:progress", (event) => {
      const progress = event.payload;
      setJobs((prev) => ({ ...prev, [progress.id]: progress }));
      if (progress.status !== "running") {
        setTimeout(() => {
          setJobs((prev) => {
            const next = { ...prev };
            delete next[progress.id];
            return next;
          });
        }, 2000);
      }
    });
    const unlistenChanged = listen("media:changed", () => {
      void loadPage(0);
    });
    return () => {
      void unlistenProgress.then((dispose) => dispose());
      void unlistenChanged.then((dispose) => dispose());
    };
  }, [loadPage]);

  return {
    folders,
    items,
    total,
    loading,
    jobs: Object.values(jobs),
    addFolder,
    removeFolder,
    rescan,
    loadPage,
    refreshFolders,
  };
}
