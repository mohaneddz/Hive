import { Loader2 } from "lucide-react";

import { useJobProgress } from "@/hooks/useJobProgress";

export function JobIndicator() {
  const jobs = useJobProgress();
  const active = jobs.find((job) => job.status === "running");
  if (!active) return null;

  return (
    <div className="flex items-center gap-2 rounded-full border border-honey/30 bg-cream/55 px-3 py-1 text-[11px] font-bold text-honey-deep">
      <Loader2 size={12} className="animate-spin" />
      {active.kind === "scan_folder" ? "Indexing" : active.kind} {active.current}/{active.total}
    </div>
  );
}
