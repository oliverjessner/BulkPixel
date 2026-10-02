# Supported Formats

BulkPixel uses the same conversion pipeline in the desktop app and CLI. The table below also shows which formats can trigger Watched Folder rules.

| Format      | File extensions  | Import | Export       | Watched Folders |
| ----------- | ---------------- | ------ | ------------ | --------------- |
| JPEG        | `.jpg`, `.jpeg`  | Yes    | Yes (`.jpg`) | Yes             |
| PNG         | `.png`           | Yes    | Yes          | Yes             |
| WebP        | `.webp`          | Yes    | Yes          | Yes             |
| AVIF        | `.avif`          | Yes    | Yes          | Yes             |
| SVG         | `.svg`           | Yes    | No           | Yes             |
| HEIC / HEIF | `.heic`, `.heif` | Yes    | No           | Yes             |
| TIFF        | `.tif`, `.tiff`  | Yes    | No           | Yes             |
| GIF         | `.gif`           | Yes    | No           | Yes             |
| JPEG XL     | `.jxl`           | Yes    | No           | Yes             |

## Format Notes

- JPEG, WebP, and AVIF exports use the selected quality setting.
- PNG exports use lossless compression. If the decoded source contains 16-bit channels, PNG preserves that channel depth, including after resizing.
- SVG files are rasterized during import at the requested width or height while preserving their aspect ratio.
- HEIC and HEIF use the native macOS decoder. BulkPixel applies image orientation, converts embedded color profiles such as Display P3 to sRGB, and produces SDR output. For HDR files, it uses the author-provided SDR-compatible base image so highlight detail is preserved instead of clipping the expanded gain map into an 8-bit export.
- TIFF imports the first page of a multipage file. Decoded 16-bit channels are preserved when exporting to PNG.
- GIF and JPEG XL import the first frame of an animated file and export a still image. Transparency is preserved in PNG, WebP, and AVIF exports; JPEG composites it onto white.
- JPEG XL supports both raw codestreams and container files, including 16-bit channels and alpha. Decoding uses [jxl-oxide](https://github.com/tirr-c/jxl-oxide), without requiring an installed native JPEG XL library.
- Watched Folders treat `.jpg` and `.jpeg` as JPEG, `.heic` and `.heif` as HEIC, and `.tif` and `.tiff` as TIFF. Extensions are case-insensitive.
