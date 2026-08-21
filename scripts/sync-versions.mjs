import fs from 'node:fs';

const packageJsonPath = 'package.json';
const tauriConfigPath = 'src-tauri/tauri.conf.json';
const cargoManifestPath = 'src-tauri/Cargo.toml';
const cargoLockPath = 'src-tauri/Cargo.lock';

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
const version = packageJson.version;

if (typeof version !== 'string' || version.trim() === '') {
    throw new Error(`${packageJsonPath} does not contain a valid version`);
}

updateJsonVersion(tauriConfigPath, version);
updateTextVersion(
    cargoManifestPath,
    /(\[package\][\s\S]*?^version\s*=\s*")[^"]*(")/m,
    version,
);
updateTextVersion(
    cargoLockPath,
    /(\[\[package\]\]\r?\nname = "bulkpixel"\r?\nversion = ")[^"]*(")/,
    version,
);

function updateJsonVersion(path, nextVersion) {
    const source = fs.readFileSync(path, 'utf8');
    const json = JSON.parse(source);

    if (json.version === nextVersion) {
        return;
    }

    updateTextVersion(path, /(\"version\"\s*:\s*\")[^"]*(")/, nextVersion, source);
}

function updateTextVersion(path, pattern, nextVersion, existingSource) {
    const source = existingSource ?? fs.readFileSync(path, 'utf8');
    let matched = false;
    const updated = source.replace(pattern, (_match, prefix, suffix) => {
        matched = true;
        return `${prefix}${nextVersion}${suffix}`;
    });

    if (!matched) {
        throw new Error(`${path} does not contain a writable version field`);
    }

    if (updated !== source) {
        fs.writeFileSync(path, updated);
    }
}
