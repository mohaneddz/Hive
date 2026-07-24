import { FolderPlus, ImagePlus, Loader2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/Button";
import { MediaCard } from "@/components/media/MediaCard";
import { useMediaLibrary } from "@/hooks/useMediaLibrary";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";

export function GalleryPage() {
  const { items, total, loading, jobs, folders, addFolder, loadPage } = useMediaLibrary();

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
        eyebrow="Your library"
        title="Every photo and video, indexed locally."
        description={
          folders.length === 0
            ? "Add a folder to start building your local library."
            : `${total} item${total === 1 ? "" : "s"} across ${folders.length} folder${folders.length === 1 ? "" : "s"}.`
        }
        action={
          <Button icon={<FolderPlus size={16} />} onClick={chooseFolder}>
            Add folder
          </Button>
        }
      />

      {activeJob && (
        <div className="mt-5 flex items-center gap-3 rounded-2xl border border-honey/30 bg-cream/45 px-4 py-3 text-xs font-semibold text-honey-deep">
          <Loader2 size={15} className="animate-spin" />
          Indexing {activeJob.current}/{activeJob.total}
          {activeJob.message ? ` — ${activeJob.message}` : ""}
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
              <Button variant="secondary" disabled={loading} onClick={() => loadPage(items.length)}>
                {loading ? "Loading…" : "Load more"}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
