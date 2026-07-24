import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "@/lib/tauri";
import { useJobProgress } from "@/hooks/useJobProgress";
import type { Folder, MediaItem } from "@/types/media";

const PAGE_SIZE = 60;

export function useMediaLibrary() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const jobs = useJobProgress();

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
    const unlistenChanged = listen("media:changed", () => {
      void loadPage(0);
    });
    return () => {
      void unlistenChanged.then((dispose) => dispose());
    };
  }, [loadPage]);

  return {
    folders,
    items,
    total,
    loading,
    jobs,
    addFolder,
    removeFolder,
    rescan,
    loadPage,
    refreshFolders,
  };
}
