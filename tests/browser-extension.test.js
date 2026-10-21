import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { HOST_NAME, MENU_ID, imageUrl, registerExtension, sendImage } from '../browser-extension/background.js';

test('sends the selected image URL with signed queries intact and no page data', async () => {
    const calls = [];
    const api = { runtime: { async sendNativeMessage(...args) { calls.push(args); return { ok: true }; } } };
    await sendImage(api, 'https://example.com/asset?token=a%2Bb%26c&width=42#preview');
    assert.deepEqual(calls, [[HOST_NAME, { action: 'import-image', url: 'https://example.com/asset?token=a%2Bb%26c&width=42' }]]);
});

test('rejects unsupported browser sources before calling the native host', async () => {
    for (const raw of [undefined, '', 'blob:https://example.com/id', 'data:image/png;base64,a', 'file:///tmp/a.png', 'javascript:alert(1)', 'https://u:p@example.com/a.png', 'https://example.com/a\n.png', 'x'.repeat(8193)]) {
        assert.throws(() => imageUrl(raw));
        await assert.rejects(sendImage({ runtime: { sendNativeMessage() { assert.fail('must not send'); } } }, raw));
    }
});

test('reports missing hosts and native failures without claiming import success', async () => {
    await assert.rejects(sendImage({ runtime: { async sendNativeMessage() { throw Error('host not found'); } } }, 'https://example.com/a.png'), /not connected/);
    await assert.rejects(sendImage({ runtime: { async sendNativeMessage() { return { ok: false, message: 'Cannot open app.' }; } } } , 'https://example.com/a.png'), /Cannot open app/);
});

test('registers only an image menu and opens setup with retry data when sending fails', async () => {
    const handlers = {};
    const menus = [];
    const tabs = [];
    const event = key => ({ addListener(callback) { handlers[key] = callback; } });
    const api = {
        runtime: {
            onInstalled: event('installed'), onStartup: event('startup'),
            getURL: path => `chrome-extension://test/${path}`,
            async sendNativeMessage() { throw Error('host not found'); },
        },
        action: { onClicked: event('action') },
        contextMenus: { async removeAll() { menus.length = 0; }, create(menu) { menus.push(menu); }, onClicked: event('context') },
        tabs: { async create(tab) { tabs.push(tab); } },
    };
    registerExtension(api);
    handlers.installed({ reason: 'update' });
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(menus, [{ id: MENU_ID, title: 'Send to BulkPixel', contexts: ['image'] }]);
    handlers.context({ menuItemId: 'other', srcUrl: 'https://example.com/wrong.png' });
    handlers.context({ menuItemId: MENU_ID, srcUrl: 'https://example.com/right.png', pageUrl: 'https://example.com/never-sent' });
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(tabs.length, 1);
    const parameters = new URLSearchParams(new URL(tabs[0].url).hash.slice(1));
    assert.equal(parameters.get('url'), 'https://example.com/right.png');
    assert.match(parameters.get('message'), /not connected/);
});

test('extension has no page, history, cookie, network, or scripting permissions', async () => {
    const manifest = JSON.parse(await readFile(new URL('../browser-extension/manifest.json', import.meta.url)));
    assert.deepEqual(manifest.permissions, ['contextMenus', 'nativeMessaging']);
    assert.equal(manifest.host_permissions, undefined);
    assert.equal(manifest.content_scripts, undefined);
});
