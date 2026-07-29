import { useEffect, useState } from "react";
import { Check, Download, Loader2, Pencil, ScanFace, Users } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { FaceCrop } from "@/components/media/FaceCrop";
import { MediaCard } from "@/components/media/MediaCard";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useJobProgress } from "@/hooks/useJobProgress";
import { backfillFaces, downloadFaceModels, getPersonMedia, isTauri, listPeople, renamePerson } from "@/lib/tauri";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import type { MediaItem, PersonSummary } from "@/types/media";
import { cn } from "@/utils/cn";

function PersonCard({ person, onRenamed }: { person: PersonSummary; onRenamed: () => void }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(person.name ?? "");

  const save = async () => {
    await renamePerson(person.id, name);
    setEditing(false);
    onRenamed();
  };

  return (
    <div className="flex flex-col items-center gap-2 text-center">
      <div className="size-20 overflow-hidden rounded-full border border-ink/[.08] bg-shell">
        <FaceCrop faceId={person.coverFaceId} alt={person.name ?? "Unnamed person"} className="size-full object-cover" />
      </div>
      {editing ? (
        <div className="flex items-center gap-1">
          <input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && save()}
            placeholder="Name"
            className="w-24 rounded-lg border border-ink/[.12] bg-panel px-2 py-1 text-xs text-ink outline-none"
          />
          <button onClick={save} className="text-honey-deep">
            <Check size={14} />
          </button>
        </div>
      ) : (
        <button onClick={() => setEditing(true)} className="flex items-center gap-1 text-xs font-bold text-ink">
          {person.name ?? "Add name"}
          <Pencil size={11} className="text-ink-muted" />
        </button>
      )}
      <p className="text-[11px] text-ink-muted">
        {person.faceCount} photo{person.faceCount === 1 ? "" : "s"}
      </p>
    </div>
  );
}

export function PeoplePage() {
  const { status: aiStatus, refresh: refreshAiStatus } = useAiStatus();
  const jobs = useJobProgress();
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [media, setMedia] = useState<MediaItem[]>([]);
  const [downloading, setDownloading] = useState(false);
  const [scanning, setScanning] = useState(false);

  const downloadJob = jobs.find((j) => j.kind === "download_face_models" && j.status === "running");
  const scanJob = jobs.find((j) => j.kind === "face_backfill" && j.status === "running");

  const refresh = () => {
    if (!isTauri()) return;
    listPeople().then(setPeople);
  };

  useEffect(refresh, []);

  useEffect(() => {
    if (scanJob) refresh();
  }, [scanJob]);

  useEffect(() => {
    if (!selected) return;
    getPersonMedia(selected).then(setMedia);
  }, [selected]);

  const startDownload = async () => {
    setDownloading(true);
    try {
      await downloadFaceModels();
      refreshAiStatus();
    } finally {
      setDownloading(false);
    }
  };

  const startScan = async () => {
    setScanning(true);
    try {
      await backfillFaces();
      refreshAiStatus();
      refresh();
    } finally {
      setScanning(false);
    }
  };

  if (aiStatus && !aiStatus.faceModelsReady) {
    return (
      <div>
        <GalleryPageHeader
          eyebrow="People"
          title="Faces Hive has recognized."
          description="Hive will group photos by the people in them, entirely on-device."
        />
        <Card className="mt-8 flex flex-col items-center gap-3 p-16 text-center">
          <div className="grid size-14 place-items-center rounded-2xl bg-cream text-honey-deep">
            <Users size={22} />
          </div>
          <p className="text-sm font-extrabold text-ink">Face recognition isn't set up yet</p>
          <p className="max-w-sm text-xs text-ink-muted">
            Detection and matching run entirely on-device — nothing is uploaded, ~67 MB one-time
            download.
          </p>
          <Button
            icon={downloading || downloadJob ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
            disabled={downloading || !!downloadJob}
            onClick={startDownload}
          >
            {downloadJob ? "Downloading…" : "Download & enable"}
          </Button>
        </Card>
      </div>
    );
  }

  return (
    <div>
      <GalleryPageHeader
        eyebrow="People"
        title="Faces Hive has recognized."
        description={
          aiStatus
            ? `${aiStatus.peopleCount} ${aiStatus.peopleCount === 1 ? "person" : "people"} found across ${aiStatus.facesIndexedCount.toLocaleString()} scanned photos.`
            : "Grouped automatically by face similarity."
        }
        action={
          aiStatus && aiStatus.facesIndexedCount < aiStatus.eligibleCount ? (
            <Button
              variant="secondary"
              icon={scanning || scanJob ? <Loader2 size={14} className="animate-spin" /> : <ScanFace size={14} />}
              disabled={scanning || !!scanJob}
              onClick={startScan}
            >
              {scanJob ? `Scanning… ${scanJob.current}/${scanJob.total}` : "Scan for faces"}
            </Button>
          ) : undefined
        }
      />

      {people.length === 0 ? (
        <Card className="mt-8 flex flex-col items-center gap-3 p-16 text-center">
          <Users size={22} className="text-ink-muted" />
          <p className="text-xs text-ink-muted">No faces found yet.</p>
          <Button
            variant="secondary"
            icon={scanning || scanJob ? <Loader2 size={14} className="animate-spin" /> : <ScanFace size={14} />}
            disabled={scanning || !!scanJob}
            onClick={startScan}
          >
            {scanJob ? `Scanning… ${scanJob.current}/${scanJob.total}` : "Scan library for faces"}
          </Button>
        </Card>
      ) : (
        <div className="mt-7 grid grid-cols-6 gap-5 sm:grid-cols-8">
          {people.map((person) => (
            <button key={person.id} onClick={() => setSelected(person.id)} className="contents">
              <div className={cn(selected === person.id && "rounded-2xl ring-2 ring-honey/60")}>
                <PersonCard person={person} onRenamed={refresh} />
              </div>
            </button>
          ))}
        </div>
      )}

      {selected && (
        <div className="mt-9">
          <h2 className="mb-4 text-sm font-extrabold text-ink">
            {people.find((p) => p.id === selected)?.name ?? "Unnamed person"}
          </h2>
          <div className="grid grid-cols-4 gap-4">
            {media.map((item) => (
              <MediaCard key={item.id} item={item} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
