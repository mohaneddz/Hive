import { invoke } from "@tauri-apps/api/core";

import type {
  AiStatus,
  DuplicateGroup,
  Folder,
  FolderStats,
  LibraryStats,
  MediaItem,
  MediaPage,
  PlaceCluster,
} from "@/types/media";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export function listFolders(): Promise<Folder[]> {
  return invoke("list_folders");
}

export function listFoldersWithStats(): Promise<FolderStats[]> {
  return invoke("list_folders_with_stats");
}

export function addWatchedFolder(path: string): Promise<Folder> {
  return invoke("add_watched_folder", { path });
}

export function removeWatchedFolder(folderId: string): Promise<void> {
  return invoke("remove_watched_folder", { folderId });
}

export function scanFolder(folderId: string): Promise<string> {
  return invoke("scan_folder", { folderId });
}

export function cancelJob(jobId: string): Promise<void> {
  return invoke("cancel_job", { jobId });
}

export function getMediaPage(options: {
  limit: number;
  offset: number;
  mediaType?: string;
  favoritesOnly?: boolean;
  folderId?: string;
}): Promise<MediaPage> {
  return invoke("get_media_page", options);
}

export function getTrash(limit: number, offset: number): Promise<MediaPage> {
  return invoke("get_trash", { limit, offset });
}

export function deleteMediaPermanently(mediaId: string): Promise<void> {
  return invoke("delete_media_permanently", { mediaId });
}

export function getPlaces(): Promise<PlaceCluster[]> {
  return invoke("get_places");
}

export function getLibraryStats(): Promise<LibraryStats> {
  return invoke("get_library_stats");
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

export function getAiStatus(): Promise<AiStatus> {
  return invoke("get_ai_status");
}

export function downloadAiModels(): Promise<void> {
  return invoke("download_ai_models");
}

export function semanticSearch(query: string, limit = 200): Promise<MediaItem[]> {
  return invoke("semantic_search", { query, limit });
}

export function backfillEmbeddings(): Promise<void> {
  return invoke("backfill_embeddings");
}

export function scanDuplicates(): Promise<number> {
  return invoke("scan_duplicates");
}

export function getDuplicateGroups(): Promise<DuplicateGroup[]> {
  return invoke("get_duplicate_groups");
}

export function dismissDuplicateGroup(groupId: string): Promise<void> {
  return invoke("dismiss_duplicate_group", { groupId });
}
