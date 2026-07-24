import { invoke } from "@tauri-apps/api/core";

import type { Folder, MediaItem, MediaPage, ScanResult } from "@/types/media";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export function listFolders(): Promise<Folder[]> {
  return invoke("list_folders");
}

export function addWatchedFolder(path: string): Promise<Folder> {
  return invoke("add_watched_folder", { path });
}

export function removeWatchedFolder(folderId: string): Promise<void> {
  return invoke("remove_watched_folder", { folderId });
}

export function scanFolder(folderId: string): Promise<ScanResult> {
  return invoke("scan_folder", { folderId });
}

export function getMediaPage(options: {
  limit: number;
  offset: number;
  mediaType?: string;
  favoritesOnly?: boolean;
}): Promise<MediaPage> {
  return invoke("get_media_page", options);
}

export function getMediaDetail(mediaId: string): Promise<MediaItem> {
  return invoke("get_media_detail", { mediaId });
}

export function searchMedia(query: string, limit = 200): Promise<MediaItem[]> {
  return invoke("search_media", { query, limit });
}

export async function readMediaUrl(
  mediaId: string,
  variant: "sm" | "md" | "original",
): Promise<string> {
  const bytes = await invoke<number[]>("read_media_bytes", { mediaId, variant });
  const blob = new Blob([new Uint8Array(bytes)]);
  return URL.createObjectURL(blob);
}

export function setFavorite(mediaId: string, favorite: boolean): Promise<void> {
  return invoke("set_favorite", { mediaId, favorite });
}

export function setTrashed(mediaId: string, trashed: boolean): Promise<void> {
  return invoke("set_trashed", { mediaId, trashed });
}
