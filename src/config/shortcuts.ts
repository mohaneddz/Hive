/**
 * Every keyboard action in the app, in one place.
 *
 * Handlers ask "does this event mean `next`?" instead of testing a literal key,
 * so a user can rebind anything without a single `if (event.key === …)` needing
 * to know about it.
 */
export interface ShortcutAction {
  id: string;
  label: string;
  /** The key as `KeyboardEvent.key` reports it. */
  defaultKey: string;
  group: "Viewer" | "Editor";
}

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  { id: "previous", label: "Previous photo", defaultKey: "ArrowLeft", group: "Viewer" },
  { id: "next", label: "Next photo", defaultKey: "ArrowRight", group: "Viewer" },
  { id: "close", label: "Close the viewer", defaultKey: "Escape", group: "Viewer" },
  { id: "info", label: "Toggle the details drawer", defaultKey: "i", group: "Viewer" },
  { id: "edit", label: "Open the editor", defaultKey: "e", group: "Viewer" },
  { id: "rotate", label: "Rotate the view", defaultKey: "r", group: "Viewer" },
  { id: "zoomIn", label: "Zoom in", defaultKey: "+", group: "Viewer" },
  { id: "zoomOut", label: "Zoom out", defaultKey: "-", group: "Viewer" },
  { id: "resetZoom", label: "Reset zoom", defaultKey: "0", group: "Viewer" },
  { id: "slideshow", label: "Play or pause the slideshow", defaultKey: " ", group: "Viewer" },
  { id: "fullscreen", label: "Fullscreen", defaultKey: "f", group: "Viewer" },
  { id: "trash", label: "Move to trash", defaultKey: "Delete", group: "Viewer" },
];

export type ShortcutBindings = Record<string, string>;

export const DEFAULT_BINDINGS: ShortcutBindings = Object.fromEntries(
  SHORTCUT_ACTIONS.map((action) => [action.id, action.defaultKey]),
);

/** How a key reads on screen. `" "` as a label would be invisible. */
export function keyLabel(key: string): string {
  const named: Record<string, string> = {
    " ": "Space",
    ArrowLeft: "←",
    ArrowRight: "→",
    ArrowUp: "↑",
    ArrowDown: "↓",
    Escape: "Esc",
    Delete: "Del",
  };
  return named[key] ?? (key.length === 1 ? key.toUpperCase() : key);
}

/**
 * Whether a key event triggers an action.
 *
 * Single characters compare case-insensitively, so `e` fires whether or not
 * caps lock is on; named keys like `ArrowLeft` compare exactly.
 */
export function eventMatches(event: KeyboardEvent, key: string): boolean {
  if (key.length === 1) return event.key.toLowerCase() === key.toLowerCase();
  return event.key === key;
}

/** Keys that would trap the user or fight the operating system. */
const UNBINDABLE = new Set(["Tab", "Meta", "Control", "Alt", "Shift", "CapsLock"]);

export function isBindable(key: string): boolean {
  return !UNBINDABLE.has(key);
}
