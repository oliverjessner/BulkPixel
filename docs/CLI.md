# BulkPixel CLI

BulkPixel ships with a CLI. In the DMG, it is included as a `bulkpixel` symlink next to `BulkPixel.app`.
The symlink points to the app binary. Running the binary without arguments opens the desktop app; running CLI commands executes BulkPixel in the terminal.

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

Inputs include JPEG, PNG, WebP, AVIF, SVG, HEIC/HEIF, TIFF (`.tif`/`.tiff`), GIF, JPEG XL (`.jxl`), and JPEG 2000 (`.jp2`). Animated GIF and JPEG XL files convert their first frame; multipage TIFF files convert their first page. The desktop app uses the same behavior. TIFF and JPEG XL retain decoded 16-bit channels when exporting to PNG. JPEG 2000 uses the native macOS decoder and produces 8-bit sRGB output.

Required:

- `--input`
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

Supported watched formats are `jpeg` (or its `jpg` alias), `png`, `webp`, `avif`, `svg`, `heic` (or its `heif` alias), `tiff` (or its `tif` alias), `gif`, `jxl`, and `jp2`. Both `.jpg` and `.jpeg` files match JPEG; both `.heic` and `.heif` files match HEIC; both `.tif` and `.tiff` files match TIFF. Files in subfolders are not watched. Multiple selected presets must have unique non-empty prefixes or postfixes.

Watched Folder rules can be chained: when one rule writes a supported output directly into another enabled Watched Folder, the matching downstream rule runs automatically. BulkPixel deduplicates the native file event, stops a chain before it repeats a rule, and limits each chain to eight rules.

Create a rule:

```sh
bulkpixel watched-folders create \
  --name "Incoming Website Images" \
  --path ./incoming-images \
  --formats jpg png webp \
  --presets "Website WEBP" "Archive AVIF"
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
UI Uses: 24
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

`UI Uses` counts desktop conversion batches with at least one successful output.
Each batch counts once, including partial successes. Opening the app, viewing statistics,
fully failed batches, CLI conversions, and Watched Folder conversions do not increase it.
The counter starts at zero when upgrading; existing conversion totals and other usage counters are preserved.

`Watched Folder Conversions` counts successful outputs created automatically by Watched Folder rules.
