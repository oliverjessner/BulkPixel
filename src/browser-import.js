import { parseImageUrls } from './url-import.js';

export function createBrowserImports({ invoke, eventApi, isBusy, importImages, onError, schedule = queueMicrotask }) {
    const pending = new Set();
    let started = false;
    let stopped = false;
    let importing = false;
    let draining = false;
    let drainAgain = false;
    let unlisten;

    async function pump() {
        if (!started || stopped || importing || isBusy() || !pending.size) return;
        importing = true;
        const urls = [...pending];
        pending.clear();
        try { await importImages(urls); }
        catch { onError('The browser image could not be imported. Send it again to retry.'); }
        finally { importing = false; notifyIdle(); }
    }

    function notifyIdle() { schedule(() => void pump()); }

    async function drain() {
        if (stopped) return;
        if (draining) { drainAgain = true; return; }
        draining = true;
        try {
            do {
                drainAgain = false;
                const batch = await invoke('take_browser_imports');
                if (stopped) return;
                const { urls, errors } = parseImageUrls((batch.urls ?? []).join('\n'));
                for (const url of urls) {
                    if (pending.size < 100) pending.add(url);
                    else if (!pending.has(url)) errors.push('The browser import queue is full. Send additional images after this import finishes.');
                }
                for (const error of [...(batch.errors ?? []), ...errors]) onError(error);
            } while (drainAgain && !stopped);
        } catch {
            onError('Browser imports are unavailable. Reopen BulkPixel and try again.');
        } finally { draining = false; notifyIdle(); }
    }

    return {
        notifyIdle,
        async start() {
            try {
                unlisten = await eventApi.listen('browser-import-pending', () => void drain());
                if (stopped) { unlisten(); return; }
                started = true;
                await drain();
            } catch { onError('Browser imports are unavailable. Reopen BulkPixel and try again.'); }
        },
        destroy() { stopped = true; pending.clear(); unlisten?.(); },
    };
}
