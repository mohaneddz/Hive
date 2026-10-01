# Hive UI, Feature, and Design-Alignment Audit

**Date:** 2026-09-13  
**Scope:** Interactive local-build QA pass, source cross-check, and comparison with `design/app/*.png`  
**Build health:** `pnpm typecheck` passed; `cargo check --manifest-path src-tauri/Cargo.toml` passed.

## Executive summary

Hive has a polished visual language and a broad feature surface, but it is not yet ready to be finalized against the supplied product designs. The largest release blockers are incomplete recovery/error states, several missing editing/management workflows, and a home experience that does not approach the supplied design once real content is expected.

The test machine's native Tauri window was not exposed to the Windows automation bridge. I therefore exercised the running local web build at `http://localhost:61427` and cross-checked the Tauri command wiring in source. Native-only operations (folder dialogs, filesystem indexing, model downloads, destructive operations) cannot be fully certified from this browser-hosted run. They are called out separately rather than claimed as passed.

## What was exercised

- Navigation: Home, Gallery, Collections, People, Places, Explorer, Search, Utilities, Trash, Settings.
- Empty-library/onboarding states and repeated **Add folder** entry points.
- Search mode switch (Exact/Semantic), free-text input, media filters, sort and orientation controls.
- Collections: New Album dialog, cancel path, Smart Album dialog and rule controls.
- Settings: AI download/backfill affordances, theme controls, cache controls, shortcut controls, privacy toggles.
- Visual comparison: running Home/Settings against `design/app/home.png` plus the shared layout language.

## Browser-control coverage (continued pass)

The following controls have now been opened or state-switched in the browser build. This is intentionally more granular than the first pass; it is not limited to Home.

| Area | Exercised controls | Result / limitation |
|---|---|---|
| Global shell | Every sidebar route, title-bar search, Settings & shortcuts, unknown route | Navigation works; global search correctly requires Enter and opens `#/search?q=…`; 404 has a recovery link. |
| Search | Exact/Semantic, Photos/Videos, query input, clear filters, result sort/orientation affordances | All client-state controls render and switch. Result correctness needs native indexed fixtures. |
| Collections | New Album open/cancel, New Smart Album dialog/rule controls, Timeline/Events/Trips navigation, Timeline Year/Month/Day selection | Dialog states work. Group contents need a native populated library. |
| People | Empty state and Scan affordance | Face scan itself is native/job-backed and must be smoke-tested in Tauri. |
| Places | Precision controls and empty/loading surface | Pins, map controls, geocoding, and map links require fixture GPS data and native state. |
| Explorer | Drives/root view and folder-watch entry point | Native filesystem enumerator and folder selection require final Tauri testing. |
| Gallery / Trash | Empty states and onboarding actions | Selection, viewer, restore, delete, and pagination require fixture media; browser cannot invoke backend. |
| Utilities | Storage, Duplicates, Out of focus, Missing & broken, Batch, Export, Backup tabs; blur sensitivity; Batch Rename/Compress modes | Tab and local state work. Scans, cache clearing, export, batch writes, and restore are intentionally deferred: they mutate data or need Rust. |
| Settings | Light/Dark theme (then restored to Dark), AI feature controls, storage-limit choices, shortcuts, privacy controls | Theme works visually. Downloads, scans, cache clearing, filesystem settings, and geocoding must be verified natively. |
| Error recovery | Unknown route | 404 screen and Back to overview link render correctly. |

### Controls deliberately not invoked during browser QA

I did not click actions that delete data, write/rename user files, restore a backup, download multi-GB models, open an OS folder picker, or transmit coordinates. Those need an isolated native fixture library and, for destructive actions, confirmation immediately before the action. They are not omissions from the audit; they are explicitly queued for the final Tauri smoke pass.

## Release blockers

1. **The UI can remain in a permanent loading state with no recovery path.**
   - Evidence: Home rendered only `Loading your library…`; Explorer and Places also displayed `Loading…` in the local app shell when their native command calls were unavailable.
   - Impact: a command failure, unavailable DB, or startup race looks like a frozen app instead of an actionable problem.
   - Required fix: introduce a standard async state for every page: skeleton while pending, concise error panel on rejection, retry action, and diagnostic details only where useful.

2. **Home is not aligned with the supplied Home design.**
   - The design contains a strong continue-viewing hero, a recent-photo rail, memories, favourites, and an activity panel. The implementation's intended home content is structured as generic section grids; its empty/loading state leaves almost all usable canvas blank.
   - Required fix: implement the designed hero/activity layout (or explicitly replace the design), use horizontal rails, and make the first-run state a designed onboarding composition rather than a blank page with one button.

3. **Viewer exit/trash loses user context.**
   - `ViewerPage.tsx` navigates to `/` in the escape, close, and trash paths. Opening an image from Gallery, Search, or an album sends the user to Home rather than back to its source.
   - Required fix: retain an origin route/state; use history back with a safe Gallery fallback.

4. **People clustering cannot be corrected.**
   - The backend and TypeScript bridge expose `merge_people`/`mergePeople`, but there is no People UI for merge, deselect, or false-positive reassignment.
   - Required fix: add person multi-select/context actions, a merge-into dialog, and correction/reassignment paths.

5. **Trash cannot handle a real library.**
   - It loads a fixed 120 items (`TrashPage.tsx`) with no pagination/infinite loading and provides no selection/batch restore/delete.
   - Required fix: use the shared paginated media pattern and selection toolbar; keep destructive confirmation clearly scoped.

6. **Editor save is split and can silently discard metadata changes.**
   - The prominent visual Save flow and Details tab's separate `Save details` flow are independent.
   - Required fix: one save transaction/confirmation that persists all dirty visual and metadata fields, with dirty-state and error feedback.

## High-priority UI and behavior issues

| Priority | Finding | Evidence / consequence | Recommendation |
|---|---|---|---|
| P1 | Search queries are not debounced | Each keystroke updates query state; semantic search may run costly CLIP work repeatedly. | Debounce 250–350 ms, cancel stale requests, and show a small pending state. |
| P1 | AI backfill actions lack reliable prerequisite feedback | Caption and NSFW backfill handlers swallow failures; buttons are presented even when their models are not ready. | Disable with an explanatory label until downloaded; surface command errors in the existing failure UI. |
| P1 | Smart albums cannot be edited | Creation is present; no edit name/rules/cover action exists for existing smart albums. | Reuse the dialog in edit mode and preserve the existing rule structure. |
| P1 | Editor crop cannot be refined | Crop rectangle is redrawn rather than resized with handles. | Add corner/edge handles, constrained aspect ratio options, undo/redo, and keyboard nudging. |
| P1 | Search results are too constrained at narrow widths | A fixed 320px chat column plus four result columns produces tiny cards. | Collapse chat into a drawer below a breakpoint and use responsive 2/3/4-column grids. |
| P1 | Places lacks real geographic context | The map is a coordinate grid; pins have no landmass/coastline context. | Add an offline world SVG/GeoJSON base layer or an intentional map provider/cache strategy. |
| P2 | Home says `NIMA AI` but implementation uses LAION aesthetic scoring | `HomePage.tsx`. | Correct attribution to LAION Aesthetic Predictor. |
| P2 | Custom title bar misses familiar maximize behavior | Frameless Tauri title bar does not wire double-click on the drag region. | Bind titlebar double-click to toggle maximize/restore. |
| P2 | Album bulk removal is serial | One IPC call is awaited for each selected image. | Add a bulk command or dispatch safely in parallel with progress/error reporting. |
| P2 | Explorer lacks search/sort and can miss nested media when deciding whether to watch | Basic browsing is not sufficient for deep photo folders. | Add search, sort, recursive count, and clear indexing status. |
| P2 | Home rails are vertically stacked grids | This conflicts with the rail-first supplied design and produces very long pages. | Use horizontal rails with arrows and `View all` links. |
| P2 | Loading UI is inconsistent/plain | Several pages use bare text such as `Loading…`. | Establish skeletons matching every page's final card/grid geometry. |

## Missing or incomplete product capabilities

- **Format coverage:** Indexer support excludes HEIC/HEIF, AVIF, and common camera RAW formats (CR2, NEF, ARW, DNG). This is a significant gap for a modern photo library.
- **Person corrections:** Beyond merging, users need false-positive removal/reassignment and an ability to mark a cluster as not-a-person.
- **Smart album management:** Editing rules and name, cover selection, duplicate rule validation, and a useful empty-state preview are absent.
- **Viewer:** no slideshow speed control; exit behavior is incorrect; verify that back/forward preserve filter and scroll context.
- **Gallery chat retrieval:** currently lacks the useful people, place, tag, and caption signals needed for natural questions about the library.
- **Model resource management:** multiple local AI models can stay loaded concurrently. Add idle unload/LRU handling and expose approximate memory use.
- **Accessibility:** visible icon-only controls should have checked, descriptive accessible names; perform a screen-reader and keyboard-focus pass after visual changes.

## Design-alignment observations

### What is working

- Dark gallery palette, amber accent, rounded dark surfaces, iconography, sidebar hierarchy, and typography are coherently applied.
- Settings is visually closest to the design system: cards, section headings, and primary calls to action look deliberate.
- The empty states provide direct entry points to adding a folder, rather than dead ends.

### Where it diverges

- The supplied Home design is content-led, dense, and visually memorable. The implementation home is generic and, while loading or empty, mostly blank.
- The design's top bar gives status/activity useful space; the current bar compresses branding and search and does not present equivalent library activity.
- Design reference uses rich thumbnail rails and card composition. Current home sections rely on vertical grids, so the information architecture and rhythm do not match.
- Empty states should still preserve the design's composition: illustration/preview rail, a short three-step onboarding path, and a visible explanation of local indexing would avoid the blank-canvas impression.

### Responsive check (900px desktop width)

The compact-desktop Search layout was exercised at **900 × 800**. The persistent sidebar, fixed chat card, and left-side search controls leave the result canvas very narrow; the heading breaks to three lines and the filter controls become a tall single column. This confirms the responsive issue is visible in the rendered UI, not just an inferred CSS concern. At this width, collapse the chat pane to a drawer or below the results and let the content area use the full available width.

### Global search check

The title-bar search is correctly functional: entering `mountains` and pressing Enter navigated to `#/search?q=mountains` and populated the Search input. It should not be treated as a defect. The only minor discoverability concern is that submission depends on Enter with no visible submit affordance.

## Native-only verification still required

These actions were intentionally not performed because this QA run was hosted in a browser and cannot safely validate Tauri-native result dialogs or mutate a real personal library:

- Add/remove watched folders; folder indexing and file watcher updates.
- Model download, cancellation, retry, and low-disk/network error handling.
- Photo editor writes, export, batch conversion/compression, backup/restore.
- Trash restore/permanent deletion/empty-trash flow.
- Duplicate grouping/dismissal, health cleanup, geocoding and face scan.
- Viewer, gallery, album and editor behavior with a populated library.

## Recommended finalization order

1. Make every async page recoverable; fix Viewer routing and destructive-flow feedback.
2. Finish the missing management workflows: People corrections, smart-album edit, Trash selection/paging, unified editor save.
3. Match the Home reference intentionally: hero, activity, horizontal rails, and designed onboarding.
4. Repair responsive Search and Places visual context; standardize skeleton/error/empty states.
5. Expand format support and enrich chat retrieval.
6. Run a populated-library native QA pass before building a release; exercise every destructive/native action on disposable fixtures and record screenshots against each design reference.
