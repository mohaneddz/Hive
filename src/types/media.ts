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
  trashedAt: string | null;
  isHidden: boolean;
  isArchived: boolean;
  lastViewedAt: string | null;
  title: string | null;
  description: string | null;
  /** A capture date the user corrected by hand; wins over `takenAt`. */
  takenAtOverride: string | null;
  editedAt: string | null;
  /** 0..1 from the sensitive-content model, or null if never scanned. */
  nsfwScore: number | null;
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
  imageCount: number;
  videoCount: number;
  albumCount: number;
  placeCount: number;
  hiddenCount: number;
  archivedCount: number;
  folderCount: number;
}

/** Mutually exclusive slices of the library. Mirrors `scope_predicate` in Rust. */
export type MediaScope = "library" | "trash" | "hidden" | "archive" | "all";

export type MediaSort = "taken" | "oldest" | "added" | "name" | "size" | "viewed";

export interface Album {
  id: string;
  name: string;
  description: string | null;
  coverMediaId: string | null;
  itemCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface PlaceGroup {
  id: string;
  lat: number;
  lon: number;
  count: number;
  coverMediaId: string;
  earliest: string | null;
  latest: string | null;
}

export interface ExplorerEntry {
  name: string;
  path: string;
  isDirectory: boolean;
  mediaCount: number;
  indexedCount: number;
  isWatched: boolean;
}

export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Applied in a fixed order — rotate → flip → crop → colour — matching
 * `apply_ops` in Rust, so the live preview and the written file agree.
 */
export interface EditOps {
  /** 0, 90, 180 or 270, clockwise. */
  rotation: number;
  flipHorizontal: boolean;
  flipVertical: boolean;
  /** Fractions of the rotated image, 0..1. `null` keeps the whole frame. */
  crop: CropRect | null;
  /** 1 leaves the channel untouched. */
  brightness: number;
  contrast: number;
  saturation: number;
  /** 0 leaves colour alone, 1 is fully black and white. */
  grayscale: number;
  /** 0 leaves colour alone, 1 is a full sepia tone. */
  sepia: number;
  /** −1 cools the image towards blue, +1 warms it towards orange. */
  temperature: number;
}

export const NEUTRAL_EDIT_OPS: EditOps = {
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
  crop: null,
  brightness: 1,
  contrast: 1,
  saturation: 1,
  grayscale: 0,
  sepia: 0,
  temperature: 0,
};

/** Named starting points. Each one only sets the colour fields it cares about. */
export const FILTER_PRESETS: { name: string; ops: Partial<EditOps> }[] = [
  { name: "Original", ops: {} },
  { name: "Black & white", ops: { grayscale: 1, contrast: 1.1 } },
  { name: "Sepia", ops: { sepia: 0.85, brightness: 1.05 } },
  { name: "Vivid", ops: { saturation: 1.45, contrast: 1.12 } },
  { name: "Muted", ops: { saturation: 0.6, contrast: 0.95 } },
  { name: "Warm", ops: { temperature: 0.45, saturation: 1.1 } },
  { name: "Cool", ops: { temperature: -0.45, saturation: 1.05 } },
  { name: "Faded", ops: { contrast: 0.78, brightness: 1.08, saturation: 0.85 } },
];

/** The user's answer to "keep the original?". */
export type SaveMode = "copy" | "overwrite";

export interface BatchReport {
  processed: number;
  skipped: number;
  failed: number;
  /** Only meaningful for compression and conversion. */
  bytesBefore: number;
  bytesAfter: number;
  destination: string | null;
  firstError: string | null;
}

export interface RenamePreview {
  mediaId: string;
  from: string;
  to: string;
  /** The target name already exists, or repeats inside the same batch. */
  conflict: boolean;
}

export type ConvertFormat = "jpg" | "png" | "webp";

export interface BackupInfo {
  path: string;
  bytes: number;
  createdAt: string;
  itemCount: number;
}

export interface FolderUsage {
  folderId: string;
  name: string;
  path: string;
  itemCount: number;
  bytes: number;
}

export interface StorageStats {
  totalItems: number;
  imageCount: number;
  videoCount: number;
  originalBytes: number;
  thumbnailBytes: number;
  databaseBytes: number;
  byFolder: FolderUsage[];
}

export interface LibraryHealth {
  checked: number;
  missing: MediaItem[];
  broken: MediaItem[];
}

export interface ExportReport {
  exported: number;
  skipped: number;
  destination: string;
}

export type TimelineGranularity = "year" | "month" | "day";

export interface TimelineBucket {
  /** The value `strftime` produced, e.g. "2024" or "2024-06". */
  key: string;
  label: string;
  count: number;
  coverMediaId: string | null;
  start: string;
  end: string;
}

/**
 * A run of photos taken close together. `isTrip` marks the ones that also
 * happened far from home; `distanceKm` is how far.
 */
export interface EventGroup {
  id: string;
  start: string;
  end: string;
  count: number;
  coverMediaId: string;
  mediaIds: string[];
  lat: number | null;
  lon: number | null;
  isTrip: boolean;
  distanceKm: number;
}

/** A photo and its sharpness score. Lower means softer. */
export interface BlurryItem {
  item: MediaItem;
  score: number;
}

/**
 * What the app does with the sensitive-content score. Lives in the backend so the
 * blur in the grid and the auto-hide decision read the same number.
 */
export interface NsfwPolicy {
  /** Score at or above which a photo is treated as sensitive, 0..1. */
  threshold: number;
  /** Whether scoring also moves those photos out of the library into Hidden. */
  autoHide: boolean;
}

/** State of the thumbnail cache. `limitBytes` of 0 means no ceiling is set. */
export interface CacheReport {
  usedBytes: number;
  limitBytes: number;
  freedBytes: number;
  removed: number;
}

export interface BlurReport {
  /** How many photos were measured during this run. */
  scanned: number;
  /** How many have a score at all, including from earlier runs. */
  measured: number;
  threshold: number;
  items: BlurryItem[];
}

export interface AiStatus {
  modelsReady: boolean;
  modelLoaded: boolean;
  embeddedCount: number;
  eligibleCount: number;
  ocrModelsReady: boolean;
  ocrModelLoaded: boolean;
  ocrIndexedCount: number;
  faceModelsReady: boolean;
  faceModelLoaded: boolean;
  facesIndexedCount: number;
  peopleCount: number;
  /** Whether each optional model is on disk — drives the Download button. */
  nsfwModelsReady: boolean;
  captionModelsReady: boolean;
}

export interface DuplicateGroup {
  groupId: string;
  items: MediaItem[];
}

export interface PersonSummary {
  id: string;
  name: string | null;
  faceCount: number;
  coverFaceId: string;
}

export interface TagResult {
  tag: string;
  confidence: number;
}

export interface TagSummary {
  tag: string;
  count: number;
  coverMediaId: string | null;
}

/**
 * One photo's card in a best-photo result. Every score is 0..1 and relative to
 * its group: 1 means "best of these photos", not "good in absolute terms".
 */
export interface PhotoRanking {
  mediaId: string;
  sharpnessScore: number;
  aestheticScore: number;
  representativenessScore: number;
  totalScore: number;
}

export interface BestPhotoResult {
  bestMediaId: string;
  item: MediaItem;
  score: number;
  rankings: PhotoRanking[];
}

/** Mirrors `SUPPORTED_RULES` in `smart_albums.rs`. */
export type SmartAlbumRuleKind =
  | "tag"
  | "person"
  | "media_type"
  | "favorite"
  | "date_range"
  | "place"
  | "camera"
  | "caption"
  | "filename"
  | "aesthetic"
  | "blur"
  | "nsfw";

export interface SmartAlbumRule {
  kind: SmartAlbumRuleKind | string;
  /** An operator the kind accepts, optionally prefixed with `not_` to invert it. */
  operator: string;
  value: string;
}

/** `all` demands every rule, `any` a single one. */
export type SmartAlbumMatch = "all" | "any";

export interface SmartAlbum {
  id: string;
  name: string;
  rules: SmartAlbumRule[];
  matchType: SmartAlbumMatch;
  coverMediaId: string | null;
  itemCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface SmartAlbumSuggestion {
  name: string;
  rules: SmartAlbumRule[];
  matchType: SmartAlbumMatch;
  previewCount: number;
}

export interface RankedItem {
  item: MediaItem;
  aestheticScore: number;
}

export interface CaptionResult {
  mediaId: string;
  text: string;
}

/** The AI editor's tools. Each downloads its own model on first use. */
export type AiEditorTool = "upscale" | "cutout" | "segment" | "inpaint" | "generate";

export interface AiEditorStatus {
  upscaleReady: boolean;
  cutoutReady: boolean;
  segmentReady: boolean;
  inpaintReady: boolean;
  generateReady: boolean;
  /** "DirectML", or null when everything runs on the processor. */
  gpuBackend: string | null;
}

/**
 * The graphics card. Off unless switched on: a driver fault closes Hive with
 * nothing to catch, so the speed is the user's choice to make.
 */
export interface GpuStatus {
  /** Whether a usable card was found at all. */
  available: boolean;
  enabled: boolean;
}

/** A click in the photo's own pixel coordinates. */
export interface SelectPoint {
  x: number;
  y: number;
  /** False means "not this" — carves back a selection that grabbed too much. */
  positive: boolean;
}

/**
 * An AI result computed and shown, but not written to disk. Held by the backend
 * until Save, so the tools behave like the sliders: previewed, undoable, and
 * committed once.
 */
export interface AiPreview {
  /** PNG bytes, scaled down for display. */
  preview: number[];
  /** The full-resolution size that would be written. */
  width: number;
  height: number;
  /** The tools applied so far, oldest first. */
  steps: string[];
}
