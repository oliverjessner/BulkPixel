const { readFile } = require('node:fs/promises');
const { extractImages } = require('fetchary');

// Rust downloads and validates the page. Fetchary only parses that snapshot;
// neither scripts nor additional network requests are permitted here.
async function extractSnapshot({ url, path, contentType }) {
    const body = await readFile(path);
    if (body.length > 20 * 1024 * 1024) throw new Error('Website exceeds the 20 MB page limit.');
    const result = await extractImages(url, {
        mode: 'http', unique: true, cacheTtlMs: 0,
        maxResponseBytes: 20 * 1024 * 1024,
        // No DNS or network access: the only transport returns the native snapshot.
        allowPrivateNetwork: true,
        fetch: async requested => {
            if (requested !== url) throw new Error('Additional requests are disabled.');
            return new Response(body, { headers: { 'content-type': contentType || 'text/html' } });
        },
    });
    const urls = [...new Set(result.data.map(image => image.url))];
    if (urls.length > 10000) throw new Error('Website contains more than 10,000 image URLs.');
    return urls;
}

module.exports = { extractSnapshot };

if (require.main === module) {
    (async () => {
        let input = '';
        for await (const chunk of process.stdin) {
            input += chunk;
            if (input.length > 32768) throw new Error('Invalid extraction request.');
        }
        const urls = await extractSnapshot(JSON.parse(input));
        const output = JSON.stringify({ urls });
        if (Buffer.byteLength(output) > 16 * 1024 * 1024) throw new Error('Image URL list exceeds the 16 MB limit.');
        process.stdout.write(output);
    })().catch(error => {
        // Do not expose page content, signed URLs, or internal file paths.
        const message = /^(Website (?:exceeds|contains)|Image URL list exceeds)/.test(error.message)
            ? error.message : 'Fetchary could not extract images from this website.';
        process.stdout.write(JSON.stringify({ error: message }));
        process.exitCode = 1;
    });
}
