import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildDeletePresetConfirmation,
    buildSummaryDeltaText,
    buildStatisticsTitle,
    formatDate,
    formatDuration,
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
