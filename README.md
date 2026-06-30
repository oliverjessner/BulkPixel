# BulkPixel 🖼️

![BulkPixel convert view](/src/assets/mockups/bulkpixel_1920.webp)

**Convert more. Click less.**

BulkPixel is a local-first desktop app for fast batch image processing (Mac Only).  
Drop in multiple images, choose an output format, resize if needed, and export everything in one go.

Built for people who do not want a bloated image editor just to prepare assets for the web, blogs, apps, client work, or side projects.

## Why BulkPixel

Most image tools are either too heavy, too online, or too annoying for simple repetitive work.

BulkPixel focuses on the practical stuff:

- batch convert images in seconds
- resize without breaking aspect ratios
- export to modern web-friendly formats
- keep filenames predictable
- avoid accidental overwrites
- see honest file savings after conversion

It is local-first, fast, and intentionally restrained.

## Features

### Available now

- Drag and drop images into the app
- Native file picker support
- Batch conversion for multiple images at once
- Input support for `jpg`, `jpeg`, `png`, `avif` and `webp`
- Export to `jpg`, `png`, `avif` or `webp`
- Resize by width or height
- Automatic aspect-ratio preservation
- Quality control for JPEG, WEBP and AVIF
- Honest PNG handling without fake "compression" promises
- Optional filename prefix or postfix (toggle between modes)
- Output folder selection
- Presets for saving reusable conversion settings
- Terminal CLI for scripted conversion, preset management, and statistics
- Collision-safe saving with `_1`, `_2`, and so on
- Per-image and total savings analysis
- Clear success, partial-success, and error feedback

## Presets

Presets let you save complete conversion setups and reuse them later.  
A preset stores the export format, resize setting, quality, filename prefix or postfix, and output folder.

Use `Convert` for normal batch conversion. Use `Presets` to create, edit, duplicate, or delete saved setups. In the conversion settings, the preset dropdown lets you switch between `Default`, saved presets, and `Custom` when settings are changed manually.

![BulkPixel presets view](/src/assets/mockups/presets_1920.webp)

## CLI

BulkPixel also ships a `bulkpixel` command for terminal workflows.

Install BulkPixel and the CLI with Homebrew:

```sh
brew tap oliverjessner/tap
brew install --cask bulkpixel
```

See [docs/CLI.md](docs/CLI.md) for commands, flags, preset usage, overwrite rules, and statistics output.

## macOS Open With test cases

1. BulkPixel is closed:
    - Select a PNG in Finder
    - Right-click -> Open With -> BulkPixel
    - Expected: BulkPixel starts and the image appears in the queue

2. BulkPixel is already running:
    - Select multiple JPG, PNG, or WEBP files in Finder
    - Right-click -> Open With -> BulkPixel
    - Expected: The existing app receives the files and adds them to the queue

3. Unsupported file type:
    - Open a TXT file or another unsupported file with BulkPixel
    - Expected: The file is ignored or reported as unsupported without starting a conversion

4. Duplicate file:
    - Open the same file twice with BulkPixel
    - Expected: The app does not crash and the queue remains valid. BulkPixel currently keeps one queue entry per path while it is already loaded.

## Who it is for

BulkPixel is useful for:

- bloggers and journalists preparing web images
- indie developers shipping assets quickly
- designers exporting lighter previews
- creators cleaning up folders of screenshots or thumbnails
- anyone who wants a focused desktop utility instead of a full editor

## License

MIT
