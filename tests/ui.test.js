import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildResizeInputState,
    buildResizeReferenceNote,
} from '../src/ui.js';

test('shows both original dimensions as read-only inputs', () => {
    const state = {
        resizeMode: 'none',
        resizeReference: { width: 1920, height: 1080, mixedSizes: false },
        width: '1920',
        height: '1080',
    };

    assert.deepEqual(buildResizeInputState(state), {
        widthValue: '1920',
        heightValue: '1080',
        widthReadOnly: true,
        heightReadOnly: true,
    });
    assert.equal(buildResizeReferenceNote(state), '');
});

test('shows the calculated reference output for width resizing', () => {
    const state = {
        resizeMode: 'width',
        resizeReference: { width: 1920, height: 1080, mixedSizes: true },
        width: '1200',
        height: '675',
    };

    assert.deepEqual(buildResizeInputState(state), {
        widthValue: '1200',
        heightValue: '675',
        widthReadOnly: false,
        heightReadOnly: true,
    });
    assert.equal(buildResizeReferenceNote(state), 'Based on first image');
});

test('shows the calculated reference output for height resizing', () => {
    const state = {
        resizeMode: 'height',
        resizeReference: { width: 1920, height: 1080, mixedSizes: false },
        width: '1422',
        height: '800',
    };

    assert.deepEqual(buildResizeInputState(state), {
        widthValue: '1422',
        heightValue: '800',
        widthReadOnly: true,
        heightReadOnly: false,
    });
    assert.equal(buildResizeReferenceNote(state), '');
});
