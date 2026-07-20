import { FolderOpen, Image, Monitor, Moon, Palette, Plus, Sun, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

import { Card } from "@/components/ui/Card";
import { useTheme, type Theme } from "@/hooks/useTheme";
import { useGalleryStore } from "@/hooks/useGalleryStore";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { cn } from "@/utils/cn";

const choices: { value: Theme; label: string; icon: typeof Sun; caption: string }[] = [
  { value: "light", label: "Light", icon: Sun, caption: "Warm gallery walls" },
  { value: "dark", label: "Dark", icon: Moon, caption: "A quieter viewing room" },
];

export function GallerySettingsPage() {
  const { theme, setTheme } = useTheme();
  const { folders, addFolder, removeFolder } = useGalleryStore();
  const chooseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: true, title: "Choose image folders" });
      (Array.isArray(selected) ? selected : selected ? [selected] : []).forEach(addFolder);
    } catch { /* The web preview has no native folder picker. */ }
  };
  return <div><GalleryPageHeader eyebrow="Preferences" title="Make the gallery yours." description="Choose where Hive looks for images and how your collection feels." />
    <div className="mt-7 grid max-w-4xl gap-5">
      <Card className="p-6"><div className="flex items-center gap-3"><div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep"><FolderOpen size={19} /></div><div><h2 className="text-base font-extrabold text-ink">Image folders</h2><p className="mt-0.5 text-xs text-ink-muted">Hive will include images from these folders in your collection.</p></div></div><div className="mt-5 space-y-2">{folders.length === 0 && <div className="rounded-2xl border border-dashed border-ink/[.14] p-5 text-center text-xs text-ink-muted">No folders connected yet. Add your Pictures folder to get started.</div>}{folders.map((folder) => <div key={folder} className="flex items-center gap-3 rounded-2xl border border-ink/[.08] bg-canvas p-3"><Image size={16} className="shrink-0 text-honey-deep" /><span className="min-w-0 flex-1 truncate text-xs font-semibold text-ink">{folder}</span><button onClick={() => removeFolder(folder)} className="icon-button !h-8 !w-8" aria-label={`Remove ${folder}`}><Trash2 size={14} /></button></div>)}</div><button onClick={chooseFolder} className="mt-4 inline-flex items-center gap-2 rounded-xl bg-honey px-4 py-2.5 text-xs font-extrabold text-[#3b2900] transition hover:bg-honey-dark"><Plus size={15} /> Add image folder</button></Card>
      <Card className="p-6"><div className="flex items-center gap-3"><div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep"><Palette size={19} /></div><div><h2 className="text-base font-extrabold text-ink">Appearance</h2><p className="mt-0.5 text-xs text-ink-muted">Choose a view that feels right for the work.</p></div></div><div className="mt-6 grid grid-cols-2 gap-4">{choices.map((choice) => <button key={choice.value} onClick={() => setTheme(choice.value)} className={cn("flex items-center gap-4 rounded-2xl border p-4 text-left transition", theme === choice.value ? "border-honey bg-cream/55" : "border-ink/[.08] bg-canvas hover:border-honey/40")}><div className="grid size-10 place-items-center rounded-xl bg-panel text-ink"><choice.icon size={18} /></div><div><p className="text-sm font-extrabold text-ink">{choice.label}</p><p className="mt-0.5 text-[11px] text-ink-muted">{choice.caption}</p></div></button>)}</div><div className="mt-6 flex items-center gap-3 rounded-2xl border border-ink/[.07] bg-canvas p-4 text-xs text-ink-muted"><Monitor size={16} /><span>Hive uses your saved preference every time it opens.</span></div></Card>
      <Card className="p-6"><h2 className="text-base font-extrabold text-ink">Library behavior</h2><div className="mt-4 space-y-4 text-xs"><label className="flex items-center justify-between gap-4 text-ink"><span><span className="block font-bold">Include subfolders</span><span className="text-ink-muted">Scan nested folders automatically</span></span><input type="checkbox" defaultChecked className="size-4 accent-[#e3a105]" /></label><label className="flex items-center justify-between gap-4 text-ink"><span><span className="block font-bold">Remember last view</span><span className="text-ink-muted">Restore your last collection filter</span></span><input type="checkbox" defaultChecked className="size-4 accent-[#e3a105]" /></label></div></Card>
    </div></div>;
}
