import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { cleanupBuildArtifacts, cleanupPaths, formatCleanupReport, parseCleanupArgs, planCleanup, runCleanup } from '../scripts/cleanup.mjs';
import { acquirePublishLock, lockName } from '../scripts/publish-lock.mjs';
import { publishAndCleanup } from '../scripts/publish.mjs';

function write(directory, relative, contents = 'generated') {
    const filename = path.join(directory, relative);
    mkdirSync(path.dirname(filename), { recursive: true });
    writeFileSync(filename, contents);
}

function fixture(t) {
    const directory = mkdtempSync(path.join(tmpdir(), 'bulkpixel-cleanup-test-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    execFileSync('git', ['init', '--quiet', directory]);
    write(directory, 'package.json', JSON.stringify({ name: 'bulkpixel', version: '3.2.0' }));
    write(directory, 'src-tauri/Cargo.toml', '[package]\nname = "bulkpixel"\n');
    return directory;
}

test('cleanup removes only generated paths and keeps source, data, dependencies and screenshots', t => {
    const directory = fixture(t);
    for (const relative of cleanupPaths) write(directory, `${relative}/output`);
    const protectedFiles = [
        'src/main.js', 'src-tauri/icons/icon.icns', 'src/assets/mockups/manual.webp',
        'tests/test_images/original.png', 'data/presets.sqlite3', 'backups/original.png',
        '.env', 'node_modules/example/index.js', 'src/vendor/oj-designsystem/main.css',
        'artifacts/manual.txt', 'Cargo.lock',
    ];
    for (const relative of protectedFiles) write(directory, relative, 'keep');
    const result = runCleanup({ directory });
    assert.deepEqual(result.entries.map(entry => entry.path), cleanupPaths);
    for (const relative of cleanupPaths) assert.equal(existsSync(path.join(directory, relative)), false, relative);
    for (const relative of protectedFiles) assert.equal(readFileSync(path.join(directory, relative), 'utf8'), 'keep', relative);
    assert.equal(runCleanup({ directory }).entries.length, 0);
    assert.equal(existsSync(path.join(directory, lockName)), false);
});

test('dry-run reports sizes without creating a lock or deleting files; unknown flags fail', t => {
    const directory = fixture(t);
    write(directory, 'src-tauri/target/debug/output', '123456');
    write(directory, '.url-import-cache/archive', '1234');
    const before = planCleanup(directory);
    const result = runCleanup({ directory, dryRun: true });
    assert.deepEqual(result, { ...before, dryRun: true });
    assert.equal(result.bytes, 10);
    assert.equal(readFileSync(path.join(directory, 'src-tauri/target/debug/output'), 'utf8'), '123456');
    assert.equal(existsSync(path.join(directory, lockName)), false);
    assert.match(formatCleanupReport(result), /Would remove 2 generated paths/);
    assert.deepEqual(parseCleanupArgs(['--dry-run']), { dryRun: true, help: false });
    assert.equal(parseCleanupArgs(['--help']).help, true);
    assert.throws(() => parseCleanupArgs(['--root', '/']), /Unknown cleanup option/);
});

test('all tracked-file and symlink checks finish before the first deletion', t => {
    for (const failure of ['tracked', 'symlink-root', 'symlink-ancestor']) {
        const directory = fixture(t);
        write(directory, 'src-tauri/target/debug/keep-on-failure');
        if (failure === 'tracked') {
            write(directory, 'coverage/important.txt', 'tracked');
            execFileSync('git', ['add', 'coverage/important.txt'], { cwd: directory });
        } else {
            write(directory, 'private/keep', 'private');
            const relative = failure === 'symlink-root' ? 'src-tauri/binaries' : 'src-tauri/resources';
            symlinkSync(path.join(directory, 'private'), path.join(directory, relative), 'dir');
        }
        assert.throws(() => runCleanup({ directory }), /Refusing (to remove tracked files|cleanup through a symlink)/);
        assert.equal(existsSync(path.join(directory, 'src-tauri/target/debug/keep-on-failure')), true);
        assert.equal(existsSync(path.join(directory, lockName)), false);
    }
});

test('symlinks inside generated output are unlinked without traversing their targets', t => {
    const directory = fixture(t);
    write(directory, 'private/data.sqlite', 'private');
    write(directory, 'src-tauri/target/debug/output');
    // Force target splitting, as in a real published app with a retained bundle.
    write(directory, 'src-tauri/target/release/bundle/macos/BulkPixel.app/Contents/MacOS/bulkpixel', 'app');
    symlinkSync(path.join(directory, 'private'), path.join(directory, 'src-tauri/target/linked-data'), 'dir');
    runCleanup({ directory });
    assert.equal(existsSync(path.join(directory, 'src-tauri/target/debug')), false);
    assert.equal(existsSync(path.join(directory, 'src-tauri/target/linked-data')), false);
    assert.equal(readFileSync(path.join(directory, 'private/data.sqlite'), 'utf8'), 'private');
});

test('native and cross-target bundles, current DMGs and failed-publish files survive cleanup', t => {
    const directory = fixture(t);
    const retained = [
        'src-tauri/target/release/bundle/macos/BulkPixel.app/Contents/MacOS/bulkpixel',
        'src-tauri/target/release/bundle/dmg/BulkPixel_3.2.0_aarch64_adhoc.dmg',
        'src-tauri/target/release/bundle/dmg/unknown-retry.dmg',
        'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/BulkPixel.app/retry',
    ];
    for (const relative of retained) write(directory, relative, 'retain');
    write(directory, 'src-tauri/target/debug/deps/output');
    write(directory, 'src-tauri/target/release/deps/output');
    write(directory, 'src-tauri/target/aarch64-apple-darwin/release/deps/output');
    const result = runCleanup({ directory, getRelease() { throw new Error('Current releases must not be queried.'); } });
    for (const relative of retained) assert.equal(readFileSync(path.join(directory, relative), 'utf8'), 'retain');
    assert.equal(existsSync(path.join(directory, 'src-tauri/target/release/deps')), false);
    assert.equal(result.preserved.length, 2);
});

test('only older DMGs with an exact published GitHub asset digest can be removed', t => {
    for (const outcome of ['published', 'mismatch', 'draft', 'missing', 'offline']) {
        const directory = fixture(t);
        const name = 'BulkPixel_3.1.9_aarch64_adhoc.dmg';
        const relative = `src-tauri/target/release/bundle/dmg/${name}`;
        write(directory, relative, 'old-dmg');
        const digest = createHash('sha256').update('old-dmg').digest('hex');
        const getRelease = tag => {
            assert.equal(tag, 'v3.1.9');
            if (outcome === 'offline') throw new Error('offline');
            if (outcome === 'missing') return null;
            return {
                tag_name: tag, draft: outcome === 'draft', published_at: '2026-11-01T00:00:00Z',
                assets: [{ name, state: 'uploaded', digest: `sha256:${outcome === 'mismatch' ? '0'.repeat(64) : digest}` }],
            };
        };
        const preview = runCleanup({ directory, dryRun: true, getRelease });
        assert.equal(existsSync(path.join(directory, relative)), true);
        assert.equal(preview.entries.some(entry => entry.path === relative), outcome === 'published');
        runCleanup({ directory, getRelease });
        assert.equal(existsSync(path.join(directory, relative)), outcome !== 'published');
    }
});

test('cleanup refuses another project, a nested checkout root and symlinked locks', t => {
    const directory = fixture(t);
    write(directory, 'src-tauri/target/output');
    write(directory, 'package.json', JSON.stringify({ name: 'pitchary' }));
    assert.throws(() => planCleanup(directory), /BulkPixel project root/);
    write(directory, 'package.json', JSON.stringify({ name: 'bulkpixel' }));
    write(directory, 'nested/package.json', JSON.stringify({ name: 'bulkpixel' }));
    write(directory, 'nested/src-tauri/Cargo.toml', '[package]\nname = "bulkpixel"\n');
    assert.throws(() => planCleanup(path.join(directory, 'nested')), /root of the BulkPixel checkout/);
    write(directory, 'private/keep', 'private');
    symlinkSync(path.join(directory, 'private'), path.join(directory, lockName), 'dir');
    assert.throws(() => runCleanup({ directory }), /symlink publish lock/);
    assert.equal(readFileSync(path.join(directory, 'private/keep'), 'utf8'), 'private');
    assert.equal(existsSync(path.join(directory, 'src-tauri/target/output')), true);
});

test('publish and standalone cleanup share a lock; previews remain read-only', t => {
    const directory = fixture(t);
    write(directory, 'src-tauri/target/output');
    const unlock = acquirePublishLock(directory);
    try {
        assert.throws(() => runCleanup({ directory }), /already running/);
        runCleanup({ directory, dryRun: true });
        assert.equal(existsSync(path.join(directory, 'src-tauri/target/output')), true);
    } finally { unlock(); }
    unlock();
    runCleanup({ directory });
    assert.equal(existsSync(path.join(directory, 'src-tauri/target/output')), false);
});

test('dead lock owners recover while incomplete or unfamiliar locks are retained', t => {
    const directory = fixture(t);
    const child = spawnSync(process.execPath, ['-e', ''], { encoding: 'utf8' });
    assert.equal(child.status, 0);
    write(directory, `${lockName}/${child.pid}.pid`, 'old-owner');
    const unlock = acquirePublishLock(directory);
    assert.equal(existsSync(path.join(directory, lockName, `${process.pid}.pid`)), true);
    unlock();
    write(directory, `${lockName}/keep.txt`, 'do not delete');
    assert.throws(() => acquirePublishLock(directory), /lock is incomplete/);
    assert.equal(readFileSync(path.join(directory, lockName, 'keep.txt'), 'utf8'), 'do not delete');
});

test('a tracked lock file is never deleted or overwritten', t => {
    const directory = fixture(t);
    const relative = `${lockName}/${process.pid}.pid`;
    write(directory, relative, 'tracked-owner');
    execFileSync('git', ['add', relative], { cwd: directory });
    assert.throws(() => runCleanup({ directory }), /tracked publish lock/);
    assert.equal(readFileSync(path.join(directory, relative), 'utf8'), 'tracked-owner');
});

test('automatic cleanup runs after successful publishing and never after a failed build, upload or tap update', async () => {
    const operations = [];
    await publishAndCleanup({
        async publish() { operations.push('build', 'upload', 'tap push'); },
        cleanup() { operations.push('cleanup'); return { entries: [], preserved: [], bytes: 0, dryRun: false }; },
        log() {},
    });
    assert.deepEqual(operations, ['build', 'upload', 'tap push', 'cleanup']);
    for (const step of ['build', 'upload', 'tap push']) {
        await assert.rejects(publishAndCleanup({
            async publish() { throw new Error(`${step} failed`); },
            cleanup() { assert.fail('cleanup must not run after a publish failure'); },
        }), new RegExp(`${step} failed`));
    }
    await assert.rejects(publishAndCleanup({
        async publish() {},
        cleanup() { throw new Error('tracked output'); },
    }), /Publishing succeeded, but local cleanup failed/);
});

test('the publish subprocess holds the lock, preserves failed artifacts and cleans after success', t => {
    const directory = fixture(t);
    for (const name of ['publish.mjs', 'publish-lock.mjs', 'cleanup.mjs']) {
        write(directory, `scripts/${name}`, readFileSync(new URL(`../scripts/${name}`, import.meta.url)));
    }
    write(directory, 'src-tauri/target/debug/output', 'retry');
    const shell = '[ "$BULKPIXEL_PUBLISH_LOCK_HELD" = "$PPID" ] || exit 40\n[ -d .bulkpixel-publish.lock ] || exit 41\n';
    write(directory, 'scripts/publish.sh', `${shell}exit 12\n`);
    const invoke = () => spawnSync(process.execPath, [path.join(directory, 'scripts/publish.mjs')], { cwd: directory, encoding: 'utf8' });
    const failed = invoke();
    assert.equal(failed.error, undefined);
    assert.equal(failed.status, 1);
    assert.match(failed.stderr, /Publish failed \(12\)/);
    assert.equal(readFileSync(path.join(directory, 'src-tauri/target/debug/output'), 'utf8'), 'retry');
    assert.equal(existsSync(path.join(directory, lockName)), false);
    write(directory, 'scripts/publish.sh', `${shell}exit 0\n`);
    const success = invoke();
    assert.equal(success.error, undefined);
    assert.equal(success.status, 0, success.stderr);
    assert.match(success.stdout, /Removed 1 generated paths/);
    assert.equal(existsSync(path.join(directory, 'src-tauri/target')), false);
    assert.equal(existsSync(path.join(directory, lockName)), false);
});

test('direct shell publish --help delegates safely without building or publishing', () => {
    const result = spawnSync('sh', [new URL('../scripts/publish.sh', import.meta.url).pathname, '--help'], {
        encoding: 'utf8', env: { ...process.env, BULKPIXEL_PUBLISH_LOCK_HELD: '' },
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Usage: npm run publish/);
});
