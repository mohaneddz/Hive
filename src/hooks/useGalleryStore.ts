import { useEffect, useState } from "react";

const savedKey = "hive:saved-artworks";
const foldersKey = "hive:image-folders";
const eventName = "hive:gallery-updated";

function read<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem(key) ?? "null") ?? fallback; } catch { return fallback; }
}

export function useGalleryStore() {
  const [savedIds, setSavedIds] = useState<string[]>(() => read(savedKey, ["night-garden", "woven-light", "after-rain"]));
  const [folders, setFolders] = useState<string[]>(() => read(foldersKey, []));

  useEffect(() => {
    const sync = () => {
      setSavedIds(read(savedKey, []));
      setFolders(read(foldersKey, []));
    };
    window.addEventListener(eventName, sync);
    return () => window.removeEventListener(eventName, sync);
  }, []);

  const update = (key: string, value: unknown) => {
    localStorage.setItem(key, JSON.stringify(value));
    window.dispatchEvent(new Event(eventName));
  };
  const toggleSaved = (id: string) => update(savedKey, savedIds.includes(id) ? savedIds.filter((item) => item !== id) : [...savedIds, id]);
  const addFolder = (folder: string) => { if (folder && !folders.includes(folder)) update(foldersKey, [...folders, folder]); };
  const removeFolder = (folder: string) => update(foldersKey, folders.filter((item) => item !== folder));
  return { savedIds, folders, toggleSaved, addFolder, removeFolder };
}
