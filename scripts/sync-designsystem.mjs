import { cp, mkdir, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Tauri serves a static frontend. Copy the installed package, including its
// relative font/icon assets and licenses, without adding a frontend bundler.
const require = createRequire(import.meta.url);
const packageDirectory = path.dirname(require.resolve("oj-designsystem/package.json"));
const destination = fileURLToPath(new URL("../src/vendor/oj-designsystem/", import.meta.url));

await rm(destination, { recursive: true, force: true });
await mkdir(destination, { recursive: true });
await cp(path.join(packageDirectory, "dist"), destination, { recursive: true });
for (const filename of ["LICENSE", "THIRD-PARTY-NOTICES.md"]) {
    await cp(path.join(packageDirectory, filename), path.join(destination, filename));
}
