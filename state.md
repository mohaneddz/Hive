# Hive — Project State & Comprehensive App Audit

**Snapshot Taken:** September 10, 2026 (Local)  
**App Version:** 0.1.0 (Tauri v2 + React 19 + Vite + Tailwind v4 + Rust)  
**Application Window State:** Active on Screen 3 (`DISPLAY5`, coordinates `X=-1340, Y=10`, maximized, dev server port `61427`)

---

## 1. Executive Summary

Hive is a desktop local-first photo and video library application built with Tauri v2, React 19, Tailwind CSS v4, and a pure Rust backend using SQLite (`rusqlite` with WAL mode). 

The application has achieved an exceptionally high degree of implementation across both local management features (indexing, thumbnails, EXIF parsing, albums, smart albums, backup/restore, batch tools) and on-device AI pipelines (CLIP embeddings, OCR, face detection/embedding, local LLM chat via `candle`, LAION aesthetic scoring, ViT-GPT2 captions, ViT NSFW filtering, and Laplacian variance sharpness).

However, an exhaustive end-to-end audit reveals specific **incomplete implementations, UI/UX bugs, routing friction, styling anomalies, and missing capabilities** that need to be resolved.

---

## 2. Feature Implementation Matrix

| Component / Feature | Backend (Rust) | Frontend (React) | Live Status | Notes & Gaps |
|---|---|---|---|---|
| **Window & Shell** | ✅ Tauri v2 / Tao | ✅ Custom Titlebar | 🟡 Functional | No double-click maximize on titlebar drag region; search in titlebar doesn't sync with Search page. |
| **Home Dashboard** | ✅ Multi-query stats & rails | ✅ Stat tiles & sections | 🟡 Functional | Subtitle cites "NIMA AI" instead of LAION; rails render as vertical stacked grids instead of carousels; no skeletons during initial load. |
| **Gallery Grid** | ✅ Paginated cursor/offset | ✅ Grid + Year scrubber | ✅ Working | Year scrubber only active on date sort; infinite scroll observer works; selection mode supports export/trash. |
| **Photo Viewer** | ✅ Byte streaming & detail | ✅ ViewerPage modal | 🔴 UX Bugs | Esc / Close button hardcodes `navigate("/")` instead of going back; moving to trash redirects to Home; slideshow has no speed controls. |
| **Photo Editor** | ✅ `image` crate operations | ✅ Canvas + sliders + crop | 🟡 Incomplete | Cropping has no resize handles; no Undo/Redo; Metadata "Save details" is disconnected from visual "Save…" dialog. |
| **Collections (Albums)** | ✅ SQLite album tables | ✅ Manual album view | ✅ Working | Full CRUD, cover assignment, multi-selection, and export supported. |
| **Collections (Smart Views)**| ✅ Scoped queries | ✅ Favorites/Hidden/Archive | ✅ Working | Filtering and scoping work correctly. |
| **Collections (Auto Groups)**| ✅ Date & distance clustering| ✅ Timeline/Events/Trips | ✅ Working | Pure algorithmic grouping without models; includes unspecified date bucket toggle. |
| **Smart Albums Engine** | ✅ SQL rule compiler (`all`/`any`)| ✅ Dialog & suggestions | 🟡 Incomplete | Cannot edit existing smart albums after creation (delete only); cannot change smart album cover image. |
| **People & Faces** | ✅ UltraFace + ArcFace int8 | ✅ PeoplePage & PersonCard | 🔴 Incomplete | `merge_people` is implemented in backend & frontend tauri.ts but **has zero UI**; no way to deselect a person or reassign false positives. |
| **Places & Map** | ✅ GPS clustering & geocoding| ✅ Equirectangular SVG | 🟡 Visual Gap | Equirectangular map is a blank coordinate grid with zero landmass outlines/tiles; pins float in void. |
| **Explorer** | ✅ Drive & filesystem walk | ✅ Drive/folder tree | 🟡 Basic | Cannot search or sort in current directory; shallow media count can prevent watching folders with nested media. |
| **Search (FTS5 & CLIP)** | ✅ FTS5 + ONNX CLIP | ✅ SearchPage + Chat Panel | 🟡 UX Friction | No input debouncing (triggers queries on every keystroke); 4-col results get severely squished on narrower windows beside chat. |
| **Gallery Chat (RAG)** | ✅ Qwen2.5-1.5B via `candle` | ✅ Chat panel in Search | ✅ Working | Works fully offline; context is currently metadata-only (does not yet ingest people names or place names). |
| **Utilities: Storage** | ✅ Directory byte calculator | ✅ Storage stats & breakdown | ✅ Working | Originals vs. thumbnails vs. database visual bar. |
| **Utilities: Duplicates** | ✅ phash + union-find | ✅ DuplicatesPanel | ✅ Working | Group review, keep-one, dismiss-group. |
| **Utilities: Out of Focus** | ✅ Laplacian variance filter | ✅ BlurPanel | ✅ Working | 3 sensitivity thresholds; instant switching on cached blur scores. |
| **Utilities: Health Scan** | ✅ Disk existence check | ✅ Health scan panel | ✅ Working | Missing & broken media detection with single-click index cleanup. |
| **Utilities: Batch Tools** | ✅ Rename, compress, convert | ✅ BatchPanel | ✅ Working | Token-based rename preview with collision detection; video compression depends on system ffmpeg. |
| **Utilities: Backup/Restore**| ✅ Staged SQLite swap | ✅ BackupPanel | ✅ Working | Staged restore applied cleanly before DB open on app restart. |
| **Trash Management** | ✅ Soft-delete (`is_trashed`) | ✅ TrashPage | 🟡 UX Friction | No multi-select; hardcoded 120 item page limit with no pagination or load more. |
| **Settings & Models** | ✅ Model downloader & jobs | ✅ Settings tabs | 🔴 Buggy UI | Caption & NSFW backfill buttons do not check if model is downloaded; errors silently swallowed without user feedback. |

---

## 3. Incomplete Implementations & Architectural Gaps

### 1. `merge_people` Has Zero UI
* **Location:** `src-tauri/src/commands/faces.rs` (lines 135–180), `src/lib/tauri.ts` (line 503).
* **The Gap:** The backend command `merge_people(source_id, target_id)` correctly moves all faces, updates media references, and removes the redundant person row. Furthermore, `tauri.ts` exports `mergePeople()`. However, in `PeoplePage.tsx`, there is **no UI affordance** (no drag-and-drop, no context menu, and no "Merge into..." dialog). If two clusters represent the same person, the user is powerless to combine them.

### 2. Smart Albums Cannot Be Edited
* **Location:** `src/pages/CollectionsPage.tsx`, `src/components/collections/SmartAlbumDialog.tsx`.
* **The Gap:** Users can create smart albums and delete them. However, unlike manual albums which offer an edit input, smart albums have no "Edit Rules" or "Edit Name" action. To modify a rule (e.g. changing an aesthetic threshold or adding a tag), the user must delete the album and reconstruct all rules from scratch.

### 3. Disconnected Image Editor Metadata vs. Visual Operations
* **Location:** `src/pages/EditorPage.tsx` (lines 190–220).
* **The Gap:** The Editor has 4 tabs: *Filters*, *Adjust*, *Frame (Crop)*, and *Details (Metadata)*. 
  * The top "Save…" button triggers `applyEdits()` which writes pixel changes (crop, rotation, sliders) to disk.
  * The "Details" tab has an isolated "Save details" button that calls `updateMediaMetadata()`.
  * If a user adjusts a brightness slider and types a new title, clicking the prominent top "Save…" button saves the image but **silently discards the metadata edits**.

### 4. Trash Page Lacks Multi-Selection & Pagination
* **Location:** `src/pages/TrashPage.tsx` (lines 14–35).
* **The Gap:** 
  * `PAGE_SIZE` is hardcoded to 120 with no infinite scroll sentinel or "Load more" button. If a user deletes 300 photos, items 121–300 cannot be inspected or restored until the first 120 are permanently emptied.
  * Users must either restore/delete items one by one, or empty all items. There is no checkbox selection to restore a specific batch.

### 5. Format Support Blindspots (HEIC, AVIF, RAW)
* **Location:** `src-tauri/src/indexing.rs` (line 10).
* **The Gap:** `IMAGE_EXTS` is limited to:
  ```rust
  const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"];
  ```
  Modern mobile and professional photo formats are ignored by the indexer:
  * Apple iPhone photos: `.heic`, `.heif`
  * Modern web image format: `.avif` (even though `ravif` is a dependency!)
  * Camera RAW formats: `.cr2`, `.nef`, `.arw`, `.dng`

---

## 4. Bugs & Bad User Experience / Routes

### 1. Navigation Escape Trap in ViewerPage
* **Location:** `src/pages/ViewerPage.tsx` (lines 142, 251, 339).
* **Bug Details:**
  * Pressing `Esc` or clicking the top-left `Close` button executes `navigate("/")` (Home).
  * Trashing a photo inside the viewer executes `navigate("/")`.
  * **Consequence:** If a user opens photo #45 while browsing `/gallery`, an album `/albums/xyz`, or search `/search?q=cats`, exiting the viewer throws them back to the Home page, losing their scroll position and search context.
  * **Fix:** Change to `navigate(-1)` with a fallback to `"/gallery"`.

### 2. Search Input Fires Un-Debounced IPC Queries on Every Keystroke
* **Location:** `src/pages/SearchPage.tsx` (lines 76–102).
* **Bug Details:** `useEffect` triggers on `[query, mediaType, favoritesOnly, mode]`. Every single letter typed in the search box immediately invokes `searchMedia` (FTS5) or `semanticSearch` (CLIP ONNX model forward pass).
* **Consequence:** In semantic mode, typing a 5-word sentence triggers 25 heavy ONNX neural-network text-encoder inference passes in rapid succession, lagging the main thread.
* **Fix:** Introduce a 250ms–350ms debounce on `query`.

### 3. Silently Swallowed Failures in AI Settings
* **Location:** `src/pages/GallerySettingsPage.tsx` (lines 262–292).
* **Bug Details:** `startCaptionBackfill` and `startNsfwBackfill` call backend commands that return an `Err` if the required ONNX model is missing. The frontend `try...catch` blocks do NOT set `failure`, show a toast, or notify the user.
* **Consequence:** If the user clicks "Generate captions" without downloading the model first, the button spins for a split-second and stops with zero feedback.
* **Fix:** Disable the backfill buttons when `!aiStatus.captionModelsReady` and display the error message via `setFailure()`.

### 4. Inaccurate Model Attribution on Home Screen
* **Location:** `src/pages/HomePage.tsx` (line 203).
* **Issue:** The section header subtitle reads:
  ```tsx
  subtitle="Highest quality photos scored by NIMA AI"
  ```
  The app does not use Google's NIMA model; it uses the **LAION Aesthetic Predictor** (a linear probe over CLIP ViT embeddings).

### 5. Titlebar Window Controls Lack Native Double-Click to Maximize
* **Location:** `src/components/layout/Titlebar.tsx` (lines 22–25).
* **Issue:** Because `tauri.conf.json` specifies `"decorations": false`, the app uses an HTML titlebar. While `data-tauri-drag-region` enables dragging, double-clicking the titlebar drag region does not maximize or restore the window on Windows.
* **Fix:** Add `onDoubleClick={toggleMaximize}` to the `<header data-tauri-drag-region>` element.

### 6. Crop Tool in Editor Has No Resizing Handles
* **Location:** `src/pages/EditorPage.tsx` (lines 145–187).
* **Issue:** The crop box is drawn by dragging a rectangle. Once drawn, there are no corner or side handles to tweak the bounds; any click immediately discards the current box and begins a brand new rectangle.

### 7. Sequential IPC Calls in Album Item Removal
* **Location:** `src/pages/AlbumDetailPage.tsx` (lines 80–82).
* **Issue:** When removing multiple selected items from an album:
  ```typescript
  for (const mediaId of selected) {
    await removeMediaFromAlbum(id, mediaId);
  }
  ```
  Removing 100 photos fires 100 sequential IPC invocations, freezing UI responsiveness.
  * **Fix:** Use `Promise.all()` or a bulk backend command `remove_media_from_album_bulk`.

---

## 5. Visual Polish & Layout Recommendations

### 1. Places Page Equirectangular MiniMap Lacks Context
* **Current:** An equirectangular grid SVG with simple coordinate lines every 30° (`viewBox="0 0 360 180"`).
* **Issue:** The map contains no continents, coastlines, or political boundaries. Photo clusters appear as floating circles in empty space.
* **Suggestion:** Embed a lightweight, offline GeoJSON or SVG path of world landmasses, or integrate an offline vector/raster map tile engine (like local PMTiles or OpenStreetMap tile caching).

### 2. Home Dashboard Rails
* **Current:** Each section (On This Day, Continue Viewing, Top Aesthetic, Favorites, Recently Added) renders a vertical 4-column `MediaGrid`.
* **Issue:** Vertically stacking 5 full grids creates an extremely tall page that requires excessive scrolling.
* **Suggestion:** Convert Home sections into horizontal scrolling rails (filmstrips) with smooth scroll buttons and a "See all (N)" link to open the full grid in Gallery/Collections.

### 3. Responsive Breakpoint in Search Page
* **Current:** `grid grid-cols-[1fr_320px] gap-6` with a hardcoded `grid-cols-4` results grid.
* **Issue:** On the minimum supported window width (900px) or split-screen views, the chat panel takes 320px, leaving only ~500px for 4 photo cards. The cards become tiny squares where metadata badges overlap.
* **Suggestion:** Use responsive grid classes (`grid-cols-2 lg:grid-cols-3 xl:grid-cols-4`) and allow collapsing the Gallery Chat panel into a slide-over drawer on smaller viewports.

### 4. Skeleton Loading States
* **Current:** Several pages (`HomePage`, `AlbumDetailPage`, `CollectionsPage`) render `"Loading…"` plain text or an empty container while loading data.
* **Suggestion:** Standardize on animated skeleton cards matching the exact aspect ratio of the photo grid.

---

## 6. Performance & Architecture Considerations

1. **Vite Bundle Splitting:**
   Vite warns that `dist/assets/index-BkctXkB-.js` is **569 kB** minified. Pages and heavy components (`ViewerPage`, `EditorPage`, `PlacesPage`, `GallerySettingsPage`, `GalleryChatPanel`) should be dynamically imported using `React.lazy()` and `Suspense`.

2. **RAG Context Enrichment for Chat:**
   `src-tauri/src/commands/chat.rs` currently supplies filename, capture date, camera make/model, and OCR text to Qwen2.5. It does not include:
   * Recognized people names (`people` table)
   * Reverse-geocoded place names (`geocode_cache` table)
   * Auto-generated tags (`tags` table)
   * AI captions (`captions` table)
   Adding these signals will make chat answers far more accurate (e.g., answering "Show me photos of Sarah in Italy eating pasta").

3. **In-Memory Model Management:**
   Currently, CLIP, OCR, Face Detection, and LLM can all be loaded into memory simultaneously if enabled. On 8 GB or 16 GB machines, this can use 2.5 GB+ of RAM. Adding an idle timeout or LRU eviction for ONNX sessions when not actively running queries would reduce Hive's resident memory footprint.

---

## 7. Actionable Roadmap & Priority Task List

### Phase 1: High Priority Bug Fixes (Next Session)
- [ ] Fix viewer navigation: Replace `navigate("/")` in `ViewerPage.tsx` with `navigate(-1)` to preserve user context.
- [ ] Add 300ms debounce to the search input in `SearchPage.tsx`.
- [ ] Add `onDoubleClick={toggleMaximize}` to `Titlebar.tsx`.
- [ ] Add error handling and disable state to Caption and NSFW backfill buttons in `GallerySettingsPage.tsx`.
- [ ] Correct "NIMA AI" typo to "LAION Aesthetic" on `HomePage.tsx`.
- [ ] Batch `removeMediaFromAlbum` calls in `AlbumDetailPage.tsx` with `Promise.all()`.

### Phase 2: Complete Missing Features
- [ ] Build **Face Cluster Merging UI** in `PeoplePage.tsx` (leveraging `merge_people`).
- [ ] Add **Smart Album Editing**: Enable modifying rules and renaming smart albums from `CollectionsPage.tsx`.
- [ ] Wire **Editor Metadata Saving**: Unify visual edits and metadata updates in `EditorPage.tsx` under a single cohesive save flow.
- [ ] Add **Multi-Select & Pagination** in `TrashPage.tsx`.
- [ ] Add crop handle dragging in `EditorPage.tsx`.

### Phase 3: Format Expansion & AI Enrichment
- [ ] Expand `is_supported_media` in `indexing.rs` to support `.avif`, `.heic` (via `libheif` / `image` crate), and common RAW extensions.
- [ ] Ingest People, Places, Tags, and Captions into the Gallery Chat RAG retrieval context in `chat.rs`.
- [ ] Add world landmass vector paths to `MiniMap` on `PlacesPage.tsx`.
- [ ] Convert Home sections into horizontal scrolling rails.
