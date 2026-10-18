import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const cache = path.join(root, '.url-import-cache');
const nodeVersion = '26.11.1';
const target = process.env.TAURI_ENV_TARGET_TRIPLE || execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)[1];
const arch = { 'aarch64-apple-darwin': 'arm64', 'x86_64-apple-darwin': 'x64' }[target];
if (!arch) throw new Error(`Website import packaging does not support ${target}.`);
mkdirSync(cache, { recursive: true });

if (!process.argv.includes('--locked')) {
    execFileSync('/usr/bin/lockf', ['-k', path.join(cache, 'prepare.lock'), process.execPath, fileURLToPath(import.meta.url), '--locked'], {
        cwd: root, stdio: 'inherit', env: { ...process.env, TAURI_ENV_TARGET_TRIPLE: target },
    });
} else {
    prepare();
}

function download(url, destination) {
    const partial = `${destination}.download`;
    try {
        execFileSync('curl', ['--fail', '--location', '--silent', '--show-error', '--retry', '2', '--max-time', '180', url, '--output', partial], { stdio: 'inherit' });
        renameSync(partial, destination);
    } finally { rmSync(partial, { force: true }); }
}

function prepare() {
    const resource = path.join(root, 'src-tauri/resources/url-import');
    const binary = path.join(root, `src-tauri/binaries/bulkpixel-node-${target}`);
    const fingerprint = createHash('sha256').update(nodeVersion + target)
        .update(readFileSync(path.join(root, 'package-lock.json')))
        .update(readFileSync(fileURLToPath(import.meta.url)))
        .update(readFileSync(path.join(root, 'scripts/extract-website-images.cjs'))).digest('hex');
    if (existsSync(binary) && existsSync(path.join(resource, 'fingerprint'))
        && readFileSync(path.join(resource, 'fingerprint'), 'utf8') === fingerprint) return;

    const archive = `node-v${nodeVersion}-darwin-${arch}.tar.gz`;
    const archivePath = path.join(cache, archive);
    const checksums = path.join(cache, `SHASUMS256-${nodeVersion}.txt`);
    const baseUrl = `https://nodejs.org/dist/v${nodeVersion}/`;
    if (!existsSync(checksums)) download(`${baseUrl}SHASUMS256.txt`, checksums);
    const expected = readFileSync(checksums, 'utf8').split('\n').find(line => line.endsWith(`  ${archive}`))?.split(' ')[0];
    if (!expected) throw new Error(`Missing official Node checksum for ${archive}.`);
    if (!existsSync(archivePath)) download(`${baseUrl}${archive}`, archivePath);
    if (createHash('sha256').update(readFileSync(archivePath)).digest('hex') !== expected) {
        rmSync(archivePath);
        throw new Error('Node runtime checksum mismatch. Run preparation again.');
    }
    const staged = mkdtempSync(path.join(cache, 'runtime-'));
    try {
        execFileSync('tar', ['-xzf', archivePath, '-C', staged]);
        const nodeRoot = path.join(staged, `node-v${nodeVersion}-darwin-${arch}`);
        const runtime = path.join(staged, 'url-import');
        mkdirSync(runtime);
        copyFileSync(path.join(root, 'scripts/extract-website-images.cjs'), path.join(runtime, 'extract.cjs'));
        copyFileSync(path.join(nodeRoot, 'LICENSE'), path.join(runtime, 'NODE-LICENSE'));
        // Keep Fetchary's complete installed dependency tree and licenses intact.
        // Chromium is unnecessary: extraction uses the HTTP snapshot only.
        const dependencies = JSON.parse(execFileSync('npm', ['query', '#fetchary, #fetchary *', '--omit=dev'], { cwd: root, encoding: 'utf8' })).map(dependency => dependency.path);
        for (const dependency of dependencies) {
            const relative = path.relative(root, dependency);
            if (!relative.startsWith('node_modules/')) throw new Error(`Dependency outside project: ${relative}`);
            cpSync(dependency, path.join(runtime, relative), { recursive: true, dereference: true });
        }
        writeFileSync(path.join(runtime, 'fingerprint'), fingerprint);
        mkdirSync(path.dirname(binary), { recursive: true });
        const pending = `${binary}.tmp`;
        copyFileSync(path.join(nodeRoot, 'bin/node'), pending);
        chmodSync(pending, 0o755);
        execFileSync('codesign', ['--force', '--sign', '-', pending], { stdio: 'inherit' });
        // Publish complete resources before Tauri's build starts reading them.
        mkdirSync(path.dirname(resource), { recursive: true });
        const previous = path.join(staged, 'previous');
        if (existsSync(resource)) renameSync(resource, previous);
        try { renameSync(runtime, resource); }
        catch (error) { if (existsSync(previous)) renameSync(previous, resource); throw error; }
        renameSync(pending, binary);
        console.log(`Prepared Fetchary and Node ${nodeVersion} for website imports (${target}).`);
    } finally { rmSync(staged, { recursive: true, force: true }); }
}
