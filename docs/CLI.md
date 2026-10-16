# BulkPixel CLI

BulkPixel ships with a CLI. In the DMG, it is included as a `bulkpixel` symlink next to `BulkPixel.app`.
The symlink points to the app binary. Running the binary without arguments opens the desktop app; running CLI commands executes BulkPixel in the terminal.

The macOS Dock menu provides `Convert`, `Presets`, and `Watched Folders` shortcuts for navigating the running desktop app. The CLI exposes the same workflows through `bulkpixel convert`, `bulkpixel presets`, and `bulkpixel watched-folders`; these commands operate in the terminal instead of switching desktop views.

## Install

Install BulkPixel and the CLI with Homebrew:

```sh
brew tap oliverjessner/tap
brew install --cask bulkpixel
```

## Help and Version

```sh
bulkpixel --help
bulkpixel --version
```

## Convert Images

See [Supported Formats](FORMATS.md) for the complete import/export matrix, Watched Folder compatibility, and format-specific behavior.

Inputs include JPEG, PNG, WebP, AVIF, SVG, HEIC/HEIF, TIFF (`.tif`/`.tiff`), GIF, JPEG XL (`.jxl`), JPEG 2000 (`.jp2`), and ICO (`.ico`). Animated GIF and JPEG XL files convert their first frame; multipage TIFF files convert their first page. ICO supports PNG- and BMP-backed images and selects the highest color depth, then the largest dimensions at that depth, producing one image per file. The desktop app uses the same behavior. TIFF and JPEG XL retain decoded 16-bit channels when exporting to PNG. JPEG 2000 uses the native macOS decoder and produces 8-bit sRGB output.

### Import from URLs

Use `--url` for one or more image URLs. Quote URLs so shell characters in signed query strings are preserved. HTTP/HTTPS URLs passed to `--input` are also recognized. Local files and URLs can be mixed, and URLs work with `--preset` as well as direct conversion settings.

```sh
bulkpixel convert \
  --input ./local.png \
  --url 'https://example.com/photo.jpg' 'https://cdn.example.com/asset?id=42' \
  --output-dir ./exports --format webp --width 1200 --quality 90 \
  --prefix web_ --url-max-mb 50

bulkpixel convert --preset 'Website WEBP' --url 'https://example.com/original'
```

The CLI and desktop app share the native downloader and existing image pipeline. Downloads retain their original bytes; format detection checks content rather than trusting an extension or Content-Type alone. Filenames prefer Content-Disposition, then the final URL path, then a generated name. The detected extension is applied, unsafe characters are removed, and duplicate names receive numeric suffixes. Query parameters are never used as filenames.

The default download limit is 50 MB per image; `--url-max-mb` accepts whole numbers from 1–200. Batches support up to 100 URLs, processed sequentially. Each image has a 30-second overall timeout and at most five redirects; HTTPS-to-HTTP redirects are rejected. Every destination is checked, DNS answers are validated and pinned to the connection, and loopback, private, link-local, multicast, and reserved network addresses are blocked. System proxies, cookies, and URL credentials are not used. Authenticated images that require browser cookies are not supported. Remote SVGs may contain inline vector content and embedded raster images, but cannot reference external resources or embedded SVGs; styles with escaped resource references are rejected.

Press Ctrl+C during URL import to cancel pending downloads. Failed or duplicate URL imports are reported individually; the remaining images continue. Successfully downloaded and local images are converted, and any failed/canceled import makes the command exit with status 1 even if other conversions succeed. `--silent` suppresses import messages as well as the conversion report. Rerun a failed URL to retry it; the desktop dialog provides a **Retry Failed Images** button.

Temporary originals and partial downloads are isolated in a private cache session and removed when the CLI finishes. Abandoned sessions are cleaned up on the next URL import without deleting active sessions from another app/CLI process. Conversion collisions, overwrite behavior, presets, output folders, and shared statistics follow the existing rules below. Downloads themselves do not increment conversion counters. The desktop app retains successful imports in its queue after a partial failure or cancellation; the CLI exports successful downloads and then cleans up its temporary originals.

Required:

- `--input` and/or `--url`
- `--output-dir`

Optional:

- `--width`
- `--height`
- `--format` default: `webp`
- `--quality` default: `100`
- `--prefix`
- `--postfix`
- `--overwrite`
- `--silent`

BulkPixel does not overwrite files by default. If an output file already exists, the affected conversion fails with an error.
Use `--overwrite` to allow the CLI to replace existing output files.

Inputs with the same filename stem, such as `test.gif` and `test.jp2`, target the same output filename. Convert these to separate output folders or use different prefixes/postfixes in separate commands. `--overwrite` does not allow two inputs in one batch to write the same output path. The desktop app instead adds numeric suffixes automatically.

Use either `--width` or `--height`. The other value is calculated automatically from the image aspect ratio.
Use either `--prefix` or `--postfix`.

```sh
bulkpixel convert \
  --input ./image.png \
  --output-dir ./exports \
  --width 1200 \
  --format png \
  --quality 100
```

Pass multiple images as a list after `--input`:

```sh
bulkpixel convert \
  --input ./image-1.png ./image-2.jpg \
  --output-dir ./exports \
  --width 1200 \
  --postfix "_1200" \
  --format webp \
  --quality 90
```

Mix the new input formats in the same batch:

```sh
bulkpixel convert \
  --input ./scan.tiff ./animation.gif ./photo.jxl \
  --output-dir ./exports \
  --format png
```

After a successful conversion, BulkPixel prints a summary:

```txt
BulkPixel Conversion Complete
-----------------------------
Images: 3/3 converted
Format: PNG → WEBP
Input: 8.4 MB
Output: 6.1 MB
Saved: 2.3 MB
Output Folder: ./exports
```

Use `--silent` to suppress this summary.

## List Presets

```sh
bulkpixel presets list
```

Example:

```txt
BulkPixel Presets (3)
-----------------
Name: Blog Header
Export Format: WEBP
Width: 1200
Height: Auto
Quality: 100%
Postfix: _1200
Output Folder: /Users/oli/exports
```

`Prefix` or `Postfix` is only shown when a value is set.

## Convert With a Preset

When `--preset` is set, conversion settings come from the preset.
Only `--input` is relevant; other conversion flags are ignored.

```sh
bulkpixel convert \
  --preset "Blog Header" \
  --input ./image-1.png ./image-2.png
```

Feedback:

```txt
BulkPixel Conversion Complete
-----------------------------
Preset: Blog Header
Images: 3/3 converted
Format: PNG → WEBP
Input: 8.4 MB
Output: 6.1 MB
Saved: 2.3 MB
Output Folder: ./exports
```

## Multiple Presets

```sh
bulkpixel convert \
  --preset "Blog Header" "Open Graph" \
  --input ./image-1.png ./image-2.png
```

Each preset creates its own output for every input image.
When multiple presets are used, each preset must have a unique non-empty prefix or postfix.
Otherwise, the CLI stops with a collision error.

Feedback:

```txt
BulkPixel Conversion Complete
-----------------------------
Presets: Blog Header, Open Graph

Blog Header
Images: 3/3 converted
Format: PNG → WEBP
Saved: 2.3 MB

Open Graph
Images: 3/3 converted
Format: PNG → WEBP
Saved: 3.1 MB

Total
Images: 6/6 converted
Input: 16.8 MB
Output: 11.4 MB
Saved: 5.4 MB
```

## Create a Preset

Required:

- `--name`
- `--output-dir`
- `--format`

Optional:

- `--width`
- `--height`
- `--prefix`
- `--postfix`
- `--quality` default: `100`

```sh
bulkpixel presets create \
  --name "Blog Header" \
  --width 1200 \
  --output-dir ./exports \
  --postfix "_1200" \
  --format webp \
  --quality 100
```

## Update a Preset

```sh
bulkpixel presets update \
  --name "Blog Header" \
  --width 1600 \
  --quality 90
```

Only the passed values are changed.

## Delete a Preset

```sh
bulkpixel presets delete \
  --name "Blog Header"
```

## Watched Folders

Watched Folder rules use the same SQLite database as the desktop app. Every rule has a required name. The desktop app performs the actual watching while it is running, and rules created or changed through the CLI are loaded the next time the desktop app starts.

Supported watched formats are `jpeg` (or its `jpg` alias), `png`, `webp`, `avif`, `svg`, `heic` (or its `heif` alias), `tiff` (or its `tif` alias), `gif`, `jxl`, `jp2`, and `ico`. Both `.jpg` and `.jpeg` files match JPEG; both `.heic` and `.heif` files match HEIC; both `.tif` and `.tiff` files match TIFF. Files in subfolders are not watched. Multiple selected presets must have unique non-empty prefixes or postfixes.

Overwrite is disabled by default for each Watched Folder. When an output filename already exists, the watcher keeps it and adds a suffix to the new file, such as `1_1.png`. Enable **Overwrite existing output files** in the desktop rule or pass `--overwrite` when creating or updating a CLI rule to replace the existing output instead. The setting applies to every preset assigned to that rule. `watched-folders list` shows each rule's `Overwrite` value.

Watched Folder rules can be chained: when one rule writes a supported output directly into another enabled Watched Folder, the matching downstream rule runs automatically. BulkPixel deduplicates the native file event, stops a chain before it repeats a rule, and limits each chain to eight rules.

Create a rule:

```sh
bulkpixel watched-folders create \
  --name "Incoming Website Images" \
  --path ./incoming-images \
  --formats jpg png webp \
  --presets "Website WEBP" "Archive AVIF"
```

To create a rule that replaces existing output files, add `--overwrite`:

```sh
bulkpixel watched-folders create \
  --name "Replace Website Images" \
  --path ./incoming-images \
  --formats png \
  --presets "Website WEBP" \
  --overwrite
```

List rules and their IDs:

```sh
bulkpixel watched-folders list
```

Update selected settings. Omitted settings keep their existing values:

```sh
bulkpixel watched-folders update \
  --id 1 \
  --name "Paused Website Imports" \
  --formats svg png \
  --disabled
```

Enable or disable overwriting on an existing rule:

```sh
bulkpixel watched-folders update --id 1 --overwrite
bulkpixel watched-folders update --id 1 --no-overwrite
```

Omitting both overwrite flags preserves the saved setting. Passing both flags is an error.

Use `--enabled` to reactivate a saved rule. Delete a rule with:

```sh
bulkpixel watched-folders delete --id 1
```

## Statistics

```sh
bulkpixel stats
```

Output:

```txt
BulkPixel Statistics
--------------------
Conversions
Total: 134
WEBP: 109
PNG: 12
AVIF: 1
JPEG: 12

Usage
CLI Uses: 18
UI Uses: 74
Watched Folder Conversions: 42

Storage
Input: 320.4 MB
Output: 100.2 MB
Saved: 220.2 MB

Performance
Processing Time: 3min 30sec

Timeline
First Conversion: 03.07.2026
Last Conversion: 05.07.2026
```

`CLI Uses` counts successful `bulkpixel convert` command runs. A single command using
multiple presets counts once, and CLI management commands such as `stats` or `presets list`
do not increase it.

`UI Uses` is derived as `Total - CLI Uses - Watched Folder Conversions`, with a minimum
of zero for inconsistent historical counters. It is calculated from existing statistics
in both the app and CLI; no separate UI counter is incremented or reset on upgrade.
`Total` and Watched Folder conversions count successful outputs, while `CLI Uses`
counts command runs. Consequently, a CLI batch producing multiple outputs also increases
this derived remainder; it is not an exact count of desktop exports or desktop batches.

`Watched Folder Conversions` counts successful outputs created automatically by Watched Folder rules.
