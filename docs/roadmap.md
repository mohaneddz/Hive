# Hive roadmap

## Core workflows

- [x] Return from the viewer to the previous Hive route, with a Gallery fallback for direct opens.
- [x] Debounce search, synchronize titlebar and page queries, and make Gallery Chat accessible on narrow windows.
- [x] Support native titlebar dragging and maximize, model readiness and error feedback, and loading skeletons.
- [x] Merge people and reassign faces, edit smart album rules and covers, and remove album items in bulk.
- [x] Save editor adjustments and details together, with undo, redo, and resize handles for cropping.
- [x] Page through Trash and restore or permanently delete a selection.

## Media and discovery

- [x] Share one image decoder across indexing, previews, editing, batch conversion, and AI. Bundle HEIC and camera RAW decoders and support AVIF processing through FFmpeg.
- [x] Keep capture metadata when saving edited images and clear stale EXIF when a file is replaced outside Hive.
- [x] Use filenames, OCR, captions, tags, people, and cached places to retrieve local chat context, supplemented by CLIP when available.
- [x] Show an offline Places map with optional online street detail. The bundled land data is [Natural Earth 1:110m, v5.1.2](https://www.naturalearthdata.com/downloads/110m-physical-vectors/).
- [x] Use horizontal Home rails, route-level bundle splitting, and idle AI session unloading.

## First release

- [ ] Verify the Windows installer on a clean machine and test the upgrade path against an existing `0.1.0` library.
- [ ] Sign and tag the installer after the clean-machine and upgrade checks pass.

Run `pnpm install --frozen-lockfile`, `pnpm build`, and
`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` before packaging. Build the Windows
installer with `pnpm tauri build --bundles nsis`.
