import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildConversionActionBar,
    buildConversionJobSummary,
    buildInputFormatSummary,
    buildPresetLibrarySummary,
    buildResizeInputState,
    buildResizeSummary,
    getTotalInputSize,
} from '../src/ui.js';

function buildActionBarState(overrides = {}) {
    return {
        images: [],
        format: 'webp',
        resizeMode: 'width',
        width: '1200',
        height: '675',
        summary: null,
        status: { kind: 'info', text: 'Choose images to begin.' },
        validationMessage: '',
        isImporting: false,
        isProcessing: false,
        outputDirectory: '/tmp/output',
        ...overrides,
    };
}

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
});

test('builds a ready job summary from image metadata and resize settings', () => {
    const state = buildActionBarState({
        images: [
            { fileType: 'JPEG', fileSize: 1024 * 1024 },
            { fileType: 'PNG', fileSize: 2 * 1024 * 1024 },
        ],
    });

    assert.equal(buildInputFormatSummary(state.images), 'Mixed');
    assert.equal(getTotalInputSize(state.images), 3 * 1024 * 1024);
    assert.equal(buildResizeSummary(state), 'Width 1200 px');
    assert.equal(buildConversionJobSummary(state), '2 images · Mixed → WEBP · Width 1200 px');
    assert.deepEqual(buildConversionActionBar(state), {
        tone: 'info',
        primary: '2 images · Mixed → WEBP · Width 1200 px',
        secondary: '3.0 MB input',
        symbol: '',
        showSpinner: false,
        showClear: true,
        showFinder: false,
        convertLabel: 'Convert 2 Images',
        convertDisabled: false,
    });
});

test('shows an inactive empty conversion action bar', () => {
    const actionBar = buildConversionActionBar(buildActionBarState());

    assert.equal(actionBar.primary, 'No images selected');
    assert.equal(actionBar.secondary, 'Add images to begin.');
    assert.equal(actionBar.convertLabel, 'Convert');
    assert.equal(actionBar.convertDisabled, true);
    assert.equal(actionBar.showClear, false);
});

test('gives processing and validation states priority over job details', () => {
    const images = [{ fileType: 'JPEG', fileSize: 1024 }];
    const processing = buildConversionActionBar(
        buildActionBarState({ images, isProcessing: true }),
    );
    const invalid = buildConversionActionBar(
        buildActionBarState({ images, validationMessage: 'Width must be between 1 and 9999.' }),
    );

    assert.equal(processing.primary, 'Converting 1 image...');
    assert.equal(processing.showSpinner, true);
    assert.equal(processing.convertLabel, 'Converting...');
    assert.equal(processing.convertDisabled, true);
    assert.equal(invalid.primary, 'Width must be between 1 and 9999.');
    assert.equal(invalid.tone, 'warning');
    assert.equal(invalid.convertDisabled, true);
});

test('summarizes a successful conversion using real summary sizes', () => {
    const actionBar = buildConversionActionBar(
        buildActionBarState({
            images: Array.from({ length: 12 }, () => ({ fileType: 'JPEG', fileSize: 1024 })),
            status: { kind: 'success', text: 'Converted.' },
            summary: {
                successCount: 12,
                failureCount: 0,
                totalOriginalSize: 24 * 1024 * 1024,
                totalConvertedSize: 6 * 1024 * 1024,
                totalDeltaBytes: 18 * 1024 * 1024,
                totalPercentChange: 75,
            },
        }),
    );

    assert.equal(actionBar.tone, 'success');
    assert.equal(actionBar.primary, '12 images converted · 18.0 MB saved (75.0%)');
    assert.equal(actionBar.secondary, '24.0 MB → 6.0 MB');
    assert.equal(actionBar.showClear, true);
    assert.equal(actionBar.showFinder, true);
});

test('does not describe a larger conversion output as saved', () => {
    const actionBar = buildConversionActionBar(
        buildActionBarState({
            images: [{ fileType: 'PNG', fileSize: 10 * 1024 * 1024 }],
            status: { kind: 'success', text: 'Converted.' },
            summary: {
                successCount: 1,
                failureCount: 0,
                totalOriginalSize: 10 * 1024 * 1024,
                totalConvertedSize: 12 * 1024 * 1024,
                totalDeltaBytes: -2 * 1024 * 1024,
                totalPercentChange: -20,
            },
        }),
    );

    assert.equal(actionBar.primary, '1 image converted · output is 2.0 MB larger (20.0%)');
    assert.equal(actionBar.primary.includes('saved'), false);
});

test('distinguishes partial success from complete failure', () => {
    const images = Array.from({ length: 3 }, () => ({ fileType: 'HEIC', fileSize: 1024 }));
    const partial = buildConversionActionBar(
        buildActionBarState({
            images,
            status: { kind: 'warning', text: 'Some images failed.' },
            summary: {
                successCount: 2,
                failureCount: 1,
                totalOriginalSize: 3000,
                totalConvertedSize: 2000,
                totalDeltaBytes: 1000,
                totalPercentChange: 33.3,
            },
        }),
    );
    const failure = buildConversionActionBar(
        buildActionBarState({
            images,
            status: { kind: 'error', text: 'No images were converted.' },
            summary: {
                successCount: 0,
                failureCount: 3,
                totalOriginalSize: 3000,
                totalConvertedSize: 0,
                totalDeltaBytes: 0,
                totalPercentChange: 0,
            },
        }),
    );

    assert.equal(partial.tone, 'warning');
    assert.equal(partial.primary, '2 of 3 images converted · 1 failed');
    assert.equal(partial.showClear, true);
    assert.equal(partial.showFinder, true);
    assert.equal(failure.tone, 'error');
    assert.equal(failure.primary, 'No images were converted.');
    assert.equal(failure.showClear, true);
    assert.equal(failure.showFinder, false);
});

test('builds compact preset library metadata', () => {
    assert.equal(
        buildPresetLibrarySummary({
            format: 'webp',
            resizeMode: 'width',
            width: 1200,
            height: null,
            quality: 90,
        }),
        'WEBP · Width 1200 px · Q90',
    );

    assert.equal(
        buildPresetLibrarySummary({
            format: 'avif',
            resizeMode: 'height',
            width: null,
            height: 1080,
            quality: 82,
        }),
        'AVIF · Height 1080 px · Q82',
    );
});

test('labels PNG presets as lossless instead of showing a quality value', () => {
    assert.equal(
        buildPresetLibrarySummary({
            format: 'png',
            resizeMode: 'none',
            width: null,
            height: null,
            quality: 100,
        }),
        'PNG · Original size · Lossless',
    );
});
