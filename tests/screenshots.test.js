import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
    buildFixtureData,
    clearMockupImages,
    FIXTURE_NAMES,
    SCREENSHOT_DIR,
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
            { filename: 'bulkpixel.webp', format: 'webp' },
            { filename: 'bulkpixel_empty.webp', format: 'webp' },
            { filename: 'presets.webp', format: 'webp' },
            { filename: 'watched_folders_directory.webp', format: 'webp' },
            { filename: 'statistics.webp', format: 'webp' },
            { filename: 'image_inspector.webp', format: 'webp' },
        ],
    );
    assert.equal(SCREENSHOT_DIR, path.resolve('src/assets/mockups'));
});

test('clears previous mockup images before generation while preserving other files', async t => {
    const directory = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'bulkpixel-mockups-test-'));
    t.after(() => fs.promises.rm(directory, { recursive: true, force: true }));
    for (const filename of ['old.png', 'old.webp', 'old.JPEG', 'old.svg', 'README.md']) {
        await fs.promises.writeFile(path.join(directory, filename), 'fixture');
    }

    await clearMockupImages(directory);

    assert.deepEqual(await fs.promises.readdir(directory), ['README.md']);
});
