import assert from 'node:assert/strict';
import test from 'node:test';
import { createBrowserImports } from '../src/browser-import.js';

const tick = () => new Promise(resolve => setImmediate(resolve));

function harness({ busy = false, initial = { urls: [], errors: [] }, importImages = async () => {} } = {}) {
    const batches = [initial];
    const imported = [];
    const errors = [];
    let listener;
    let removed = false;
    const receiver = createBrowserImports({
        invoke: async command => { assert.equal(command, 'take_browser_imports'); return batches.shift() ?? { urls: [], errors: [] }; },
        eventApi: { async listen(name, callback) { assert.equal(name, 'browser-import-pending'); listener = callback; return () => { removed = true; }; } },
        isBusy: () => busy,
        async importImages(urls) { imported.push(urls); await importImages(urls); },
        onError: error => errors.push(error),
    });
    return {
        receiver, imported, errors,
        setBusy(value) { busy = value; receiver.notifyIdle(); },
        send(batch) { batches.push(batch); listener(); },
        get removed() { return removed; },
    };
}

test('drains cold-start requests only after listener registration', async () => {
    const h = harness({ initial: { urls: ['https://example.com/a.png'], errors: [] } });
    await h.receiver.start();
    await tick();
    assert.deepEqual(h.imported, [['https://example.com/a.png']]);
    h.receiver.destroy();
    assert.equal(h.removed, true);
});

test('retains and deduplicates images while conversion or a dialog is busy', async () => {
    const h = harness({ busy: true });
    await h.receiver.start();
    h.send({ urls: ['https://example.com/a.png', 'https://example.com/a.png'], errors: [] });
    h.send({ urls: ['https://example.com/b.png'], errors: [] });
    await tick();
    assert.deepEqual(h.imported, []);
    h.setBusy(false);
    await tick();
    assert.deepEqual(h.imported, [['https://example.com/a.png', 'https://example.com/b.png']]);
});

test('serializes incoming imports without losing a click during a running import', async () => {
    let finish;
    const h = harness({ importImages: () => new Promise(resolve => { finish = resolve; }) });
    await h.receiver.start();
    h.send({ urls: ['https://example.com/a.png'], errors: [] });
    await tick();
    h.send({ urls: ['https://example.com/b.png'], errors: [] });
    await tick();
    assert.equal(h.imported.length, 1);
    finish();
    await tick();
    assert.deepEqual(h.imported, [['https://example.com/a.png'], ['https://example.com/b.png']]);
    finish();
});

test('shows errors, bounds pending requests, and releases listeners on shutdown', async () => {
    const h = harness({ busy: true });
    await h.receiver.start();
    h.send({ urls: Array.from({ length: 100 }, (_, n) => `https://example.com/${n}.png`), errors: ['Rejected unsafe URL.'] });
    h.send({ urls: ['https://example.com/overflow.png'], errors: [] });
    await tick();
    assert(h.errors.includes('Rejected unsafe URL.'));
    assert(h.errors.some(error => error.includes('queue is full')));
    h.receiver.destroy();
    h.setBusy(false);
    await tick();
    assert.deepEqual(h.imported, []);
});
