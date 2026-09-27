import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import {
    buildFixtureData,
    FIXTURE_NAMES,
    SCREENSHOT_SPECS,
} from '../scripts/screenshots.mjs';

test('uses the requested test images for the populated screenshots', () => {
    assert.deepEqual(FIXTURE_NAMES, [
        'ayn_hor_akkutes.webp',
        'digimon_adventure_psp.png',
        'digimon_adventure_psp.png',
        'razer.png',
        'rentahuman.png',
    ]);
    assert.equal(
        FIXTURE_NAMES.filter(name => name === 'digimon_adventure_psp.png').length,
        2,
    );

    const fixtures = buildFixtureData();
    assert.equal(fixtures.length, 5);
    for (const fixture of fixtures) {
        assert.equal(path.basename(path.dirname(fixture.path)), 'test_images');
        assert.ok(fs.statSync(fixture.path).size > 0);
        assert.ok(fixture.fileSize > 0);
    }
});

test('defines exactly the requested screenshot outputs', () => {
    assert.deepEqual(
        SCREENSHOT_SPECS.map(({ filename, format }) => ({ filename, format })),
        [
            { filename: 'bulkpixel.png', format: 'png' },
            { filename: 'bulkpixel_empty.png', format: 'png' },
            { filename: 'presets.png', format: 'png' },
            { filename: 'watched_folders_directory.png', format: 'png' },
            { filename: 'statistics.png', format: 'png' },
            { filename: 'image_inspector.png', format: 'png' },
        ],
    );
});
