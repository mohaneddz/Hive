import { useCallback, useEffect, useState } from "react";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { Archive, Loader2, RotateCcw, ShieldCheck, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import {
  backupLibrary,
  cancelPendingRestore,
  hasPendingRestore,
  inspectBackup,
  isTauri,
  restoreLibrary,
} from "@/lib/tauri";
import type { BackupInfo } from "@/types/media";
import { formatBytes, formatCount, formatDateTime } from "@/utils/format";

export function BackupPanel() {
  const [busy, setBusy] = useState(false);
  const [lastBackup, setLastBackup] = useState<BackupInfo | null>(null);
  const [pending, setPending] = useState(false);

  const refreshPending = useCallback(async () => {
    if (!isTauri()) return;
    setPending(await hasPendingRestore());
  }, []);

  useEffect(() => {
    void refreshPending();
  }, [refreshPending]);

  const runBackup = async () => {
    const destination = await open({
      directory: true,
      multiple: false,
      title: "Save the backup to…",
    });
    if (typeof destination !== "string") return;

    setBusy(true);
    try {
      const info = await backupLibrary(destination);
      setLastBackup(info);
      await confirm(
        `Backup saved: ${formatCount(info.itemCount, "item")}, ${formatBytes(info.bytes)}.`,
        { title: "Backup complete", kind: "info" },
      );
    } finally {
      setBusy(false);
    }
  };

  const runRestore = async () => {
    const picked = await open({
      multiple: false,
      title: "Choose a Hive backup",
      filters: [{ name: "Hive backup", extensions: ["db"] }],
    });
    if (typeof picked !== "string") return;

    setBusy(true);
    try {
      const info = await inspectBackup(picked);
      const confirmed = await confirm(
        `This backup holds ${formatCount(info.itemCount, "item")} from ${formatDateTime(info.createdAt)}.\n\nRestoring replaces your current index — albums, favorites and watched folders included. Your photo files are not touched.`,
        { title: "Restore this backup?", kind: "warning" },
      );
      if (!confirmed) return;

      const message = await restoreLibrary(picked);
      await confirm(message, { title: "Restore staged", kind: "info" });
      await refreshPending();
    } catch (cause) {
      await confirm(String(cause), { title: "Could not read that file", kind: "error" });
    } finally {
      setBusy(false);
    }
  };

  const cancelRestore = async () => {
    await cancelPendingRestore();
    await refreshPending();
  };

  return (
    <>
      {pending && (
        <Card className="mt-6 flex items-center justify-between gap-4 border-honey/50 bg-cream/45 p-5">
          <div className="flex items-center gap-3">
            <RotateCcw size={17} className="shrink-0 text-honey-deep" />
            <div>
              <p className="text-xs font-extrabold text-honey-deep">A restore is waiting</p>
              <p className="mt-0.5 text-[11px] text-ink-muted">
                Close and reopen Hive to apply it. Nothing has changed yet.
              </p>
            </div>
          </div>
          <Button variant="secondary" icon={<X size={15} />} onClick={cancelRestore}>
            Cancel restore
          </Button>
        </Card>
      )}

      <Card className="mt-4 p-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-base font-extrabold text-ink">Back up your library</h2>
            <p className="mt-0.5 max-w-xl text-xs text-ink-muted">
              Saves everything Hive knows into a single file: watched folders, albums, favorites,
              hidden and archived items, extracted EXIF and the search index.
            </p>
          </div>
          <Button
            icon={busy ? <Loader2 size={15} className="animate-spin" /> : <Archive size={15} />}
            onClick={runBackup}
            disabled={busy}
          >
            Back up now
          </Button>
        </div>

        {lastBackup && (
          <div className="mt-5 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
            <p>
              {formatCount(lastBackup.itemCount, "item")} · {formatBytes(lastBackup.bytes)}
            </p>
            <p className="mt-1 break-all font-normal">{lastBackup.path}</p>
          </div>
        )}

        <div className="mt-5 flex items-start gap-3 rounded-2xl border border-ink/[.07] bg-canvas p-4">
          <ShieldCheck size={16} className="mt-0.5 shrink-0 text-honey-deep" />
          <p className="text-[11px] leading-relaxed text-ink-muted">
            Your photos are <strong>not</strong> inside the backup — they already live in your own
            folders, and copying them would only duplicate gigabytes you already have. Thumbnails are
            left out too, since Hive rebuilds them from your photos. What a backup protects is the
            work you cannot get back by rescanning: your albums, your favorites, what you hid or
            archived.
          </p>
        </div>
      </Card>

      <Card className="mt-4 p-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-base font-extrabold text-ink">Restore</h2>
            <p className="mt-0.5 max-w-xl text-xs text-ink-muted">
              Replaces the current index with a backup. Hive checks the file first, then applies it
              at the next start — the database cannot be swapped while it is open.
            </p>
          </div>
          <Button variant="secondary" icon={<RotateCcw size={15} />} onClick={runRestore} disabled={busy}>
            Choose a backup…
          </Button>
        </div>
      </Card>
    </>
  );
}
