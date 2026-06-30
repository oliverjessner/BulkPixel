# BulkPixel CLI

BulkPixel ships with a CLI. In the DMG, it is included as a `bulkpixel` symlink next to `BulkPixel.app`.
The symlink points to the app binary. Running the binary without arguments opens the desktop app; running CLI commands executes BulkPixel in the terminal.

## Help and Version

```sh
bulkpixel --help
bulkpixel --version
```

## Convert Images

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
