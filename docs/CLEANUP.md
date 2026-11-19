# Build artifact cleanup

Preview the generated files and their sizes without changing files:

```sh
npm run clean -- --dry-run
```

Remove them:

```sh
npm run clean
```

`npm run cleanup` is an alias with the same options. Unknown arguments are rejected.
Run cleanup when no development app, test or build is using the generated files.

Cleanup covers Rust build output (including debug/test builds), generated Node
sidecars, the Fetchary website-import runtime, its Node download cache, Tauri
generated schemas and coverage output. It only accepts the BulkPixel Git checkout
root, refuses tracked files and symlinked cleanup roots or ancestors, and validates
all destinations before the first deletion. Symlinks inside disposable build trees
are removed without following their targets.

## Releases and publishing

Both `sh scripts/publish.sh` and `npm run publish` use the same publish/cleanup lock.
Cleanup runs automatically only after the app build, signing, GitHub release and
Homebrew tap update have all succeeded. A failed publish retains its build output.
An interrupted process with a dead PID can release its stale lock on the next run;
unknown or incomplete lock contents require manual inspection.

BulkPixel stores app bundles and DMGs inside `src-tauri/target`, so cleanup preserves
the `bundle` directories within that tree. App bundles, the current version's DMG,
newer versions and unknown files are retained. Only older DMGs with BulkPixel's
release filename are eligible for removal, and only when the published GitHub
release has an uploaded asset with an exact matching SHA-256 digest. This also
applies to `--dry-run`. Missing `gh`, network errors, missing digests, drafts and
checksum mismatches retain the DMG. No GitHub releases are deleted.

Source files, icons, original test images, screenshots, installed npm dependencies,
copied UI assets, presets/statistics databases, backups and installed applications
are outside the allowlist. Cleanup does not use `git clean` or touch global Cargo,
npm or Homebrew caches. A custom external `CARGO_TARGET_DIR` is not cleaned.

`npm run dev` and `npm run build` prepare missing UI/website-import resources again;
deleting the download cache can require Internet access. Rust builds will compile
again after cleanup.

## Validation

The cleanup regression tests use temporary Git repositories and mocked GitHub
responses, without publishing or deleting the real project's build output:

```sh
node --test tests/cleanup.test.js
node --check scripts/cleanup.mjs
node --check scripts/publish.mjs
node --check scripts/publish-lock.mjs
sh -n scripts/publish.sh
```
