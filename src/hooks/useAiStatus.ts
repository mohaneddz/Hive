import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { getAiStatus, isTauri } from "@/lib/tauri";
import type { AiStatus } from "@/types/media";

export function useAiStatus() {
  const [status, setStatus] = useState<AiStatus | null>(null);

  const refresh = useCallback(() => {
    if (!isTauri()) return;
    void getAiStatus().then(setStatus);
  }, []);

  useEffect(() => {
    refresh();
    if (!isTauri()) return;
    const unlistenProgress = listen("job:progress", refresh);
    const unlistenChanged = listen("media:changed", refresh);
    return () => {
      void unlistenProgress.then((dispose) => dispose());
      void unlistenChanged.then((dispose) => dispose());
    };
  }, [refresh]);

  return { status, refresh };
}
