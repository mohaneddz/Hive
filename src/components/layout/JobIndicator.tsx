import { Loader2, X } from "lucide-react";

import { useJobProgress } from "@/hooks/useJobProgress";
import { cancelJob, isTauri } from "@/lib/tauri";

export function JobIndicator() {
  const jobs = useJobProgress();
  const active = jobs.find((job) => job.status === "running");
  if (!active) return null;

  const handleCancel = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isTauri()) {
      void cancelJob(active.id);
    }
  };

  return (
    <div className="flex items-center gap-2 rounded-full border border-honey/30 bg-cream/55 px-3 py-1 text-[11px] font-bold text-honey-deep">
      <Loader2 size={12} className="animate-spin" />
      <span>
        {active.kind === "scan_folder" ? "Indexing" : active.kind} {active.current}/{active.total}
      </span>
      <button
        onClick={handleCancel}
        className="ml-0.5 rounded-full p-0.5 hover:bg-honey/20 transition-colors text-honey-deep"
        title="Cancel indexing"
        aria-label="Cancel indexing"
      >
        <X size={12} />
      </button>
    </div>
  );
}
