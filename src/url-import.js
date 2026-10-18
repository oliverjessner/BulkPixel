import { closeDialog, openDialog } from './vendor/oj-designsystem/index.js';

export function parseImageUrls(text) {
    const urls = [];
    const errors = [];
    const seen = new Set();
    let duplicates = 0;
    String(text).split(/\r?\n/).forEach((line, index) => {
        const raw = line.trim();
        if (!raw) return;
        try {
            if (raw.length > 8192 || /[\u0000-\u0020]/u.test(raw)) throw new Error('Enter one complete URL per line.');
            const url = new URL(raw);
            if (!['http:', 'https:'].includes(url.protocol) || !url.hostname) throw new Error('Only HTTP and HTTPS URLs are supported.');
            if (url.username || url.password) throw new Error('URLs containing usernames or passwords are not supported.');
            url.hash = '';
            if (seen.has(url.href)) duplicates += 1;
            else { seen.add(url.href); urls.push(url.href); }
        } catch (error) {
            errors.push(`Line ${index + 1}: ${error instanceof TypeError ? 'Enter a complete HTTP or HTTPS image or website URL.' : error.message}`);
        }
    });
    if (urls.length > 100) errors.push('Enter up to 100 image or website URLs at a time.');
    return { urls, errors, duplicates };
}

export function clipboardImageUrls(text) {
    const candidates = String(text).match(/https?:\/\/[^\s<>"']+/gi) ?? [];
    return parseImageUrls(candidates.join('\n')).urls.slice(0, 100);
}

export function isEditablePaste(event, activeElement) {
    const targets = [...(event.composedPath?.() ?? []), event.target, activeElement];
    return targets.some(target => target?.isContentEditable || target?.closest?.(
        'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"], [role="searchbox"], [role="combobox"]',
    ));
}

export function createUrlImporter({ root = document, invoke, eventApi, isBusy, onBusy, onImages, onStatus, loadedUrls }) {
    const dialog = root.querySelector('#url-import-dialog');
    const input = root.querySelector('#url-import-input');
    const sizeInput = root.querySelector('#url-import-max-size');
    const validation = root.querySelector('#url-import-validation');
    const results = root.querySelector('#url-import-results');
    const summary = root.querySelector('#url-import-summary');
    const submit = root.querySelector('#url-import-submit');
    const cancel = root.querySelector('#url-import-cancel');
    const retry = root.querySelector('#url-import-retry');
    const buttons = [...root.querySelectorAll('[data-add-from-url]')];
    const view = root.defaultView;
    let busy = false;
    let cancelRequested = false;
    let batchId = null;
    let items = [];
    let trigger;
    let unlisten;

    function updateValidation() {
        const parsed = parseImageUrls(input.value);
        const validSize = Number.isInteger(Number(sizeInput.value)) && Number(sizeInput.value) >= 1 && Number(sizeInput.value) <= 200;
        const notes = [...parsed.errors];
        if (parsed.duplicates) notes.push(`${parsed.duplicates} duplicate URL${parsed.duplicates === 1 ? '' : 's'} ignored.`);
        if (!validSize) notes.push('Maximum image size must be between 1 and 200 MB.');
        if (!notes.length && parsed.urls.length) notes.push(`${parsed.urls.length} URL${parsed.urls.length === 1 ? '' : 's'} ready to import. Websites import all discovered images.`);
        validation.textContent = notes.join(' ');
        validation.dataset.invalid = String(parsed.errors.length > 0 || !validSize);
        input.setAttribute('aria-invalid', String(parsed.errors.length > 0));
        submit.disabled = busy || !validSize || parsed.errors.length > 0 || !parsed.urls.length;
        input.disabled = busy;
        sizeInput.disabled = busy;
        cancel.disabled = cancelRequested;
        cancel.textContent = cancelRequested ? 'Canceling…' : busy || !items.length ? 'Cancel' : 'Done';
        submit.textContent = busy ? 'Importing…' : 'Import Images';
        retry.hidden = busy || !items.some(item => ['failed', 'canceled'].includes(item.status));
        return parsed;
    }

    function renderItem(item) {
        const row = root.createElement('li');
        row.className = 'url-import-result';
        row.dataset.status = item.status;
        const name = root.createElement('span');
        name.className = 'oj-caption url-import-result-name';
        // Keep signed query strings out of result labels and tooltips.
        try { const url = new URL(item.url); name.textContent = `${url.hostname}${url.pathname}`; }
        catch { name.textContent = `Image ${item.index + 1}`; }
        const status = root.createElement('span');
        status.className = 'oj-caption url-import-result-status';
        status.textContent = { waiting: 'Waiting', discovering: 'Finding images', discovered: 'Images found', downloading: 'Downloading', ready: 'Ready', failed: 'Failed', canceled: 'Canceled' }[item.status] ?? 'Waiting';
        row.append(name, status);
        const message = root.createElement('span');
        message.className = 'oj-helper url-import-result-message';
        if (item.status === 'downloading' && item.downloadedBytes) {
            const megabytes = (item.downloadedBytes / (1024 * 1024)).toFixed(1);
            message.textContent = `${megabytes} MB downloaded${item.totalBytes ? ` of ${(item.totalBytes / (1024 * 1024)).toFixed(1)} MB` : ''}`;
        } else message.textContent = item.message ?? '';
        row.append(message);
        return row;
    }

    function renderProgress() {
        results.hidden = !items.length;
        results.replaceChildren(...items.map(renderItem));
    }

    function receive(item) {
        if (!Number.isInteger(item.index) || item.index < 0 || item.index > items.length) return;
        const wasReady = items[item.index]?.status === 'ready';
        items[item.index] = item;
        if (!wasReady && item.status === 'ready' && item.image) onImages([item.image], item.url);
        const row = renderItem(item);
        if (results.children[item.index]) results.children[item.index].replaceWith(row);
        else results.append(row);
        results.hidden = false;
    }

    function open(text, source) {
        if (busy || isBusy()) return;
        trigger = source ?? root.activeElement;
        if (text !== undefined) input.value = text;
        items = [];
        results.replaceChildren();
        results.hidden = true;
        summary.hidden = true;
        cancelRequested = false;
        updateValidation();
        openDialog(dialog, { trigger });
        input.focus();
    }

    async function stop() {
        if (!busy) { closeDialog(dialog); return; }
        if (cancelRequested) return;
        cancelRequested = true;
        updateValidation();
        try { await invoke('cancel_url_import', { batchId }); }
        catch { summary.hidden = false; summary.textContent = 'Unable to cancel immediately. The current request will time out shortly.'; }
    }

    async function start(urls) {
        if (busy || isBusy()) return;
        const alreadyLoaded = loadedUrls();
        urls = urls.filter(url => !alreadyLoaded.has(url));
        if (!urls.length) { validation.textContent = 'Those URLs are already loaded.'; submit.disabled = true; return; }
        busy = true;
        cancelRequested = false;
        batchId = `url-${crypto.randomUUID()}`;
        items = urls.map((url, index) => ({ index, url, status: 'waiting', message: '' }));
        summary.hidden = true;
        onBusy(true);
        updateValidation();
        renderProgress();
        let closeWhenFinished = false;
        try {
            const response = { items: [] };
            // A website can produce more images than the input URL limit. Retry
            // those images in bounded batches while retaining one progress list.
            for (let offset = 0; offset < urls.length && !cancelRequested; offset += 100) {
                batchId = `url-${crypto.randomUUID()}`;
                const chunk = urls.slice(offset, offset + 100);
                const indices = new Map(chunk.map((_, index) => [index, offset + index]));
                const receiveChunk = item => {
                    if (!indices.has(item.index)) indices.set(item.index, items.length);
                    const mapped = { ...item, index: indices.get(item.index) };
                    receive(mapped);
                    return mapped;
                };
                unlisten = await eventApi.listen('url-import-progress', event => {
                    if (event.payload?.batchId === batchId) receiveChunk(event.payload);
                });
                const batch = await invoke('import_image_urls', {
                    batchId, urls: chunk, settings: { maxMegabytes: Number(sizeInput.value) },
                });
                response.items.push(...batch.items.map(receiveChunk));
                unlisten(); unlisten = null;
            }
            if (cancelRequested) {
                items.filter(item => item.status === 'waiting').forEach(item => {
                    const canceled = { ...item, status: 'canceled', message: 'Import canceled.' };
                    receive(canceled);
                    response.items.push(canceled);
                });
            }
            const ready = response.items.filter(item => item.status === 'ready').length;
            const failed = response.items.filter(item => item.status === 'failed').length;
            const canceled = response.items.filter(item => item.status === 'canceled').length;
            const message = `${ready} image${ready === 1 ? '' : 's'} imported${failed ? `, ${failed} failed` : ''}${canceled ? `, ${canceled} canceled` : ''}.`;
            summary.textContent = message;
            summary.hidden = false;
            onStatus(failed || canceled ? 'warning' : 'success', message);
            closeWhenFinished = cancelRequested || !failed && !canceled;
        } catch (error) {
            const message = typeof error === 'string' ? error : 'The image import could not be completed. Try again.';
            items = items.map(item => item.status === 'ready' ? item : { ...item, status: cancelRequested ? 'canceled' : 'failed', message });
            renderProgress();
            summary.textContent = message;
            summary.hidden = false;
            onStatus('error', message);
        } finally {
            unlisten?.(); unlisten = null;
            busy = false;
            const canceled = cancelRequested;
            cancelRequested = false;
            batchId = null;
            onBusy(false);
            updateValidation();
            if ((canceled || closeWhenFinished) && dialog.open) closeDialog(dialog);
        }
    }

    buttons.forEach(button => button.addEventListener('click', () => open(undefined, button)));
    input.addEventListener('input', () => { items = []; results.hidden = true; summary.hidden = true; updateValidation(); });
    sizeInput.addEventListener('input', updateValidation);
    submit.addEventListener('click', () => {
        const parsed = updateValidation();
        if (!submit.disabled) void start(parsed.urls);
    });
    cancel.addEventListener('click', () => void stop());
    retry.addEventListener('click', () => {
        const urls = items.filter(item => ['failed', 'canceled'].includes(item.status)).map(item => item.url);
        input.value = urls.join('\n');
        void start(urls);
    });
    dialog.addEventListener('cancel', event => { event.preventDefault(); void stop(); });
    dialog.addEventListener('close', () => { if (busy) void stop(); });
    function paste(event) {
        if (event.defaultPrevented || isEditablePaste(event, root.activeElement) || busy || isBusy()) return;
        const urls = clipboardImageUrls(event.clipboardData?.getData('text/plain') ?? '');
        if (!urls.length) return;
        event.preventDefault();
        open(urls.join('\n'));
    }
    root.addEventListener('paste', paste);
    view.addEventListener('pagehide', () => {
        root.removeEventListener('paste', paste);
        if (busy) void invoke('cancel_url_import', { batchId }).catch(() => {});
        unlisten?.();
    }, { once: true });
    return { open };
}
