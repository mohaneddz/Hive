import { invoke } from "@tauri-apps/api/core";

import type {
  AiStatus,
  Album,
  BackupInfo,
  BatchReport,
  BestPhotoResult,
  BlurReport,
  CacheReport,
  ConvertFormat,
  DuplicateGroup,
  EditOps,
  EventGroup,
  ExplorerEntry,
  ExportReport,
  Folder,
  FolderStats,
  LibraryHealth,
  LibraryStats,
  MediaItem,
  MediaPage,
  MediaScope,
  MediaSort,
  NsfwPolicy,
  PersonSummary,
  PlaceCluster,
  PlaceGroup,
  RankedItem,
  RenamePreview,
  SaveMode,
  SmartAlbum,
  SmartAlbumMatch,
  SmartAlbumRule,
  SmartAlbumSuggestion,
  StorageStats,
  TagResult,
  TagSummary,
  TimelineBucket,
  TimelineGranularity,
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

/**
 * Declared as a type alias, not an interface: `invoke` wants a
 * `Record<string, unknown>`, and only type aliases get an implicit index signature.
 */
export type MediaPageOptions = {
  limit: number;
  offset: number;
  mediaType?: string;
  favoritesOnly?: boolean;
  folderId?: string;
  /** Restrict to the members of one album. */
  albumId?: string;
  /** Which slice of the library to read. Defaults to "library". */
  scope?: MediaScope;
  sort?: MediaSort;
};

export function getMediaPage(options: MediaPageOptions): Promise<MediaPage> {
  return invoke("get_media_page", options);
}

/** Pauses or resumes live watching without un-indexing anything. */
export function setFolderWatched(folderId: string, watched: boolean): Promise<void> {
  return invoke("set_folder_watched", { folderId, watched });
}

export function setHidden(mediaId: string, hidden: boolean): Promise<void> {
  return invoke("set_hidden", { mediaId, hidden });
}

export function setArchived(mediaId: string, archived: boolean): Promise<void> {
  return invoke("set_archived", { mediaId, archived });
}

export function touchLastViewed(mediaId: string): Promise<void> {
  return invoke("touch_last_viewed", { mediaId });
}

/** Purges every trashed item. Resolves with the number removed. */
export function emptyTrash(): Promise<number> {
  return invoke("empty_trash");
}

/** Photos taken on this calendar day in an earlier year. */
export function getOnThisDay(limit = 24): Promise<MediaItem[]> {
  return invoke("get_on_this_day", { limit });
}

/* ----------------------------------------------------------------- albums -- */

export function listAlbums(): Promise<Album[]> {
  return invoke("list_albums");
}

export function getAlbum(albumId: string): Promise<Album> {
  return invoke("get_album", { albumId });
}

export function createAlbum(name: string, description?: string): Promise<Album> {
  return invoke("create_album", { name, description });
}

export function updateAlbum(albumId: string, name: string, description?: string): Promise<void> {
  return invoke("update_album", { albumId, name, description });
}

export function deleteAlbum(albumId: string): Promise<void> {
  return invoke("delete_album", { albumId });
}

export function setAlbumCover(albumId: string, mediaId: string | null): Promise<void> {
  return invoke("set_album_cover", { albumId, mediaId });
}

/** Resolves with how many were actually new to the album. */
export function addMediaToAlbum(albumId: string, mediaIds: string[]): Promise<number> {
  return invoke("add_media_to_album", { albumId, mediaIds });
}

export function removeMediaFromAlbum(albumId: string, mediaId: string): Promise<void> {
  return invoke("remove_media_from_album", { albumId, mediaId });
}

export function listAlbumsForMedia(mediaId: string): Promise<string[]> {
  return invoke("list_albums_for_media", { mediaId });
}

/* ----------------------------------------------------------------- places -- */

export function listPlaces(precision?: number): Promise<PlaceGroup[]> {
  return invoke("list_places", { precision });
}

export function listMediaAtPlace(
  lat: number,
  lon: number,
  precision?: number,
  limit = 300,
): Promise<MediaItem[]> {
  return invoke("list_media_at_place", { lat, lon, precision, limit });
}

/* --------------------------------------------------------------- explorer -- */

export function listDrives(): Promise<ExplorerEntry[]> {
  return invoke("list_drives");
}

export function listDirectory(path: string): Promise<ExplorerEntry[]> {
  return invoke("list_directory", { path });
}

export function parentDirectory(path: string): Promise<string | null> {
  return invoke("parent_directory", { path });
}

/* ----------------------------------------------------------------- editor -- */

/**
 * Bakes the adjustments into a real file.
 * `"copy"` writes `name (edited).ext` beside the original and leaves it intact;
 * `"overwrite"` replaces it, keeping favorites, albums and flags.
 */
export function applyEdits(mediaId: string, ops: EditOps, mode: SaveMode): Promise<MediaItem> {
  return invoke("apply_edits", { mediaId, ops, mode });
}

/** Stored by Hive, never written into the photo file. */
export function updateMediaMetadata(
  mediaId: string,
  title: string | null,
  description: string | null,
  takenAtOverride: string | null,
): Promise<MediaItem> {
  return invoke("update_media_metadata", { mediaId, title, description, takenAtOverride });
}

/* ------------------------------------------------------------------ batch -- */

/**
 * Pattern tokens: `{name}` original stem Â· `{n}` sequence number Â· `{date}`
 * capture day. The extension is always carried over, never part of the pattern.
 */
export function previewBatchRename(
  mediaIds: string[],
  pattern: string,
  startIndex = 1,
): Promise<RenamePreview[]> {
  return invoke("preview_batch_rename", { mediaIds, pattern, startIndex });
}

export function batchRename(
  mediaIds: string[],
  pattern: string,
  startIndex = 1,
): Promise<BatchReport> {
  return invoke("batch_rename", { mediaIds, pattern, startIndex });
}

/** Writes compressed JPEG copies into `destination`. Originals are untouched. */
export function compressImages(
  mediaIds: string[],
  quality: number,
  maxDimension: number | undefined,
  destination: string,
): Promise<BatchReport> {
  return invoke("compress_images", { mediaIds, quality, maxDimension, destination });
}

/** Writes converted copies into `destination`. Originals are untouched. */
export function convertImages(
  mediaIds: string[],
  format: ConvertFormat,
  destination: string,
): Promise<BatchReport> {
  return invoke("convert_images", { mediaIds, format, destination });
}

/* -------------------------------------------------------------- utilities -- */

export function scanLibraryHealth(): Promise<LibraryHealth> {
  return invoke("scan_library_health");
}

/** Drops rows whose file is gone. Resolves with the number removed. */
export function removeMissingEntries(): Promise<number> {
  return invoke("remove_missing_entries");
}

export function getStorageStats(): Promise<StorageStats> {
  return invoke("get_storage_stats");
}

/** Deletes every generated thumbnail. Resolves with the bytes freed. */
export function clearThumbnailCache(): Promise<number> {
  return invoke("clear_thumbnail_cache");
}

export function exportMedia(mediaIds: string[], destination: string): Promise<ExportReport> {
  return invoke("export_media", { mediaIds, destination });
}

/* ----------------------------------------------------------------- backup -- */

export function backupLibrary(destination: string): Promise<BackupInfo> {
  return invoke("backup_library", { destination });
}

export function inspectBackup(backupPath: string): Promise<BackupInfo> {
  return invoke("inspect_backup", { backupPath });
}

/** Stages a restore; it is applied the next time Hive starts. */
export function restoreLibrary(backupPath: string): Promise<string> {
  return invoke("restore_library", { backupPath });
}

export function cancelPendingRestore(): Promise<boolean> {
  return invoke("cancel_pending_restore");
}

export function hasPendingRestore(): Promise<boolean> {
  return invoke("has_pending_restore");
}

/* --------------------------------------------------------------- organize -- */
/* Grouping that needs no model: dates, gaps and distances.                    */

export function getTimeline(granularity: TimelineGranularity = "year"): Promise<TimelineBucket[]> {
  return invoke("get_timeline", { granularity });
}

export function listMediaInBucket(
  granularity: TimelineGranularity,
  key: string,
  limit = 500,
): Promise<MediaItem[]> {
  return invoke("list_media_in_bucket", { granularity, key, limit });
}

/** Bursts of photos separated by quiet gaps. */
export function detectEvents(gapHours?: number, minItems?: number): Promise<EventGroup[]> {
  return invoke("detect_events", { gapHours, minItems });
}

/** Events that also happened far from where most of your photos are taken. */
export function detectTrips(minDistanceKm?: number): Promise<EventGroup[]> {
  return invoke("detect_trips", { minDistanceKm });
}

/* ---------------------------------------------------------------- quality -- */

/** Scores sharpness for anything unmeasured, then lists what falls below `threshold`. */
export function scanBlur(threshold?: number, rescan?: boolean): Promise<BlurReport> {
  return invoke("scan_blur", { threshold, rescan });
}

/** Whether ffmpeg is on PATH, which is what video compression needs. */
export function videoToolsAvailable(): Promise<boolean> {
  return invoke("video_tools_available");
}

/**
 * Re-encodes videos into `destination`. `quality` is a CRF value: 18 is
 * visually lossless, 28 noticeably compressed. Originals are untouched.
 */
export function compressVideos(
  mediaIds: string[],
  quality: number,
  maxHeight: number | undefined,
  destination: string,
): Promise<BatchReport> {
  return invoke("compress_videos", { mediaIds, quality, maxHeight, destination });
}

/* ---------------------------------------------------------------- geocode -- */
/* The only feature that leaves this machine, and it is off until enabled.      */

export function getGeocodingEnabled(): Promise<boolean> {
  return invoke("get_geocoding_enabled");
}

export function setGeocodingEnabled(enabled: boolean): Promise<void> {
  return invoke("set_geocoding_enabled", { enabled });
}

/** Everything already looked up â€” reads the cache, never the network. */
export function getCachedPlaceNames(): Promise<[number, number, string][]> {
  return invoke("get_cached_place_names");
}

/**
 * Names a batch of coordinates, using the cache first. Rejects when the feature
 * is disabled, so nothing can reach the network by accident.
 */
export function lookupPlaceNames(
  coordinates: [number, number][],
): Promise<[number, number, string][]> {
  return invoke("lookup_place_names", { coordinates });
}

/* ------------------------------------------------------------ preferences -- */

/** 0 means no ceiling on the thumbnail cache. */
export function getNsfwPolicy(): Promise<NsfwPolicy> {
  return invoke("get_nsfw_policy");
}

export function setNsfwPolicy(threshold: number, autoHide: boolean): Promise<void> {
  return invoke("set_nsfw_policy", { threshold, autoHide });
}

export function getCacheLimitMb(): Promise<number> {
  return invoke("get_cache_limit_mb");
}

export function setCacheLimitMb(megabytes: number): Promise<void> {
  return invoke("set_cache_limit_mb", { megabytes });
}

/** Trims the least recently used thumbnails until the cache fits its limit. */
export function applyCacheLimit(): Promise<CacheReport> {
  return invoke("apply_cache_limit");
}

/** Only the bindings the user changed; the rest fall back to the defaults. */
export function getShortcutOverrides(): Promise<Record<string, string>> {
  return invoke("get_shortcut_overrides");
}

export function setShortcutOverrides(overrides: Record<string, string>): Promise<void> {
  return invoke("set_shortcut_overrides", { overrides });
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

export function backfillThumbnails(): Promise<void> {
  return invoke("backfill_thumbnails");
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
  // read_media_bytes returns a raw tauri::ipc::Response, so invoke() resolves to an
  // ArrayBuffer here instead of a JSON-decoded value.
  const buffer = await invoke<ArrayBuffer>("read_media_bytes", { mediaId, variant });
  const blob = new Blob([buffer]);
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

export function downloadOcrModels(): Promise<void> {
  return invoke("download_ocr_models");
}

export function backfillOcr(): Promise<void> {
  return invoke("backfill_ocr");
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

export function downloadFaceModels(): Promise<void> {
  return invoke("download_face_models");
}

export function backfillFaces(): Promise<void> {
  return invoke("backfill_faces");
}

export function listPeople(): Promise<PersonSummary[]> {
  return invoke("list_people");
}

export function renamePerson(personId: string, name: string): Promise<void> {
  return invoke("rename_person", { personId, name });
}

export function mergePeople(sourceId: string, targetId: string): Promise<void> {
  return invoke("merge_people", { sourceId, targetId });
}

export function getPersonMedia(personId: string): Promise<MediaItem[]> {
  return invoke("get_person_media", { personId });
}

export async function readFaceCropUrl(faceId: string): Promise<string> {
  const buffer = await invoke<ArrayBuffer>("read_face_crop_bytes", { faceId });
  const blob = new Blob([buffer]);
  return URL.createObjectURL(blob);
}

/* ---------------------------------------------------------------- tagging -- */

export function getTags(mediaId: string): Promise<TagResult[]> {
  return invoke("get_tags", { mediaId });
}

export function listAllTags(): Promise<TagSummary[]> {
  return invoke("list_all_tags");
}

export function listMediaByTag(tag: string, limit = 300): Promise<MediaItem[]> {
  return invoke("list_media_by_tag", { tag, limit });
}

export function backfillTags(): Promise<void> {
  return invoke("backfill_tags");
}

/* ----------------------------------------------------------- best photo -- */

export function selectBestPhoto(mediaIds: string[]): Promise<BestPhotoResult> {
  return invoke("select_best_photo", { mediaIds });
}

/* ------------------------------------------------------- smart albums -- */

export function createSmartAlbum(
  name: string,
  rules: SmartAlbumRule[],
  matchType: SmartAlbumMatch = "all"
): Promise<SmartAlbum> {
  return invoke("create_smart_album", { name, rules, matchType });
}

export function listSmartAlbums(): Promise<SmartAlbum[]> {
  return invoke("list_smart_albums");
}

export function getSmartAlbumMedia(
  albumId: string,
  limit = 300
): Promise<MediaItem[]> {
  return invoke("get_smart_album_media", { albumId, limit });
}

export function deleteSmartAlbum(albumId: string): Promise<void> {
  return invoke("delete_smart_album", { albumId });
}

export function suggestSmartAlbums(): Promise<SmartAlbumSuggestion[]> {
  return invoke("suggest_smart_albums");
}

/* ----------------------------------------------------------- aesthetic -- */

export function backfillAesthetic(): Promise<void> {
  return invoke("backfill_aesthetic");
}

export function getAestheticRanking(limit = 50): Promise<RankedItem[]> {
  return invoke("get_aesthetic_ranking", { limit });
}

/* ----------------------------------------------------------------- nsfw -- */

export function downloadNsfwModel(): Promise<void> {
  return invoke("download_nsfw_model");
}

export function backfillNsfw(): Promise<void> {
  return invoke("backfill_nsfw");
}

/* ------------------------------------------------------------- captions -- */

export function downloadCaptionModel(): Promise<void> {
  return invoke("download_caption_model");
}

export function backfillCaptions(): Promise<void> {
  return invoke("backfill_captions");
}

export function getCaption(mediaId: string): Promise<string | null> {
  return invoke("get_caption", { mediaId });
}
