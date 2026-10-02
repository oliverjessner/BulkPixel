#!/usr/bin/env node

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, '..');
const SCREENSHOT_DIR = path.join(SCRIPT_DIR, 'screenshots');
const VIEWPORT = { width: 1920, height: 1080 };
export const FIXTURE_NAMES = Object.freeze([
    'ayn_hor_akkutes.webp',
    'digimon_adventure_psp.png',
    'digimon_adventure_psp.png',
    'razer.png',
    'rentahuman.png',
]);
export const SCREENSHOT_SPECS = Object.freeze([
    { scenario: 'convert', filename: 'bulkpixel.png', format: 'png' },
    { scenario: 'empty', filename: 'bulkpixel_empty.png', format: 'png' },
    { scenario: 'presets', filename: 'presets.png', format: 'png' },
    {
        scenario: 'watched-folders',
        filename: 'watched_folders_directory.png',
        format: 'png',
    },
    { scenario: 'statistics', filename: 'statistics.png', format: 'png' },
    { scenario: 'inspector', filename: 'image_inspector.png', format: 'png' },
]);

let browserProcess;
let server;
let browserProfile;

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    await main().catch(async error => {
        console.error(`Screenshot generation failed: ${error.message}`);
        await cleanup();
        process.exitCode = 1;
    });
}

async function main() {
    await emptyScreenshotDirectory();

    const fixtures = buildFixtureData();
    server = await startStaticServer();
    const serverAddress = server.address();
    const appOrigin = `http://127.0.0.1:${serverAddress.port}`;

    const chromePath = findChromeExecutable();
    const debugPort = await reservePort();
    browserProfile = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'bulkpixel-screenshots-'));
    browserProcess = startChrome(chromePath, debugPort, browserProfile);

    const page = await waitForChromePage(debugPort, browserProcess);
    const cdp = await createCdpClient(page.webSocketDebuggerUrl);

    try {
        await cdp.send('Page.enable');
        await cdp.send('Runtime.enable');
        await cdp.send('Emulation.setDeviceMetricsOverride', {
            ...VIEWPORT,
            deviceScaleFactor: 1,
            mobile: false,
        });
        await cdp.send('Emulation.setEmulatedMedia', {
            features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
        });
        await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
            source: buildTauriMock(appOrigin, fixtures),
        });

        for (const spec of SCREENSHOT_SPECS) {
            await captureScenario(cdp, appOrigin, spec.scenario, spec.filename, spec.format);
        }

        console.log(`Created screenshots in ${SCREENSHOT_DIR}`);
    } finally {
        cdp.close();
        await cleanup();
    }
}

async function emptyScreenshotDirectory() {
    await fs.promises.rm(SCREENSHOT_DIR, { recursive: true, force: true });
    await fs.promises.mkdir(SCREENSHOT_DIR, { recursive: true });
}

export function buildFixtureData() {
    return FIXTURE_NAMES.map(name => {
        const fixturePath = path.join(ROOT_DIR, 'tests', 'test_images', name);
        const fileSize = fs.statSync(fixturePath).size;
        const extension = path.extname(name).slice(1).toLowerCase();
        return {
            path: fixturePath,
            name,
            fileType: extension === 'png' ? 'PNG' : 'WEBP',
            width: 1280,
            height: 720,
            fileSize,
        };
    });
}

function startStaticServer() {
    server = http.createServer(async (request, response) => {
        try {
            const requestUrl = new URL(request.url, 'http://127.0.0.1');
            const decodedPath = decodeURIComponent(requestUrl.pathname);
            const filePath = await resolveStaticPath(decodedPath);
            if (!filePath) {
                response.writeHead(403).end('Forbidden');
                return;
            }
            response.writeHead(200, {
                'Content-Type': mimeTypeFor(filePath),
                'Cache-Control': 'no-store',
            });
            fs.createReadStream(filePath).pipe(response);
        } catch {
            response.writeHead(404).end('Not found');
        }
    });

    return new Promise((resolve, reject) => {
        server.once('error', reject);
        server.listen(0, '127.0.0.1', () => resolve(server));
    });
}

async function resolveStaticPath(requestPath) {
    const candidates = [path.resolve(ROOT_DIR, `.${requestPath}`), path.resolve(ROOT_DIR, 'src', `.${requestPath}`)];
    const allowedRoots = [ROOT_DIR, path.join(ROOT_DIR, 'src')];

    for (const candidate of candidates) {
        const isAllowed = allowedRoots.some(
            allowedRoot => candidate === allowedRoot || candidate.startsWith(`${allowedRoot}${path.sep}`),
        );
        if (!isAllowed) {
            continue;
        }
        try {
            if ((await fs.promises.stat(candidate)).isFile()) {
                return candidate;
            }
        } catch {
            // Try the next static root.
        }
    }
    return null;
}

function mimeTypeFor(filePath) {
    switch (path.extname(filePath).toLowerCase()) {
        case '.html':
            return 'text/html; charset=utf-8';
        case '.js':
        case '.mjs':
            return 'text/javascript; charset=utf-8';
        case '.css':
            return 'text/css; charset=utf-8';
        case '.png':
            return 'image/png';
        case '.webp':
            return 'image/webp';
        case '.jpg':
        case '.jpeg':
            return 'image/jpeg';
        default:
            return 'application/octet-stream';
    }
}

function findChromeExecutable() {
    const candidates = [
        process.env.CHROME_PATH,
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/Applications/Chromium.app/Contents/MacOS/Chromium',
        '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary',
    ].filter(Boolean);
    const executable = candidates.find(candidate => fs.existsSync(candidate));
    if (!executable) {
        throw new Error('Chrome was not found. Set CHROME_PATH to a Chrome or Chromium executable.');
    }
    return executable;
}

function reservePort() {
    return new Promise((resolve, reject) => {
        const portServer = net.createServer();
        portServer.once('error', reject);
        portServer.listen(0, '127.0.0.1', () => {
            const address = portServer.address();
            portServer.close(error => (error ? reject(error) : resolve(address.port)));
        });
    });
}

function startChrome(executable, debugPort, profileDirectory) {
    const child = spawn(
        executable,
        [
            '--headless=new',
            '--disable-gpu',
            '--hide-scrollbars',
            '--no-first-run',
            '--no-default-browser-check',
            `--remote-debugging-port=${debugPort}`,
            `--user-data-dir=${profileDirectory}`,
            `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
            'about:blank',
        ],
        { stdio: ['ignore', 'pipe', 'pipe'] },
    );
    child.stderr.setEncoding('utf8');
    child.stdout.setEncoding('utf8');
    child.chromeOutput = '';
    for (const stream of [child.stdout, child.stderr]) {
        stream.on('data', chunk => {
            child.chromeOutput = `${child.chromeOutput}${chunk}`.slice(-8000);
        });
    }
    return child;
}

async function waitForChromePage(debugPort, child) {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
        if (child.exitCode !== null) {
            throw new Error(`Chrome exited early.\n${child.chromeOutput}`);
        }
        try {
            const pages = await fetch(`http://127.0.0.1:${debugPort}/json/list`).then(response => response.json());
            const page = pages.find(candidate => candidate.type === 'page');
            if (page) {
                return page;
            }
        } catch {
            // Chrome is still starting.
        }
        await delay(100);
    }
    throw new Error(`Chrome did not expose a page target.\n${child.chromeOutput}`);
}

async function createCdpClient(webSocketUrl) {
    if (typeof WebSocket === 'undefined') {
        throw new Error('This script requires Node.js 22 or newer for its built-in WebSocket client.');
    }

    const socket = new WebSocket(webSocketUrl);
    await new Promise((resolve, reject) => {
        socket.addEventListener('open', resolve, { once: true });
        socket.addEventListener('error', reject, { once: true });
    });

    let nextId = 1;
    const pending = new Map();
    socket.addEventListener('message', event => {
        const message = JSON.parse(event.data);
        const callback = pending.get(message.id);
        if (!callback) {
            return;
        }
        pending.delete(message.id);
        if (message.error) {
            callback.reject(new Error(message.error.message));
        } else {
            callback.resolve(message.result);
        }
    });

    return {
        send(method, params = {}) {
            const id = nextId++;
            socket.send(JSON.stringify({ id, method, params }));
            return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
        },
        close() {
            socket.close();
        },
    };
}

async function captureScenario(cdp, origin, scenario, filename, format = 'png') {
    console.log(`Creating ${filename}...`);
    await cdp.send('Page.navigate', {
        url: `${origin}/src/index.html?screenshot=${encodeURIComponent(scenario)}`,
    });
    await waitForExpression(cdp, `document.readyState === 'complete'`);
    await waitForScenario(cdp, scenario);
    await cdp.send('Runtime.evaluate', {
        expression: `(() => {
            const style = document.createElement('style');
            style.textContent = '* { animation: none !important; transition: none !important; caret-color: transparent !important; }';
            document.head.append(style);
        })()`,
    });
    await waitForExpression(cdp, `[...document.images].every(image => image.complete)`);
    await delay(120);

    const screenshot = await cdp.send('Page.captureScreenshot', {
        format,
        captureBeyondViewport: false,
        fromSurface: true,
    });
    await fs.promises.writeFile(path.join(SCREENSHOT_DIR, filename), Buffer.from(screenshot.data, 'base64'));
}

async function waitForScenario(cdp, scenario) {
    if (scenario === 'convert') {
        await waitForExpression(cdp, `document.querySelectorAll('.preview-card').length === 5`);
        return;
    }
    if (scenario === 'empty') {
        await waitForExpression(cdp, `!document.querySelector('#dropzone').hidden`);
        return;
    }
    if (scenario === 'presets') {
        await waitForExpression(cdp, `document.querySelectorAll('.preset-library-card').length >= 3`);
        await evaluate(cdp, `document.querySelector('[data-view="presets"]').click()`);
        await waitForExpression(cdp, `!document.querySelector('#presets-view').hidden`);
        return;
    }
    if (scenario === 'watched-folders') {
        await waitForExpression(cdp, `document.querySelectorAll('.magic-directory-card').length >= 2`);
        await evaluate(cdp, `document.querySelector('[data-view="magic"]').click()`);
        await waitForExpression(cdp, `!document.querySelector('#magic-view').hidden`);
        return;
    }
    if (scenario === 'statistics') {
        await waitForExpression(cdp, `document.querySelector('#statistics-trigger')`);
        await evaluate(cdp, `document.querySelector('#statistics-trigger').click()`);
        await waitForExpression(
            cdp,
            `document.querySelector('#statistics-dialog').open && !document.querySelector('#statistics-content').hidden`,
        );
        return;
    }
    if (scenario === 'inspector') {
        await waitForExpression(cdp, `document.querySelectorAll('.preview-card').length === 5`);
        await evaluate(cdp, `document.querySelector('.image-info-button').click()`);
        await waitForExpression(
            cdp,
            `!document.querySelector('#image-inspector').hidden && document.querySelectorAll('.image-inspector-section').length >= 4`,
        );
    }
}

async function waitForExpression(cdp, expression, timeoutMs = 10_000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (await evaluate(cdp, `Boolean(${expression})`)) {
            return;
        }
        await delay(50);
    }
    const pageState = await evaluate(
        cdp,
        `({
        url: window.location.href,
        title: document.title,
        text: document.body?.innerText?.slice(0, 1200) ?? '',
        cards: document.querySelectorAll('.preview-card').length,
        status: document.querySelector('#status-text')?.textContent ?? '',
    })`,
    );
    throw new Error(`Timed out waiting for: ${expression}\n${JSON.stringify(pageState, null, 2)}`);
}

async function evaluate(cdp, expression) {
    const response = await cdp.send('Runtime.evaluate', {
        expression,
        returnByValue: true,
        awaitPromise: true,
    });
    if (response.exceptionDetails) {
        throw new Error(response.exceptionDetails.text || `Browser evaluation failed: ${expression}`);
    }
    return response.result.value;
}

function buildTauriMock(origin, fixtures) {
    const loadedImages = fixtures.map(fixture => ({
        path: fixture.path,
        name: fixture.name,
        fileType: fixture.fileType,
        width: fixture.width,
        height: fixture.height,
        fileSize: fixture.fileSize,
        previewDataUrl: `${origin}/tests/test_images/${encodeURIComponent(fixture.name)}`,
    }));
    const uniquePaths = [...new Set(fixtures.map(fixture => fixture.path))];
    const presets = [
        {
            id: 1,
            name: 'Website WEBP',
            format: 'webp',
            resizeMode: 'width',
            width: 1600,
            height: null,
            quality: 86,
            filenameComponent: '-web',
            filenameMode: 'postfix',
            outputDirectory: '/Users/oli/Pictures/Exports',
        },
        {
            id: 2,
            name: 'Social PNG',
            format: 'png',
            resizeMode: 'width',
            width: 1080,
            height: null,
            quality: 100,
            filenameComponent: 'social-',
            filenameMode: 'prefix',
            outputDirectory: '/Users/oli/Pictures/Social',
        },
        {
            id: 3,
            name: 'Archive AVIF',
            format: 'avif',
            resizeMode: 'none',
            width: null,
            height: null,
            quality: 92,
            filenameComponent: '-archive',
            filenameMode: 'postfix',
            outputDirectory: '/Users/oli/Pictures/Archive',
        },
    ];
    const watchedFolders = [
        {
            id: 1,
            name: 'Incoming Photos',
            path: '/Users/oli/Pictures/Incoming',
            enabled: true,
            formats: ['jpeg', 'png', 'heic'],
            presetIds: [1, 3],
        },
        {
            id: 2,
            name: 'Design Exports',
            path: '/Users/oli/Desktop/Design Exports',
            enabled: false,
            formats: ['png', 'webp'],
            presetIds: [2],
        },
    ];
    const metadata = buildInspectorMetadata();

    return `
        window.__TAURI__ = {
            core: {
                invoke: async (command, args = {}) => {
                    const scenario = new URL(window.location.href).searchParams.get('screenshot');
                    if (command === 'get_app_version') return '3.0.0';
                    if (command === 'get_default_output_directory') return '/Users/oli/Downloads';
                    if (command === 'get_opened_files') {
                        return ['convert', 'inspector'].includes(scenario)
                            ? ${JSON.stringify(uniquePaths)}
                            : [];
                    }
                    if (command === 'probe_images_command') {
                        return { loaded: ${JSON.stringify(loadedImages)}, rejected: [] };
                    }
                    if (command === 'list_presets') return ${JSON.stringify(presets)};
                    if (command === 'list_magic_directories') return ${JSON.stringify(watchedFolders)};
                    if (command === 'inspect_image_metadata_command') return ${JSON.stringify(metadata)};
                    if (command === 'get_statistics') return ${JSON.stringify(buildStatistics())};
                    return null;
                },
            },
            dialog: { open: async () => null },
            event: { listen: async () => () => {} },
            webview: {
                getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }),
            },
        };
    `;
}

function buildInspectorMetadata() {
    const exifEntries = [
        entry('Exif.primary.Make', 'Make', 'Example Camera Co.', 'EXIF'),
        entry('Exif.primary.Model', 'Model', 'Sample Camera X1', 'EXIF'),
        entry('Exif.primary.LensModel', 'Lens Model', '24–70 mm f/2.8', 'EXIF'),
        entry('Exif.primary.DateTimeOriginal', 'Date Time Original', '2026:09:27 14:20:00', 'EXIF'),
        entry('Exif.primary.ExposureTime', 'Exposure Time', '1/120 s', 'EXIF'),
        entry('Exif.primary.FNumber', 'F Number', 'f/2.8', 'EXIF'),
        entry('Exif.primary.ISOSpeed', 'ISO', '80', 'EXIF'),
        entry('Exif.primary.GPSLatitude', 'GPS Latitude', '48 deg 12 min 0 sec N', 'EXIF'),
        entry('Exif.primary.GPSLongitude', 'GPS Longitude', '16 deg 22 min 0 sec E', 'EXIF'),
        entry('Exif.primary.BodySerialNumber', 'Body Serial Number', 'DEMO-1234', 'EXIF'),
    ];
    const iptcEntries = [entry('IPTC.2.116', 'Copyright', 'BulkPixel Demo Studio', 'IPTC')];
    const xmpEntries = [entry('XMP.CreatorTool', 'Creator Tool', 'BulkPixel Screenshot Fixture', 'XMP')];
    return {
        general: {
            format: 'WEBP',
            mimeType: 'image/webp',
            bitDepth: 8,
            channels: 3,
            animated: false,
            frameCount: null,
        },
        color: {
            colorModel: 'RGB',
            iccProfileChecked: true,
            iccProfileEmbedded: true,
            iccProfileName: 'Display P3',
            alpha: false,
        },
        exif: { status: 'present', entries: exifEntries, error: null },
        iptc: { status: 'present', entries: iptcEntries, error: null },
        xmp: { status: 'present', entries: xmpEntries, error: null },
        privacy: { gps: true, serialNumber: true, deviceModel: true, creator: true, software: true, timestamps: true },
        contentCredentials: { status: 'notChecked', summary: 'C2PA validation is not enabled.' },
        raw: [...exifEntries, ...iptcEntries, ...xmpEntries],
        warnings: [],
    };
}

function entry(key, label, value, group) {
    return { key, label, value, group };
}

function buildStatistics() {
    return {
        amount: 2847,
        cliUses: 412,
        uiUses: 2847 - 412 - 693,
        watchedFolderConversions: 693,
        webp: 1462,
        avif: 384,
        jpeg: 516,
        png: 485,
        inputBytes: 18_621_542_400,
        outputBytes: 7_298_875_392,
        processingTimeMs: 3_842_000,
        savedBytes: 11_322_667_008,
        createdAt: '2026-05-14 10:20:00',
        lastConversionAt: '2026-09-27 19:42:00',
    };
}

function delay(milliseconds) {
    return new Promise(resolve => setTimeout(resolve, milliseconds));
}

async function cleanup() {
    if (browserProcess && browserProcess.exitCode === null) {
        browserProcess.kill('SIGTERM');
        await Promise.race([new Promise(resolve => browserProcess.once('exit', resolve)), delay(2000)]);
    }
    if (server) {
        await new Promise(resolve => server.close(resolve));
        server = null;
    }
    if (browserProfile) {
        await fs.promises.rm(browserProfile, { recursive: true, force: true });
        browserProfile = null;
    }
}
