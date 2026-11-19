import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { cleanupBuildArtifacts, formatCleanupReport, projectRoot, root } from './cleanup.mjs';
import { acquirePublishLock } from './publish-lock.mjs';

export async function publishAndCleanup({ publish, cleanup = cleanupBuildArtifacts, log = console.log }) {
    await publish();
    try { log(formatCleanupReport(cleanup())); }
    catch (error) {
        throw new Error(`Publishing succeeded, but local cleanup failed: ${error.message}\nResolve the issue and run npm run clean.`, { cause: error });
    }
}

export async function main(args = process.argv.slice(2)) {
    if (args.length) {
        if (args.length === 1 && ['--help', '-h'].includes(args[0])) {
            console.log('Usage: npm run publish\n\nBuild, sign, publish the GitHub release and update the Homebrew tap, then clean generated caches.\nFailed publishes retain build artifacts. Preview cleanup with npm run clean -- --dry-run.');
            return;
        }
        throw new Error(`Unknown publish option: ${args.join(' ')}`);
    }
    const directory = projectRoot(root);
    const unlock = acquirePublishLock(directory);
    try {
        await publishAndCleanup({
            publish() {
                const result = spawnSync('sh', [path.join(directory, 'scripts/publish.sh')], {
                    cwd: directory, stdio: 'inherit', env: { ...process.env, BULKPIXEL_PUBLISH_LOCK_HELD: String(process.pid) },
                });
                if (result.error) throw result.error;
                if (result.status !== 0) throw new Error(`Publish failed (${result.signal ?? result.status}); build artifacts were retained.`);
            },
        });
    } finally { unlock(); }
}

if (import.meta.main) {
    main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
