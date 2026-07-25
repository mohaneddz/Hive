export interface Folder {
  id: string;
  path: string;
  name: string;
  isWatched: boolean;
  addedAt: string;
}

export interface ExifData {
  cameraMake: string | null;
  cameraModel: string | null;
  lens: string | null;
  iso: number | null;
  fNumber: number | null;
  exposureTime: string | null;
  focalLength: number | null;
  gpsLat: number | null;
  gpsLon: number | null;
}

export type MediaType = "image" | "video";

export interface MediaItem {
  id: string;
  folderId: string;
  path: string;
  filename: string;
  hash: string;
  size: number;
  width: number | null;
  height: number | null;
  durationMs: number | null;
  mimeType: string;
  mediaType: MediaType;
  takenAt: string | null;
  createdAt: string;
  modifiedAt: string;
  indexedAt: string;
  isFavorite: boolean;
  isTrashed: boolean;
  exif: ExifData | null;
  thumbnailPath: string | null;
}

export interface MediaPage {
  items: MediaItem[];
  total: number;
}

export interface JobProgress {
  id: string;
  kind: string;
  status: "running" | "completed" | "failed" | "cancelled";
  current: number;
  total: number;
  message: string | null;
}

export interface FolderStats {
  folder: Folder;
  itemCount: number;
  coverMediaId: string | null;
}

export interface PlaceCluster {
  lat: number;
  lon: number;
  count: number;
  coverMediaId: string;
}

export interface LibraryStats {
  totalItems: number;
  totalBytes: number;
  favorites: number;
  trashed: number;
}

export interface AiStatus {
  modelsReady: boolean;
  modelLoaded: boolean;
  embeddedCount: number;
  eligibleCount: number;
}

export interface DuplicateGroup {
  groupId: string;
  items: MediaItem[];
}
