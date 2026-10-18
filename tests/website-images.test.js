import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { extractSnapshot } from '../scripts/extract-website-images.cjs';

test('Fetchary extracts all static image sources from a native snapshot without network access', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'bulkpixel-page-'));
    const filename = path.join(directory, 'page.html');
    try {
        await writeFile(filename, `<html><head><base href="https://cdn.example.com/assets/">
            <meta property="og:image" content="social.png"></head><body>
            <img src="one.JPG?token=kept#fragment" srcset="two.webp 2x, three.avif 3x">
            <picture><source srcset="four.png 1x"></picture>
            <img data-src="lazy.gif"><img src="one.JPG?token=kept#fragment">
            <div style="background-image:url('background.svg')"></div>
            <a href="download.png">Original</a>
            <img src="data:image/png;base64,abc"><img src="javascript:alert(1)">
            <script>throw new Error('must not execute')</script></body></html>`);
        const urls = await extractSnapshot({ url: 'https://does-not-resolve.invalid/gallery', path: filename, contentType: 'text/html' });
        assert.deepEqual(new Set(urls), new Set([
            'https://cdn.example.com/assets/one.JPG?token=kept#fragment',
            ...['two.webp', 'three.avif', 'four.png', 'lazy.gif', 'social.png', 'background.svg', 'download.png'].map(name => `https://cdn.example.com/assets/${name}`),
        ]));
        assert.equal(urls.length, 8);
    } finally { await rm(directory, { recursive: true, force: true }); }
});

test('Fetchary returns an empty list for a page without images', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'bulkpixel-empty-page-'));
    try {
        const filename = path.join(directory, 'page.html');
        await writeFile(filename, '<html><body>No images</body></html>');
        assert.deepEqual(await extractSnapshot({ url: 'https://does-not-resolve.invalid/', path: filename, contentType: 'text/html' }), []);
    } finally { await rm(directory, { recursive: true, force: true }); }
});
