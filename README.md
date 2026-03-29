# BulkPixel

BulkPixel is a local-first desktop utility for converting and resizing multiple images at once with a restrained dark interface inspired by VSCode. It supports drag and drop, native file picking, JPEG/PNG/WEBP export, optional width-or-height resizing with preserved aspect ratio, filename prefixes, output folder selection, collision-safe saves, and per-image plus total savings analysis after conversion.

## Stack

- Tauri v2
- Plain HTML
- Plain CSS
- Plain JavaScript
- Rust image processing with the `image` and `webp` crates

## Folder Structure

```text
BulkPixel/
├── logo.png
├── package.json
├── package-lock.json
├── src/
│   ├── formatters.js
│   ├── index.html
│   ├── logo.png
│   ├── main.js
│   ├── styles.css
│   └── ui.js
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── capabilities/
    │   └── default.json
    ├── icons/
    │   └── ...
    ├── src/
    │   ├── image_pipeline.rs
    │   ├── lib.rs
    │   ├── main.rs
    │   └── models.rs
    └── tauri.conf.json
```

## App Organization

- `src/index.html`
  Defines the desktop layout: header, dropzone, settings, previews, results, and action bar.

- `src/styles.css`
  Holds the full dark UI system, section layout, controls, card styling, empty states, and responsive behavior.

- `src/main.js`
  Owns application state, event wiring, native file picker integration, drag-and-drop handling, validation, and the conversion flow.

- `src/ui.js`
  Renders preview cards, results rows, summary cards, and control states.

- `src/formatters.js`
  Contains formatting and small helper utilities for bytes, percentages, dimensions, and safe text output.

- `src-tauri/src/image_pipeline.rs`
  Performs image probing, preview thumbnail generation, resize calculations, encoding, safe filename generation, and savings calculations.

- `src-tauri/src/models.rs`
  Defines the request and response models passed between the frontend and Rust backend.

- `src-tauri/src/lib.rs`
  Registers Tauri commands and initializes the dialog plugin.

## Features

- Drag and drop images directly into the desktop window
- Click the upload area to open the native file picker
- Supports `jpg`, `jpeg`, `png`, and `webp` inputs
- Export all selected images into a single chosen format
- Resize by width or height only, with automatic aspect-ratio preservation
- Quality slider for JPEG and WEBP
- Honest lossless PNG handling with disabled quality control
- Optional filename prefix
- Default output folder set to the system Downloads directory
- Safe collision handling with `_1`, `_2`, and so on
- Per-image and total space-saved reporting
- Clear success, partial-success, and error feedback

## Setup

Prerequisites:

- Node.js 20+ recommended
- Rust toolchain
- Tauri desktop prerequisites for your platform

Install dependencies:

```bash
npm install
```

## Run in Development

```bash
npm run dev
```

This starts the Tauri desktop app directly against the static frontend in `src/`.

## Build

Debug bundle:

```bash
npm run build -- --debug
```

Release bundle:

```bash
npm run build
```

On macOS, Tauri will output the app bundle and disk image under:

```text
src-tauri/target/{debug|release}/bundle/
```

## Validation Performed

The current implementation was verified with:

```bash
cargo check
cargo test
npm run build -- --debug
```

## Notes

- JPEG export flattens transparent pixels against white so the output remains valid.
- PNG export remains lossless, so the quality slider is disabled when PNG is selected.
- Savings analysis is honest: if a converted image is larger, BulkPixel reports it as larger instead of pretending there were savings.
