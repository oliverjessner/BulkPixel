import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { lstatSync, mkdirSync, readFileSync, readdirSync, rmdirSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export const lockName = '.bulkpixel-publish.lock';

export function acquirePublishLock(root) {
    const tracked = execFileSync('git', ['--literal-pathspecs', 'ls-files', '-z', '--', lockName], {
        cwd: root, encoding: 'utf8',
    });
    if (tracked) throw new Error('Refusing a tracked publish lock.');
    const directory = path.join(root, lockName);
    const owner = path.join(directory, `${process.pid}.pid`);
    for (;;) {
        try {
            mkdirSync(directory);
            break;
        } catch (error) {
            if (error.code !== 'EEXIST') throw error;
        }
        let entries;
        try {
            if (lstatSync(directory).isSymbolicLink()) throw new Error('Refusing a symlink publish lock.');
            entries = readdirSync(directory);
        } catch (error) {
            if (error.code === 'ENOENT') continue;
            throw error;
        }
        if (entries.length !== 1 || !/^[1-9]\d*\.pid$/.test(entries[0])) {
            throw new Error('A publish or cleanup is starting, or its lock is incomplete. Check running processes before removing the lock.');
        }
        const previousOwner = path.join(directory, entries[0]);
        if (!lstatSync(previousOwner).isFile()) throw new Error('Refusing an unexpected publish lock owner.');
        const pid = Number.parseInt(entries[0], 10);
        if (!Number.isSafeInteger(pid)) throw new Error('Invalid publish lock PID.');
        try {
            process.kill(pid, 0);
            throw new Error(`Another publish or cleanup is already running (PID ${pid}).`);
        } catch (error) {
            if (error.code !== 'ESRCH') throw error;
        }
        // Never recursively remove a lock; another process may already be taking it.
        try {
            unlinkSync(previousOwner);
            rmdirSync(directory);
        } catch (error) {
            if (error.code === 'ENOENT') continue;
            throw error;
        }
    }
    const token = randomUUID();
    try {
        writeFileSync(owner, token, { flag: 'wx' });
    } catch (error) {
        rmdirSync(directory);
        throw error;
    }
    let released = false;
    return () => {
        if (released) return;
        if (lstatSync(directory).isSymbolicLink() || !lstatSync(owner).isFile() || readFileSync(owner, 'utf8') !== token) {
            throw new Error('The publish lock owner changed; refusing to remove it.');
        }
        unlinkSync(owner);
        rmdirSync(directory);
        released = true;
    };
}
