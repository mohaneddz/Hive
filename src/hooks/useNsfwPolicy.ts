import { useEffect, useState } from "react";

import { getNsfwPolicy, isTauri } from "@/lib/tauri";
import type { NsfwPolicy } from "@/types/media";

/** Mirrors the backend default, used for the instant before the real one lands. */
const FALLBACK: NsfwPolicy = { threshold: 0.7, autoHide: false };

// A grid mounts hundreds of cards at once and every one of them wants the
// threshold. They share a single request and a single answer.
let pending: Promise<NsfwPolicy> | null = null;
let current: NsfwPolicy = FALLBACK;
const listeners = new Set<(policy: NsfwPolicy) => void>();

function load(): Promise<NsfwPolicy> {
  if (!pending) {
    pending = isTauri()
      ? getNsfwPolicy().catch(() => FALLBACK)
      : Promise.resolve(FALLBACK);
    void pending.then((policy) => {
      current = policy;
      listeners.forEach((notify) => notify(policy));
    });
  }
  return pending;
}

/** Call after saving in Settings so open grids re-cover or reveal immediately. */
export function refreshNsfwPolicy() {
  pending = null;
  void load();
}

/** The threshold and auto-hide choice, shared by every caller. */
export function useNsfwPolicy(): NsfwPolicy {
  const [policy, setPolicy] = useState(current);

  useEffect(() => {
    listeners.add(setPolicy);
    void load();
    return () => {
      listeners.delete(setPolicy);
    };
  }, []);

  return policy;
}
