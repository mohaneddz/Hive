import { useCallback, useEffect, useState } from "react";
import { Copy, Loader2, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { MediaThumb } from "@/components/media/MediaThumb";
import {
  dismissDuplicateGroup,
  getDuplicateGroups,
  isTauri,
  scanDuplicates,
  setTrashed,
} from "@/lib/tauri";
import type { DuplicateGroup } from "@/types/media";
import { formatBytes } from "@/utils/format";

export function DuplicatesPanel() {
  const [groups, setGroups] = useState<DuplicateGroup[] | null>(null);
  const [scanning, setScanning] = useState(false);

  const load = useCallback(() => {
    if (!isTauri()) return;
    getDuplicateGroups().then(setGroups);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const runScan = async () => {
    setScanning(true);
    try {
      await scanDuplicates();
      load();
    } finally {
      setScanning(false);
    }
  };

  const keepOnly = async (group: DuplicateGroup, keepId: string) => {
    await Promise.all(
      group.items.filter((item) => item.id !== keepId).map((item) => setTrashed(item.id, true)),
    );
    await dismissDuplicateGroup(group.groupId);
    load();
  };

  const dismiss = async (groupId: string) => {
    await dismissDuplicateGroup(groupId);
    load();
  };

  return (
    <Card className="mt-5 p-6">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep">
            <Copy size={18} />
          </div>
          <div>
            <p className="text-sm font-extrabold text-ink">Duplicate detection</p>
            <p className="mt-0.5 text-xs text-ink-muted">
              Finds visually near-identical photos by perceptual hash.
            </p>
          </div>
        </div>
        <Button variant="secondary" disabled={scanning} onClick={runScan} icon={scanning ? <Loader2 size={14} className="animate-spin" /> : <Copy size={14} />}>
          {scanning ? "Scanning…" : "Scan for duplicates"}
        </Button>
      </div>

      {groups && groups.length === 0 && (
        <p className="mt-5 text-xs text-ink-muted">No duplicate groups found yet — run a scan.</p>
      )}

      {groups && groups.length > 0 && (
        <div className="mt-5 space-y-4">
          {groups.map((group) => (
            <div key={group.groupId} className="rounded-2xl border border-ink/[.08] bg-canvas p-4">
              <div className="mb-3 flex items-center justify-between">
                <p className="text-xs font-bold text-ink">{group.items.length} similar items</p>
                <button
                  onClick={() => dismiss(group.groupId)}
                  className="inline-flex items-center gap-1 text-[11px] font-bold text-ink-muted hover:text-ink"
                >
                  <X size={12} /> Not duplicates
                </button>
              </div>
              <div className="grid grid-cols-4 gap-3 sm:grid-cols-6">
                {group.items.map((item) => (
                  <div key={item.id} className="group relative">
                    <div className="artwork-frame aspect-square">
                      <MediaThumb mediaId={item.id} alt={item.filename} className="size-full object-cover" />
                    </div>
                    <p className="mt-1 truncate text-[10px] text-ink-muted">{formatBytes(item.size)}</p>
                    <button
                      onClick={() => keepOnly(group, item.id)}
                      className="absolute inset-x-1 top-1 flex items-center justify-center gap-1 rounded-lg bg-black/70 py-1 text-[10px] font-bold text-white opacity-0 transition group-hover:opacity-100"
                    >
                      <Trash2 size={11} /> Keep this
                    </button>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}
