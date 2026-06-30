# AGENTS.md

## Project Overview

BulkPixel is a local-first macOS batch image converter built with Tauri.
The desktop UI and terminal CLI are two entry points into the same product and should stay behaviorally aligned.

Core capabilities:

- Drag and drop or select images
- Batch convert `jpg`, `jpeg`, `png`, `webp`, and `avif`
- Export to `jpeg`, `png`, `webp`, and `avif`
- Resize by width or height while preserving aspect ratio
- Control quality for JPEG, WEBP, and AVIF
- Add an optional filename prefix or postfix
- Select an output folder
- Save, edit, duplicate, delete, list, and apply presets
- Track conversion statistics in SQLite

## Mandatory UI and CLI Parity

Every user-facing conversion feature available in the desktop UI must also be represented in the CLI.

When adding, changing, or removing a UI feature, update the CLI in the same change unless there is a documented reason not to.
When adding, changing, or removing a CLI feature, check whether the desktop UI needs the same behavior.

At minimum, keep parity for:

- Export format
- Width and height resize settings
- Quality
- Prefix or postfix filename handling
- Output folder
- Preset creation, update, duplication, deletion, listing, and application
- Batch conversion behavior
- Collision and overwrite behavior
- Statistics updates
- User-facing success, partial-success, and error feedback

If a feature cannot be represented exactly in both surfaces, document the difference in `docs/CLI.md` and keep the underlying Rust behavior shared where possible.

## Architecture Notes

- Shared conversion logic lives in `src-tauri/src/image_pipeline.rs`.
- Shared data models live in `src-tauri/src/models.rs`.
- Preset and statistics persistence lives in `src-tauri/src/presets.rs`.
- CLI behavior lives in `src-tauri/src/cli.rs`.
- Tauri command wiring lives in `src-tauri/src/lib.rs`.
- The process entry point is `src-tauri/src/main.rs`.

The app runs the GUI when launched without CLI arguments.
When launched with CLI arguments, the same binary dispatches to the CLI path.

Do not duplicate conversion logic between UI and CLI.
Prefer extending shared Rust request/response models and the existing pipeline.

## SQLite

BulkPixel stores presets and statistics in the app data directory:

- macOS app identifier: `com.oli.bulkpixel`
- database file: `presets.sqlite3`

Presets and statistics must be shared between the desktop app and CLI.
Do not create a second CLI-only database.

Statistics are a single-row table.
The generated `amount` field must not be updated directly.
Update only the concrete format counters and byte/time fields.

## CLI Commands

Document CLI behavior in `docs/CLI.md`.

Important commands:

- `bulkpixel convert`
- `bulkpixel convert --preset ...`
- `bulkpixel presets list`
- `bulkpixel presets create`
- `bulkpixel presets update`
- `bulkpixel presets delete`
- `bulkpixel stats`
- `bulkpixel --help`
- `bulkpixel --version`

By default, the CLI must not overwrite output files.
`--overwrite` is the explicit opt-in.

When multiple presets are used in one conversion, each preset must have a unique non-empty prefix or postfix, otherwise the CLI must fail with a collision error.

## macOS Bundling

AVIF input support depends on `libdav1d.7.dylib`.
Release builds must bundle Homebrew dylibs into:

```txt
BulkPixel.app/Contents/Frameworks/
```

The app binary must reference bundled dylibs with:

```txt
@executable_path/../Frameworks/...
```

Do not ship a build that references `/opt/homebrew/...` or `/usr/local/...` dylibs directly.

Use:

```sh
node scripts/bundle-macos-dylibs.mjs src-tauri/target/release/bundle/macos/BulkPixel.app
```

`scripts/publish.sh` already runs this before codesigning.
Codesign must happen after dylib path rewriting.

Verify with:

```sh
otool -L src-tauri/target/release/bundle/macos/BulkPixel.app/Contents/MacOS/bulkpixel
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/BulkPixel.app
```

## Versioning

Keep these versions synchronized:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`

`bulkpixel --version` comes from the Cargo package version.
Do not publish when these versions differ.
`scripts/publish.sh` checks this before building.

## Release and Homebrew

BulkPixel is distributed through GitHub Releases and a Homebrew Cask.

Release script:

```sh
sh scripts/publish.sh
```

The script:

- Builds the Tauri app
- Bundles macOS dylibs
- Codesigns the app
- Creates the DMG
- Creates the GitHub release
- Updates `Casks/bulkpixel.rb` in the Homebrew tap
- Commits and pushes the tap update

Default Homebrew tap path:

```txt
../homebrew-tap
```

Override:

```sh
HOMEBREW_TAP_DIR=/path/to/homebrew-tap sh scripts/publish.sh
```

Users install with:

```sh
brew tap oliverjessner/tap
brew install --cask bulkpixel
```

## Validation Checklist

Before finishing changes that affect conversion, presets, CLI, release, or packaging, run the relevant checks:

```sh
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
sh -n scripts/publish.sh
node --check scripts/bundle-macos-dylibs.mjs
git diff --check
```

For release/package changes, also verify:

```sh
npm run tauri -- build --bundles app --no-sign
node scripts/bundle-macos-dylibs.mjs src-tauri/target/release/bundle/macos/BulkPixel.app
codesign --force --deep --sign - src-tauri/target/release/bundle/macos/BulkPixel.app
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/BulkPixel.app
otool -L src-tauri/target/release/bundle/macos/BulkPixel.app/Contents/MacOS/bulkpixel
src-tauri/target/release/bundle/macos/BulkPixel.app/Contents/MacOS/bulkpixel --version
```

For CLI parity, test at least one real conversion path when behavior changes.
