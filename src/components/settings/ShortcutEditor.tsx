import { useEffect, useState } from "react";
import { RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { SHORTCUT_ACTIONS, isBindable, keyLabel } from "@/config/shortcuts";
import { useShortcuts } from "@/hooks/useShortcuts";
import { cn } from "@/utils/cn";

export function ShortcutEditor() {
  const { bindings, overrides, rebind, resetAll } = useShortcuts();
  const [recording, setRecording] = useState<string | null>(null);

  // While recording, the next key press becomes the binding — so this listener
  // has to swallow the event before anything else acts on it.
  useEffect(() => {
    if (!recording) return;

    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (event.key === "Escape") {
        setRecording(null);
        return;
      }
      if (!isBindable(event.key)) return;

      void rebind(recording, event.key);
      setRecording(null);
    };

    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [recording, rebind]);

  /** Keys used by more than one action — worth flagging, not worth forbidding. */
  const duplicates = new Set(
    Object.values(bindings).filter(
      (key, index, all) => all.indexOf(key) !== index,
    ),
  );

  return (
    <>
      <div className="mt-5 space-y-1.5">
        {SHORTCUT_ACTIONS.map((action) => {
          const key = bindings[action.id];
          const isRecording = recording === action.id;
          return (
            <div
              key={action.id}
              className="flex items-center gap-3 rounded-2xl border border-ink/[.08] bg-canvas p-2.5 pl-3.5"
            >
              <span className="min-w-0 flex-1 truncate text-[11px] text-ink">{action.label}</span>
              {overrides[action.id] && (
                <span className="shrink-0 text-[10px] font-bold text-honey-deep">changed</span>
              )}
              <button
                onClick={() => setRecording(isRecording ? null : action.id)}
                className={cn(
                  "min-w-[68px] shrink-0 rounded-lg border px-2.5 py-1.5 text-[11px] font-extrabold transition",
                  isRecording
                    ? "animate-pulse border-honey bg-honey/20 text-honey-deep"
                    : duplicates.has(key)
                      ? "border-red-500/40 bg-red-500/5 text-red-600"
                      : "border-ink/15 bg-panel text-ink hover:border-honey/50",
                )}
              >
                {isRecording ? "Press a key" : keyLabel(key)}
              </button>
            </div>
          );
        })}
      </div>

      {duplicates.size > 0 && (
        <p className="mt-3 text-[11px] font-semibold text-red-600">
          Some keys are bound twice. Whichever action is checked first will win.
        </p>
      )}

      <div className="mt-4 flex items-center gap-3">
        <Button
          variant="secondary"
          icon={<RotateCcw size={14} />}
          onClick={resetAll}
          disabled={Object.keys(overrides).length === 0}
        >
          Reset to defaults
        </Button>
        <p className="text-[11px] text-ink-muted">
          {recording ? "Press Esc to cancel." : "Click a key to change it."}
        </p>
      </div>
    </>
  );
}
