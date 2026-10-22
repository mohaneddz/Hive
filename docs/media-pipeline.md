# Media pipeline inventory

This document records every media ingestion and loading surface. A change to a format or loader must be checked against every row, not just the Gallery. The table records current behavior after the October 2026 loading pass and remaining limitations.

| Surface | Code | Current behavior / remaining limitation |
|---|---|---|
| Folder scan | `commands/media.rs`, `indexing.rs` | Recursively collects paths. Size plus nanosecond mtime skips unchanged files on repeat scans; changed files are hashed. Native image decode is shared with thumbnail generation. |
| Live watcher | `watcher.rs` | Forces a hash check on create/modify events even when a tool preserves mtime. Consecutive events can still repeat work. |
| Thumbnails | `thumbnails.rs` | Produces 240 and 800 pixel JPEGs named with the source hash, so edits invalidate cached URLs. HEIC/RAW and videos depend on system ffmpeg. Evicted thumbnails rebuild when visible; unsupported files show an error tile instead of loading a full original. |
| Grid loading | `MediaThumb.tsx`, `lib/tauri.ts`, `commands/media.rs` | Near-viewport cards request scoped thumbnail asset URLs. The WebView handles file loading, decoding and cache behavior without an IPC byte copy. |
| Viewer | `ViewerPage.tsx`, `commands/media.rs`, `thumbnails.rs` | Streams indexed originals via the asset protocol; videos use native controls. Unsupported image formats get a cached 4096-pixel JPEG preview where ffmpeg or the Rust decoder can read them. |
| Editor preview and save | `EditorPage.tsx`, `commands/editor.rs` | Uses an 800 pixel preview, then decodes the full original for saving. The backend image decoder cannot always open formats that thumbnail ffmpeg can read. |
| AI image passes | `ai/clip.rs`, `ai/ocr.rs`, `ai/face.rs`, `ai/nsfw.rs`, `ai/captioning.rs`, `commands/faces.rs` | Each pass decodes source images independently. Model downloads and inference are separate from the initial index job. |
| Quality and duplicates | `commands/quality.rs`, `duplicates.rs`, `commands/best_photo.rs` | Sharpness and duplicate scoring load original image data; their coverage differs from the thumbnail fallback formats. |
| Batch tools | `commands/batch.rs` | Image conversion/compression decodes original files; video compression uses ffmpeg. |
| Cache | `commands/preferences.rs` | File cache limit counts thumbnail files and removes oldest files. The WebView owns in-flight asset requests; large AI passes remain independent. |

Current recognized image extensions: JPEG, PNG, GIF, WebP, BMP, TIFF, AVIF, HEIC/HEIF, and RAW (CR2, CR3, NEF, ARW, DNG, ORF, RW2, RAF). Current recognized video extensions: MP4, MOV, AVI, MKV, WebM. Recognition does not guarantee that the platform can decode, view, edit, or play the format; those are separate capabilities.
