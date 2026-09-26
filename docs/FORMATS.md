# Supported Formats

BulkPixel uses the same conversion pipeline in the desktop app and CLI. The table below also shows which formats can trigger Magic Directory rules.

| Format | File extensions | Import | Export | Magic Directories |
| --- | --- | --- | --- | --- |
| JPEG | `.jpg`, `.jpeg` | Yes | Yes (`.jpg`) | Yes |
| PNG | `.png` | Yes | Yes | Yes |
| WebP | `.webp` | Yes | Yes | Yes |
| AVIF | `.avif` | Yes | Yes | Yes |
| SVG | `.svg` | Yes | No | Yes |
| HEIC / HEIF | `.heic`, `.heif` | Yes | No | Yes |

## Format Notes

- JPEG, WebP, and AVIF exports use the selected quality setting.
- PNG exports use lossless compression. If the decoded source contains 16-bit channels, PNG preserves that channel depth, including after resizing.
- SVG files are rasterized during import at the requested width or height while preserving their aspect ratio.
- HEIC and HEIF use the native macOS decoder. BulkPixel applies image orientation, converts embedded color profiles such as Display P3 to sRGB, and produces SDR output. HDR images are tone-mapped on macOS 15 and later; older macOS versions use the file's SDR-compatible representation.
- Magic Directories treat `.jpg` and `.jpeg` as JPEG, and `.heic` and `.heif` as HEIC.

