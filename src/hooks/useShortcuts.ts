import { useCallback, useEffect, useRef, useState } from "react";

import {
  DEFAULT_BINDINGS,
  eventMatches,
  type ShortcutBindings,
} from "@/config/shortcuts";
import { getShortcutOverrides, isTauri, setShortcutOverrides } from "@/lib/tauri";

/**
 * The current key bindings, and a matcher to test events against them.
 *
 * Only what the user changed is stored; the rest falls back to the defaults, so
 * changing a default later reaches anyone who never rebound that key.
 */
export function useShortcuts() {
  const [overrides, setOverrides] = useState<ShortcutBindings>({});
  const bindings: ShortcutBindings = { ...DEFAULT_BINDINGS, ...overrides };

  // Key handlers read this instead of `bindings`, so they can stay registered
  // once instead of re-subscribing every time a binding changes.
  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  useEffect(() => {
    if (!isTauri()) return;
    void getShortcutOverrides().then(setOverrides);
  }, []);

  const matches = useCallback(
    (event: KeyboardEvent, actionId: string) =>
      eventMatches(event, bindingsRef.current[actionId] ?? ""),
    [],
  );

  const rebind = useCallback(async (actionId: string, key: string) => {
    setOverrides((previous) => {
      const next = { ...previous, [actionId]: key };
      void setShortcutOverrides(next);
      return next;
    });
  }, []);

  const resetAll = useCallback(async () => {
    setOverrides({});
    await setShortcutOverrides({});
  }, []);

  return { bindings, overrides, matches, rebind, resetAll };
}
