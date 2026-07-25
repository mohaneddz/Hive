import { FolderPlus, ImagePlus, Loader2, X } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { Button } from "@/components/ui/Button";
import { MediaCard } from "@/components/media/MediaCard";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function GalleryPage() {
  const { items, total, loading, jobs, folders, addFolder, cancelJob, loadPage } = useMediaLibrary();
  const [searchParams, setSearchParams] = useSearchParams();
  const folderId = searchParams.get("folder") ?? undefined;
  const activeFolder = useMemo(() => folders.find((f) => f.id === folderId), [folders, folderId]);

  useEffect(() => {
    void loadPage(0, { folderId });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folderId]);

  const chooseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Add a media folder" });
      if (typeof selected === "string") {
        await addFolder(selected);
      }
    } catch {
      /* No native folder picker in the web preview. */
    }
  };

  const activeJob = jobs.find((job) => job.status === "running");

  return (
    <div>
      <GalleryPageHeader
        eyebrow={activeFolder ? "Folder" : "Your library"}
        title={activeFolder ? activeFolder.name : "Every photo and video, indexed locally."}
        description={
          folders.length === 0
            ? "Add a folder to start building your local library."
            : `${total} item${total === 1 ? "" : "s"}${activeFolder ? "" : ` across ${folders.length} folder${folders.length === 1 ? "" : "s"}`}.`
        }
        action={
          activeFolder ? (
            <Link
              to="/gallery"
              onClick={() => setSearchParams({})}
              className="inline-flex items-center gap-1.5 text-xs font-bold text-honey-deep hover:underline"
            >
              <X size={13} /> Clear filter
            </Link>
          ) : (
            <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
              Add folder
            </Button>
          )
        }
      />

      {activeJob && (
        <div className="mt-5 flex items-center justify-between gap-3 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
          <div className="flex items-center gap-3">
            <Loader2 size={15} className="animate-spin shrink-0" />
            <span>
              Indexing {activeJob.current}/{activeJob.total}
              {activeJob.message ? ` — ${activeJob.message}` : ""}
            </span>
          </div>
          <button
            onClick={() => cancelJob(activeJob.id)}
            className="flex items-center gap-1.5 rounded-lg bg-honey/20 px-3 py-1.5 text-xs font-bold text-honey-deep transition hover:bg-honey/30"
          >
            <X size={14} />
            Cancel
          </button>
        </div>
      )}

      {items.length === 0 && !loading && (
        <div className="mt-12 flex flex-col items-center gap-4 rounded-3xl border border-dashed border-ink/[.15] p-16 text-center">
          <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
            <ImagePlus size={22} />
          </div>
          <div>
            <p className="text-sm font-extrabold text-ink">No media yet</p>
            <p className="mt-1 text-xs text-ink-muted">
              Add a folder full of photos or videos to build your library.
            </p>
          </div>
          <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
            Add folder
          </Button>
        </div>
      )}

      {items.length > 0 && (
        <>
          <div className="mt-7 grid grid-cols-4 gap-4">
            {items.map((item) => (
              <MediaCard key={item.id} item={item} />
            ))}
          </div>
          {items.length < total && (
            <div className="mt-8 flex justify-center">
              <Button variant="secondary" disabled={loading} onClick={() => loadPage(items.length, { folderId })}>
                {loading ? "Loading…" : "Load more"}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
