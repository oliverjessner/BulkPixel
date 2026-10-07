# 3.1.2

- moving to https://github.com/oliverjessner/oj-designsystem

# 3.1.1

- JPEG 2000 (`.jp2`) input support in the desktop app, CLI, Finder, and Watched Folders

# 3.0.1

- Allow Watched Folder rules to cascade outputs into other watched folders, with native-event deduplication, per-chain cycle detection, and an eight-rule depth limit.

# 3.0.0

- Resolve Dependabot security alerts by updating affected Rust and Tauri dependencies.
- Add JPEG and JPG support to Watched Folders, including migration of existing watcher configurations.
- Add native HEIC and HEIF import across the desktop app, CLI, Finder integration, and Watched Folders, with orientation handling, sRGB color management, and highlight-preserving SDR output for HDR source images.
- Preserve 16-bit channel depth when exporting PNG files, including resized images, instead of always reducing them to 8-bit RGBA.
- Document import, export, and Watched Folder compatibility in a central supported-formats matrix.
- the command --help shows the version
- screenshots are automated
- click on the image opens a modal window for infos like exif
- Rename the automatic-folder feature to Watched Folders and add required names to every rule.
- Track successful Watched Folder conversions in shared statistics.

## UX overhaul

from
![](/docs/ux_overhaul/convert_v2.1.1.webp)

to
![](/src/assets/mockups/bulkpixel.webp)

from
![](/docs/ux_overhaul/magic_directory_v2.1.1.webp)

to
![](/src/assets/mockups/watched_folders_directory.webp)

from
![](/docs/ux_overhaul/presets_v2.1.1.webp)

to
![](/src/assets/mockups/presets.webp)

# 2.1.1

- the statistics from the cli bulkpixel stats is now availabe via clicking the top left BulkPixel text
- track successful CLI conversion runs in the shared statistics
- "Show in Finder" button next to the output path
- Bump serde_with from 3.18.0 to 3.21.0

# 2.1.0

- Watched Folders for automatic preset-based conversion
- show Watched Folder activity in the app header
- migrate early Watched Folder database schemas without losing preset links
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
