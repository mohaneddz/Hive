import { AlertTriangle, Archive, Download, HardDrive, Scissors } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { DuplicatesPanel } from "@/components/duplicates/DuplicatesPanel";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { useLibraryStats } from "@/hooks/useLibraryStats";
import { formatBytes } from "@/utils/format";

const upcoming = [
  { icon: Scissors, label: "Compressors", description: "Shrink large photos and videos without losing much quality." },
  { icon: Archive, label: "Batch tools", description: "Rename, tag, or move many files at once." },
  { icon: Download, label: "Export", description: "Export albums or selections to another folder or drive." },
  { icon: HardDrive, label: "Backup", description: "Mirror your library to an external drive." },
  { icon: AlertTriangle, label: "Missing / broken media", description: "Surface files that failed to index or whose source went missing." },
];

export function UtilitiesPage() {
  const stats = useLibraryStats();

  return (
    <div>
      <GalleryPageHeader
        eyebrow="Utilities"
        title="Tools for your library."
        description="Most of these ship alongside the AI pipeline. Storage analysis works today."
      />

      <Card className="mt-7 flex items-center gap-4 p-6">
        <div className="grid size-11 place-items-center rounded-xl bg-cream text-honey-deep">
          <HardDrive size={20} />
        </div>
        <div>
          <p className="text-sm font-extrabold text-ink">Storage analyzer</p>
          <p className="mt-0.5 text-xs text-ink-muted">
            {stats
              ? `${formatBytes(stats.totalBytes)} across ${stats.totalItems.toLocaleString()} indexed items.`
              : "Add a folder to see storage usage."}
          </p>
        </div>
      </Card>

      <DuplicatesPanel />

      <h2 className="mt-8 text-sm font-extrabold text-ink">Coming soon</h2>
      <div className="mt-4 grid grid-cols-3 gap-4">
        {upcoming.map((tool) => (
          <Card key={tool.label} className="flex flex-col gap-3 p-5 opacity-70">
            <div className="grid size-9 place-items-center rounded-xl bg-shell text-ink-muted">
              <tool.icon size={17} />
            </div>
            <div>
              <p className="text-sm font-extrabold text-ink">{tool.label}</p>
              <p className="mt-1 text-[11px] text-ink-muted">{tool.description}</p>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
