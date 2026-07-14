import assert from 'node:assert/strict';
import test from 'node:test';

import { buildBrandTitle, buildDeletePresetConfirmation } from '../src/formatters.js';

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
