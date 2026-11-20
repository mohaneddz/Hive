# Media pipeline inventory

| Surface | Behavior |
|---|---|
| Folder scan | Walks folders without retaining every file path. A size and nanosecond mtime check skips unchanged files. Changed files are hashed and decoded once for dimensions, perceptual hash, and thumbnails. A rescan retries previously indexed AVIF, HEIC, and RAW items with missing dimensions. |
| Live watcher | Uses the same pipeline. A create or modify notification forces a hash check, including when another editor preserves the file's mtime. |
| Thumbnails | Generates 240 and 800 pixel JPEGs, keyed by the source hash. Missing cached thumbnails regenerate when viewed. Video thumbnails use system FFmpeg. |
| Grid loading | Requests scoped thumbnail URLs near the viewport. The WebView reads and caches the assets. |
| Viewer | Streams standard image and video originals via the asset protocol. TIFF, HEIC, and RAW use cached JPEG previews up to 4096 pixels. The operating system WebView supplies video and AVIF codecs. |
| Editor | Previews the image and saves changes from a full-resolution decode. HEIC, HEIF, and camera RAW edits save as JPEG copies. Hive retains the source's indexed capture date, camera fields, and GPS on edits. |
| AI and quality | CLIP, OCR, face recognition, captions, sensitive-content classification, and sharpness use the same image decoder as indexing. Each pass still decodes independently. Duplicate detection uses the perceptual hash stored at indexing time. |
| Batch tools | Image conversion and compression use the shared decoder. Video compression uses system FFmpeg. |
| Cache | The cache limit counts thumbnail and preview files. The WebView manages its own asset requests. |

Hive recognizes JPEG, PNG, GIF, WebP, BMP, TIFF, AVIF, HEIC/HEIF, CR2, CR3, NEF, ARW, DNG,
ORF, RW2, and RAF images. It recognizes MP4, MOV, AVI, MKV, and WebM videos. Decoding still depends
on the codec, camera model, and file contents.

Built-in HEIC and RAW decoding use Heifer and FlashRaw. AVIF decoding in the Rust tools uses
system FFmpeg with AV1 support. FFmpeg also provides a fallback for supported HEIC and RAW variants.
The video viewer uses the operating system WebView's codec support.
