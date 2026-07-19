import { Monitor, Moon, Palette, Sun } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { useTheme, type Theme } from "@/hooks/useTheme";
import { GalleryPageHeader } from "@/pages/GalleryPageHeader";
import { cn } from "@/utils/cn";

const choices: { value: Theme; label: string; icon: typeof Sun; caption: string }[] = [
  { value: "light", label: "Light", icon: Sun, caption: "Warm gallery walls" },
  { value: "dark", label: "Dark", icon: Moon, caption: "A quieter viewing room" },
];

export function GallerySettingsPage() {
  const { theme, setTheme } = useTheme();
  return <div><GalleryPageHeader eyebrow="Preferences" title="Make the gallery yours." description="Set the atmosphere for browsing your collection." /><Card className="mt-7 max-w-3xl p-6"><div className="flex items-center gap-3"><div className="grid size-10 place-items-center rounded-xl bg-cream text-honey-deep"><Palette size={19} /></div><div><h2 className="text-base font-extrabold text-ink">Appearance</h2><p className="mt-0.5 text-xs text-ink-muted">Choose a view that feels right for the work.</p></div></div><div className="mt-6 grid grid-cols-2 gap-4">{choices.map((choice) => <button key={choice.value} onClick={() => setTheme(choice.value)} className={cn("flex items-center gap-4 rounded-2xl border p-4 text-left transition", theme === choice.value ? "border-honey bg-cream/55" : "border-ink/[.08] bg-canvas hover:border-honey/40")}><div className="grid size-10 place-items-center rounded-xl bg-panel text-ink"><choice.icon size={18} /></div><div><p className="text-sm font-extrabold text-ink">{choice.label}</p><p className="mt-0.5 text-[11px] text-ink-muted">{choice.caption}</p></div></button>)}</div><div className="mt-6 flex items-center gap-3 rounded-2xl border border-ink/[.07] bg-canvas p-4 text-xs text-ink-muted"><Monitor size={16} /><span>Hive uses your saved preference every time it opens.</span></div></Card></div>;
}
