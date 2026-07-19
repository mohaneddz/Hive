import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export function useWindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);

  const syncMaximized = useCallback(async () => {
    if (!isTauri()) return;
    setIsMaximized(await getCurrentWindow().isMaximized());
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void syncMaximized();
    const unlisten = getCurrentWindow().onResized(syncMaximized);
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [syncMaximized]);

  return {
    isMaximized,
    minimize: () => isTauri() && void getCurrentWindow().minimize(),
    toggleMaximize: () =>
      isTauri() && void getCurrentWindow().toggleMaximize().then(syncMaximized),
    close: () => isTauri() && void getCurrentWindow().close(),
  };
}
