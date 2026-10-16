import assert from 'node:assert/strict';
import test from 'node:test';
import { clipboardImageUrls, isEditablePaste, parseImageUrls } from '../src/url-import.js';

test('parses multiple URLs, ignores empty lines, trims and deduplicates without losing queries', () => {
    const result = parseImageUrls(' https://example.com/image?id=42 \n\nhttp://cdn.example.com/123\nhttps://example.com/image?id=42#ignored');
    assert.deepEqual(result.urls, ['https://example.com/image?id=42', 'http://cdn.example.com/123']);
    assert.equal(result.duplicates, 1);
    assert.deepEqual(result.errors, []);
});

test('rejects non-HTTP URLs, credentials, broken URLs and multiple URLs on one line', () => {
    const result = parseImageUrls('file:///etc/passwd\njavascript:alert(1)\nhttps://user:pass@example.com/a\nnot a url\nhttps://example.com/a https://example.com/b\nhttps://example.com/good.png');
    assert.equal(result.errors.length, 5);
    assert.deepEqual(result.urls, ['https://example.com/good.png']);
});

test('detects clipboard URLs and leaves unrelated clipboard text alone', () => {
    assert.deepEqual(clipboardImageUrls('Image: https://example.com/a.png\nhttps://cdn.example.com/123?id=4'), ['https://example.com/a.png', 'https://cdn.example.com/123?id=4']);
    assert.deepEqual(clipboardImageUrls('ordinary text, file:///tmp/a.png'), []);
    assert.deepEqual(clipboardImageUrls('https://user:secret@example.com/a'), []);
});

test('never intercepts paste in native inputs, textareas, editable ancestors or shadow editors', () => {
    const editable = { closest: selector => selector.includes('textarea') ? {} : null };
    const plain = { closest: () => null };
    assert.equal(isEditablePaste({ target: editable }, plain), true);
    assert.equal(isEditablePaste({ target: plain }, editable), true);
    assert.equal(isEditablePaste({ target: plain, composedPath: () => [{ isContentEditable: true }] }, plain), true);
    assert.equal(isEditablePaste({ target: plain }, plain), false);
});

test('validates batch count and accepts extensionless URLs and signed queries', () => {
    assert.equal(parseImageUrls(Array.from({ length: 101 }, (_, i) => `https://example.com/${i}`).join('\n')).errors.length, 1);
    assert.deepEqual(parseImageUrls('https://example.com/asset?token=abc%2Bdef&width=1200').urls, ['https://example.com/asset?token=abc%2Bdef&width=1200']);
});
