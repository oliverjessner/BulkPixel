import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildPrivacySummary,
    buildDeletePresetConfirmation,
    buildSummaryDeltaText,
    buildStatisticsTitle,
    filterMetadataEntries,
    formatAspectRatio,
    formatDate,
    formatDuration,
    formatMegapixels,
} from '../src/formatters.js';

test('builds the preset deletion prompt as display text', () => {
    const name = 'Demo"); DROP TABLE presets; --';

    assert.equal(
        buildDeletePresetConfirmation(name),
        'Delete preset "Demo"); DROP TABLE presets; --"?',
    );
});

test('shows the app version in the statistics title', () => {
    assert.equal(buildStatisticsTitle('2.2.0'), 'BulkPixel 2.2.0 Statistics');
    assert.equal(buildStatisticsTitle(''), 'BulkPixel Statistics');
});

test('formats statistics durations like the CLI', () => {
    assert.equal(formatDuration(0), '0sec');
    assert.equal(formatDuration(210_000), '3min 30sec');
});

test('formats statistics dates like the CLI', () => {
    assert.equal(formatDate('2026-07-03 12:34:56'), '03.07.2026');
    assert.equal(formatDate(''), '—');
});

test('describes conversion size changes without claiming larger outputs were saved', () => {
    assert.equal(buildSummaryDeltaText(2 * 1024 * 1024, 25), '2.0 MB saved (25.0%)');
    assert.equal(buildSummaryDeltaText(-2 * 1024 * 1024, -25), 'output is 2.0 MB larger (25.0%)');
});

test('formats common and non-standard image aspect ratios', () => {
    assert.equal(formatAspectRatio(4032, 3024), '4:3');
    assert.equal(formatAspectRatio(1920, 1080), '16:9');
    assert.equal(formatAspectRatio(6011, 4000), '1.50:1');
    assert.equal(formatAspectRatio(0, 0), 'Unknown');
});

test('formats image megapixels', () => {
    assert.equal(formatMegapixels(4032, 3024), '12.2 MP');
    assert.equal(formatMegapixels(undefined, 3024), 'Unknown');
});

test('filters metadata by readable label, raw key, and value', () => {
    const entries = [
        { label: 'Camera Model', key: 'Exif.primary.Model', value: 'Canon EOS R5' },
        { label: 'Creator Tool', key: 'XMP.CreatorTool', value: 'Adobe Photoshop' },
    ];

    assert.deepEqual(filterMetadataEntries(entries, 'camera'), [entries[0]]);
    assert.deepEqual(filterMetadataEntries(entries, 'CreatorTool'), [entries[1]]);
    assert.deepEqual(filterMetadataEntries(entries, 'adobe'), [entries[1]]);
    assert.deepEqual(filterMetadataEntries(entries, ''), entries);
});

test('builds cautious privacy findings only for detected metadata', () => {
    assert.deepEqual(buildPrivacySummary({ gps: true, software: true }), {
        findings: [
            { label: 'GPS location', warning: true },
            { label: 'Software information', warning: false },
        ],
        emptyText: '',
    });
    assert.equal(
        buildPrivacySummary({}).emptyText,
        'No obvious location or device identifiers detected.',
    );
});
