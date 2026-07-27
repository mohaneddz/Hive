import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { isTauri } from "@/lib/tauri";

export interface DownloadProgress {
  done: number;
  total: number;
  percent: number;
}

/**
 * Follows one model download.
 *
 * The download commands emit `(bytes_done, bytes_total)` on their own channel
 * instead of creating a job row, so without a listener a several-hundred-megabyte
 * download looks exactly like a button that does nothing.
 *
 * @param event the channel, e.g. `"nsfw:download:progress"`
 */
export function useDownloadProgress(event: string): DownloadProgress | null {
  const [progress, setProgress] = useState<DownloadProgress | null>(null);

  useEffect(() => {
    if (!isTauri()) return;

    const subscription = listen<[number, number]>(event, ({ payload }) => {
      const [done, total] = payload;
      setProgress({
        done,
        total,
        // `total` comes from a HEAD request and can be 0 when the server sends
        // no content-length; guard so the bar never shows NaN.
        percent: total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0,
      });

      // Clear once complete so the bar does not linger at 100%.
      if (total > 0 && done >= total) {
        setTimeout(() => setProgress(null), 1500);
      }
    });

    return () => {
      void subscription.then((dispose) => dispose());
    };
  }, [event]);

  return progress;
}
