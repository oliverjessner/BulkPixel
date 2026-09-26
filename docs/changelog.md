# 3.0.0

- Resolve Dependabot security alerts by updating affected Rust and Tauri dependencies.
- Add JPEG and JPG support to Magic Directories, including migration of existing watcher configurations.
- Add native HEIC and HEIF import across the desktop app, CLI, Finder integration, and Magic Directories, with orientation handling, sRGB color management, and highlight-preserving SDR output for HDR source images.
- Preserve 16-bit channel depth when exporting PNG files, including resized images, instead of always reducing them to 8-bit RGBA.
- Document import, export, and Magic Directory compatibility in a central supported-formats matrix.
- the command --help shows the version
- UX overhaul
- click on the image opens a modal window for infos like exif

# 2.1.1

- the statistics from the cli bulkpixel stats is now availabe via clicking the top left BulkPixel text
- track successful CLI conversion runs in the shared statistics
- "Show in Finder" button next to the output path
- Bump serde_with from 3.18.0 to 3.21.0

# 2.1.0

- Magic Directories for automatic preset-based conversion
- show Magic Directory activity in the app header
- migrate early Magic Directory database schemas without losing preset links
- SVG import support
- itworksbut fix

# 2.0.3

- bundle macOS AVIF decoder dependency in the app
- keep CLI, app and Cargo versions in sync before publishing

# 2.0.0

- vertical scroll for pictures
- clicking on the reference badge resets the height and width settings
- status now shows the sum of the savings
- presets, define your own preset and use it
- CLI, full docu in README.md
- update outdated deps

# 1.0.4

- open with feature
