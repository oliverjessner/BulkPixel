import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildResizeHelperText,
    buildResizeOutputText,
    buildResizeReferenceNote,
    buildResizeReferenceText,
} from '../src/ui.js';

test('describes the original resize state without editable dimensions', () => {
    const state = {
        resizeMode: 'none',
        resizeReference: null,
        width: '',
        height: '',
    };

    assert.equal(buildResizeReferenceText(state), 'Waiting for images');
    assert.equal(buildResizeReferenceNote(state), '');
    assert.equal(buildResizeOutputText(state), '');
    assert.equal(buildResizeHelperText(state), 'Images keep their original size.');
});

test('shows the calculated reference output for width resizing', () => {
    const state = {
        resizeMode: 'width',
        resizeReference: { width: 1920, height: 1080, mixedSizes: true },
        width: '1200',
        height: '675',
    };

    assert.equal(buildResizeReferenceText(state), '1920 × 1080');
    assert.equal(buildResizeReferenceNote(state), 'Based on first image');
    assert.equal(buildResizeOutputText(state), '1200 × 675');
    assert.equal(buildResizeHelperText(state), 'Height is calculated automatically.');
});

test('shows the calculated reference output for height resizing', () => {
    const state = {
        resizeMode: 'height',
        resizeReference: { width: 1920, height: 1080, mixedSizes: false },
        width: '1422',
        height: '800',
    };

    assert.equal(buildResizeReferenceNote(state), '');
    assert.equal(buildResizeOutputText(state), '1422 × 800');
    assert.equal(buildResizeHelperText(state), 'Width is calculated automatically.');
});
