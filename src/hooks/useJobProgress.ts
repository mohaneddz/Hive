import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { isTauri } from "@/lib/tauri";
import type { JobProgress } from "@/types/media";

export function useJobProgress() {
  const [jobs, setJobs] = useState<Record<string, JobProgress>>({});

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen<JobProgress>("job:progress", (event) => {
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
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  return Object.values(jobs);
}
