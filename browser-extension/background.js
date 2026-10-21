export const HOST_NAME = 'com.oli.bulkpixel.browser';
export const MENU_ID = 'send-to-bulkpixel';

export function imageUrl(raw) {
    if (typeof raw !== 'string' || !raw || raw.length > 8192 || /[\u0000-\u0020]/u.test(raw)) {
        throw new Error('This image has no supported web address. Save it to your Mac and add the file to BulkPixel.');
    }
    let url;
    try { url = new URL(raw); }
    catch { throw new Error('This image has no supported web address. Save it to your Mac and add the file to BulkPixel.'); }
    if (!['http:', 'https:'].includes(url.protocol) || !url.hostname || url.username || url.password) {
        throw new Error('Only public HTTP/HTTPS images are supported. For blob, data, local, or signed-in images, save the image and add the file to BulkPixel.');
    }
    url.hash = '';
    return url.href;
}

export async function sendImage(api, raw) {
    const url = imageUrl(raw);
    let response;
    try { response = await api.runtime.sendNativeMessage(HOST_NAME, { action: 'import-image', url }); }
    catch { throw new Error('The browser is not connected to BulkPixel. Complete the setup below, then retry.'); }
    if (response?.ok !== true) {
        throw new Error(typeof response?.message === 'string' ? response.message : 'BulkPixel could not receive this image. Reconnect the browser and retry.');
    }
    return response;
}

export function registerExtension(api) {
    async function createMenu() {
        await api.contextMenus.removeAll();
        api.contextMenus.create({ id: MENU_ID, title: 'Send to BulkPixel', contexts: ['image'] });
    }
    function help(parameters = {}) {
        return api.tabs.create({ url: `${api.runtime.getURL('setup.html')}#${new URLSearchParams(parameters)}` });
    }
    api.runtime.onInstalled.addListener(details => {
        void createMenu();
        if (details.reason === 'install') void help();
    });
    api.runtime.onStartup.addListener(() => void createMenu());
    api.action.onClicked.addListener(() => void help());
    api.contextMenus.onClicked.addListener(info => {
        if (info.menuItemId !== MENU_ID) return;
        void sendImage(api, info.srcUrl).catch(error => {
            const parameters = { message: error.message };
            try { parameters.url = imageUrl(info.srcUrl); } catch { /* No retry for unsupported sources. */ }
            return help(parameters);
        });
    });
}

if (globalThis.chrome?.runtime?.id && !globalThis.document) registerExtension(globalThis.chrome);
