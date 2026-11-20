import { useEffect, useState } from "react";
import { Check, Download, Loader2, Merge, Pencil, ScanFace, UserMinus, Users, X } from "lucide-react";
import { confirm } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { FaceCrop } from "@/components/media/FaceCrop";
import { MediaCard } from "@/components/media/MediaCard";
import { useAiStatus } from "@/hooks/useAiStatus";
import { useJobProgress } from "@/hooks/useJobProgress";
import { backfillFaces, downloadFaceModels, getPersonMedia, isTauri, listPeople, mergePeople, reassignPersonMedia, renamePerson } from "@/lib/tauri";
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
          <button onClick={(event) => { event.stopPropagation(); void save(); }} className="text-honey-deep" aria-label="Save person name">
            <Check size={14} />
          </button>
        </div>
      ) : (
        <button onClick={(event) => { event.stopPropagation(); setEditing(true); }} className="flex items-center gap-1 text-xs font-bold text-ink">
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
  const [mergeTarget, setMergeTarget] = useState("");
  const [merging, setMerging] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [correcting, setCorrecting] = useState(false);
  const [correctionTarget, setCorrectionTarget] = useState("");
  const [selectedMedia, setSelectedMedia] = useState<Set<string>>(new Set());

  const downloadJob = jobs.find((j) => j.kind === "download_face_models" && j.status === "running");
  const scanJob = jobs.find((j) => j.kind === "face_backfill" && j.status === "running");

  const refresh = () => {
    if (!isTauri()) return;
    setFailure(null);
    listPeople().then(setPeople).catch((cause) => setFailure(String(cause)));
  };

  useEffect(refresh, []);

  useEffect(() => {
    if (scanJob) refresh();
  }, [scanJob]);

  useEffect(() => {
    if (!selected) {
      setMedia([]);
      return;
    }
    getPersonMedia(selected).then(setMedia).catch((cause) => setFailure(String(cause)));
  }, [selected]);

  const mergeSelected = async () => {
    if (!selected || !mergeTarget || selected === mergeTarget) return;
    const source = people.find((person) => person.id === selected);
    const target = people.find((person) => person.id === mergeTarget);
    const approved = await confirm(
      `Merge ${source?.name ?? "this person"} into ${target?.name ?? "the selected person"}? Their face groups will become one.`,
      { title: "Merge people", kind: "warning" },
    );
    if (!approved) return;
    setMerging(true);
    setFailure(null);
    try {
      await mergePeople(selected, mergeTarget);
      setSelected(mergeTarget);
      setMergeTarget("");
      refresh();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setMerging(false);
    }
  };

  const toggleMedia = (mediaId: string) => {
    setSelectedMedia((current) => {
      const next = new Set(current);
      if (next.has(mediaId)) next.delete(mediaId);
      else next.add(mediaId);
      return next;
    });
  };

  const applyCorrection = async () => {
    if (!selected || selectedMedia.size === 0) return;
    setMerging(true);
    setFailure(null);
    try {
      await reassignPersonMedia(selected, correctionTarget || null, [...selectedMedia]);
      setMedia((current) => current.filter((item) => !selectedMedia.has(item.id)));
      setSelectedMedia(new Set());
      setCorrecting(false);
      setCorrectionTarget("");
      refresh();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setMerging(false);
    }
  };

  const startDownload = async () => {
    setDownloading(true);
    setFailure(null);
    try {
      await downloadFaceModels();
      refreshAiStatus();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setDownloading(false);
    }
  };

  const startScan = async () => {
    setScanning(true);
    setFailure(null);
    try {
      await backfillFaces();
      refreshAiStatus();
      refresh();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setScanning(false);
    }
  };

  const markNotPerson = async () => {
    if (!selected || media.length === 0) return;
    const label = people.find((person) => person.id === selected)?.name ?? "this cluster";
    const approved = await confirm(
      `Mark ${label} as not a person? ${media.length} matching photo${media.length === 1 ? "" : "s"} will be removed from this face group.`,
      { title: "Remove face group", kind: "warning" },
    );
    if (!approved) return;
    setMerging(true);
    setFailure(null);
    try {
      await reassignPersonMedia(selected, null, media.map((item) => item.id));
      setSelected(null);
      setMedia([]);
      refresh();
    } catch (cause) {
      setFailure(String(cause));
    } finally {
      setMerging(false);
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
          <div className="grid size-14 place-items-center rounded-2xl bg-cream dark:bg-honey/15 text-honey-deep">
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

      {failure && (
        <div className="mt-5 flex items-center justify-between rounded-2xl border border-red-500/25 bg-red-500/5 px-4 py-3 text-xs text-red-600">
          <span className="break-words">{failure}</span>
          <button onClick={() => setFailure(null)} className="font-bold">Dismiss</button>
        </div>
      )}

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
            <div key={person.id} onClick={() => setSelected((current) => current === person.id ? null : person.id)} className="cursor-pointer">
              <div className={cn(selected === person.id && "rounded-2xl ring-2 ring-honey/60")}>
                <PersonCard person={person} onRenamed={refresh} />
              </div>
            </div>
          ))}
        </div>
      )}

      {selected && (
        <div className="mt-9">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <h2 className="text-sm font-extrabold text-ink">
              {people.find((p) => p.id === selected)?.name ?? "Unnamed person"}
            </h2>
            <div className="flex items-center gap-2">
              <select
                value={mergeTarget}
                onChange={(event) => setMergeTarget(event.target.value)}
                className="select-input"
                aria-label="Merge selected person into"
              >
                <option value="">Merge into…</option>
                {people.filter((person) => person.id !== selected).map((person) => (
                  <option key={person.id} value={person.id}>{person.name ?? "Unnamed person"}</option>
                ))}
              </select>
              <Button variant="secondary" icon={<Merge size={14} />} disabled={!mergeTarget || merging} onClick={mergeSelected}>
                {merging ? "Merging…" : "Merge"}
              </Button>
              <Button variant="secondary" icon={<UserMinus size={14} />} disabled={media.length === 0 || merging} onClick={markNotPerson}>
                Not a person
              </Button>
              <Button variant="ghost" icon={<X size={14} />} onClick={() => setSelected(null)}>Close</Button>
            </div>
          </div>
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-ink/[.08] bg-panel p-3">
            <p className="text-xs text-ink-muted">
              {correcting ? `${selectedMedia.size} selected for correction` : "Found a wrong match? Select photos to remove or reassign."}
            </p>
            {correcting ? (
              <div className="flex flex-wrap items-center gap-2">
                <select value={correctionTarget} onChange={(event) => setCorrectionTarget(event.target.value)} className="select-input">
                  <option value="">Not this person</option>
                  {people.filter((person) => person.id !== selected).map((person) => (
                    <option key={person.id} value={person.id}>Move to {person.name ?? "unnamed person"}</option>
                  ))}
                </select>
                <Button variant="secondary" icon={<UserMinus size={14} />} disabled={selectedMedia.size === 0 || merging} onClick={applyCorrection}>
                  Apply
                </Button>
                <Button variant="ghost" onClick={() => { setCorrecting(false); setSelectedMedia(new Set()); }}>Cancel</Button>
              </div>
            ) : (
              <Button variant="secondary" onClick={() => setCorrecting(true)}>Correct matches</Button>
            )}
          </div>
          <div className="grid grid-cols-4 gap-4">
            {media.map((item) => (
              <MediaCard
                key={item.id}
                item={item}
                selected={correcting ? selectedMedia.has(item.id) : undefined}
                onToggleSelect={correcting ? toggleMedia : undefined}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
