import assert from 'node:assert/strict';
import test from 'node:test';

import {
    buildBrandTitle,
    buildDeletePresetConfirmation,
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

test('shows Magic Directory activity in the brand title only while active', () => {
    assert.equal(buildBrandTitle(true), 'BulkPixel - Magic Directory change detected');
    assert.equal(buildBrandTitle(false), 'BulkPixel');
});

test('formats statistics durations like the CLI', () => {
    assert.equal(formatDuration(0), '0sec');
    assert.equal(formatDuration(210_000), '3min 30sec');
});

test('formats statistics dates like the CLI', () => {
    assert.equal(formatDate('2026-07-03 12:34:56'), '03.07.2026');
    assert.equal(formatDate(''), '—');
});
