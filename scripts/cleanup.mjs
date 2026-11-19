import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, lstatSync, openSync, readFileSync, readdirSync, readSync, realpathSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { acquirePublishLock } from './publish-lock.mjs';

export const root = fileURLToPath(new URL('../', import.meta.url));
export const cleanupPaths = Object.freeze([
    'src-tauri/target',
    'src-tauri/binaries',
    'src-tauri/resources/url-import',
    'src-tauri/gen',
    '.url-import-cache',
    'coverage',
]);

export function parseCleanupArgs(args) {
    const options = { dryRun: false, help: false };
    for (const arg of args) {
        if (arg === '--dry-run') options.dryRun = true;
        else if (arg === '--help' || arg === '-h') options.help = true;
        else throw new Error(`Unknown cleanup option: ${arg}`);
    }
    return options;
}

export function projectRoot(directory) {
    const absolute = realpathSync(directory);
    const metadata = JSON.parse(readFileSync(path.join(absolute, 'package.json'), 'utf8'));
    const cargo = readFileSync(path.join(absolute, 'src-tauri/Cargo.toml'), 'utf8');
    if (metadata.name !== 'bulkpixel' || !/^name\s*=\s*"bulkpixel"\s*$/m.test(cargo)) {
        throw new Error('Cleanup requires the BulkPixel project root.');
    }
    const checkout = execFileSync('git', ['rev-parse', '--show-toplevel'], { cwd: absolute, encoding: 'utf8' }).trim();
    if (realpathSync(checkout) !== absolute) throw new Error('Cleanup requires the root of the BulkPixel checkout.');
    return absolute;
}

function safeTarget(directory, relative) {
    let current = directory;
    const parts = relative.split('/');
    for (const [index, part] of parts.entries()) {
        current = path.join(current, part);
        let stat;
        try { stat = lstatSync(current); }
        catch (error) { if (error.code === 'ENOENT') return null; throw error; }
        if (stat.isSymbolicLink()) throw new Error(`Refusing cleanup through a symlink: ${relative}`);
        if (index < parts.length - 1 && !stat.isDirectory()) throw new Error(`Cleanup parent is not a directory: ${relative}`);
    }
    return current;
}

function sizeOf(filename) {
    const stat = lstatSync(filename);
    // In particular, never traverse symlinks inside generated trees.
    if (!stat.isDirectory()) return stat.size;
    return readdirSync(filename).reduce((bytes, name) => bytes + sizeOf(path.join(filename, name)), 0);
}

function sha256(filename) {
    const hash = createHash('sha256');
    const fd = openSync(filename, 'r');
    try {
        const buffer = Buffer.alloc(1024 * 1024);
        let bytes;
        while ((bytes = readSync(fd, buffer, 0, buffer.length, null)) > 0) hash.update(buffer.subarray(0, bytes));
        return hash.digest('hex');
    } finally { closeSync(fd); }
}

function githubRelease(tag, directory) {
    return JSON.parse(execFileSync('gh', ['api', '--hostname', 'github.com', `repos/oliverjessner/BulkPixel/releases/tags/${tag}`], {
        cwd: directory, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 30_000, maxBuffer: 8 * 1024 ** 2,
    }));
}

function olderThan(version, current) {
    const parts = value => /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value) ? value.split('.').map(BigInt) : null;
    const left = parts(version), right = parts(current);
    if (!left || !right) return false;
    for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] < right[i];
    return false;
}

// BulkPixel keeps release bundles inside target/. Preserve them even when
// cleaning manually after an unsuccessful publish, including cross-target builds.
function splitTarget(directory, relative) {
    const filename = path.join(directory, relative);
    if (!lstatSync(filename).isDirectory()) return { paths: [relative], bundles: [] };
    if (path.basename(relative) === 'bundle') return { paths: [], bundles: [relative] };
    const children = readdirSync(filename).map(name => {
        const child = `${relative}/${name}`;
        // rm removes internal symlinks themselves without following them.
        return lstatSync(path.join(directory, child)).isSymbolicLink()
            ? { paths: [child], bundles: [] } : splitTarget(directory, child);
    });
    const bundles = children.flatMap(child => child.bundles);
    return { paths: bundles.length ? children.flatMap(child => child.paths) : [relative], bundles };
}

function planOldDmgs(directory, bundles, getRelease) {
    const entries = [], preserved = [];
    const { version: current } = JSON.parse(readFileSync(path.join(directory, 'package.json'), 'utf8'));
    let unavailable = false;
    for (const bundle of bundles) {
        preserved.push({ path: bundle, reason: 'App bundles, current releases and unverified files are retained.' });
        const dmgDirectory = safeTarget(directory, `${bundle}/dmg`);
        if (!dmgDirectory) continue;
        for (const name of readdirSync(dmgDirectory)) {
            const version = /^BulkPixel_(\d+\.\d+\.\d+)_aarch64_adhoc\.dmg$/.exec(name)?.[1];
            if (!version || !olderThan(version, current)) continue;
            const relative = `${bundle}/dmg/${name}`;
            const filename = safeTarget(directory, relative);
            if (!filename || !lstatSync(filename).isFile()) continue;
            try {
                if (unavailable) throw new Error('GitHub verification unavailable.');
                let remote;
                try { remote = getRelease(`v${version}`, directory); }
                catch { unavailable = true; throw new Error('GitHub verification unavailable; DMG retained.'); }
                const asset = remote?.assets?.find(asset => asset.name === name);
                if (remote?.tag_name !== `v${version}` || remote.draft !== false || !remote.published_at
                    || asset?.state !== 'uploaded' || asset.digest !== `sha256:${sha256(filename)}`) {
                    throw new Error('No published GitHub DMG with a matching SHA-256 digest.');
                }
                entries.push({ path: relative, bytes: sizeOf(filename) });
            } catch (error) { preserved.push({ path: relative, reason: error.message }); }
        }
    }
    return { entries, preserved };
}

function assertUntracked(directory, paths) {
    if (!paths.length) return;
    const tracked = execFileSync('git', ['--literal-pathspecs', 'ls-files', '-z', '--', ...paths], {
        cwd: directory, encoding: 'utf8', maxBuffer: 8 * 1024 ** 2,
    }).split('\0').filter(Boolean);
    if (tracked.length) throw new Error(`Refusing to remove tracked files: ${tracked.join(', ')}`);
}

export function planCleanup(directory = root, { getRelease = githubRelease } = {}) {
    const absolute = projectRoot(directory);
    // Validate every allowlisted root before planning or deleting anything.
    const available = cleanupPaths.filter(relative => safeTarget(absolute, relative));
    const target = available.includes('src-tauri/target')
        ? splitTarget(absolute, 'src-tauri/target') : { paths: [], bundles: [] };
    const releases = planOldDmgs(absolute, target.bundles, getRelease);
    const paths = [...target.paths, ...available.filter(relative => relative !== 'src-tauri/target')];
    const entries = paths.map(relative => ({ path: relative, bytes: sizeOf(path.join(absolute, relative)) }));
    entries.push(...releases.entries);
    assertUntracked(absolute, entries.map(entry => entry.path));
    return { root: absolute, entries, preserved: releases.preserved, bytes: entries.reduce((bytes, entry) => bytes + entry.bytes, 0) };
}

export function cleanupBuildArtifacts({ directory = root, dryRun = false, getRelease = githubRelease } = {}) {
    const plan = planCleanup(directory, { getRelease });
    if (!dryRun) {
        // Revalidate all destinations and the Git index before the first deletion.
        for (const entry of plan.entries) {
            // Internal symlinks are safe to unlink, but their ancestors may never be links.
            const parent = path.posix.dirname(entry.path);
            if (parent !== '.') safeTarget(plan.root, parent);
        }
        assertUntracked(plan.root, plan.entries.map(entry => entry.path));
        for (const entry of plan.entries) rmSync(path.join(plan.root, entry.path), { recursive: true, force: true });
    }
    return { ...plan, dryRun };
}

export function runCleanup({ directory = root, dryRun = false, getRelease = githubRelease } = {}) {
    if (dryRun) return cleanupBuildArtifacts({ directory, dryRun, getRelease });
    const absolute = projectRoot(directory);
    const unlock = acquirePublishLock(absolute);
    try { return cleanupBuildArtifacts({ directory: absolute, getRelease }); }
    finally { unlock(); }
}

export function formatCleanupReport(result) {
    return [
        `${result.dryRun ? 'Would remove' : 'Removed'} ${result.entries.length} generated paths (${(result.bytes / 1024 ** 2).toFixed(1)} MiB of file contents).`,
        ...result.entries.map(entry => `  ${entry.path}`),
        ...result.preserved.map(entry => `  Kept ${entry.path}: ${entry.reason}`),
        'Source files, installed dependencies, icons, screenshots, user data and backups are outside the cleanup allowlist.',
    ].join('\n');
}

export function main(args = process.argv.slice(2)) {
    const options = parseCleanupArgs(args);
    if (options.help) {
        console.log('Usage: npm run clean -- [--dry-run]\n\nRemove generated Rust builds, sidecars, URL-import resources/downloads, schemas and coverage.\nKeep app bundles and current or unverified DMGs. Older DMGs require a matching published GitHub SHA-256 digest (gh required).\n--dry-run previews paths and sizes without writing files.\nRun only when no development app, build or test is using these files. Successful publishes run cleanup automatically.');
        return;
    }
    console.log(formatCleanupReport(runCleanup(options)));
}

if (import.meta.main) {
    try { main(); }
    catch (error) { console.error(error.message); process.exitCode = 1; }
}
