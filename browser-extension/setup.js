import { sendImage } from './background.js';

const browser = document.querySelector('#browser');
const command = document.querySelector('#command');
const status = document.querySelector('#status');
const retry = document.querySelector('#retry');
const parameters = new URLSearchParams(location.hash.slice(1));
const url = parameters.get('url');
// Keep signed image addresses out of the address bar after reading the retry data.
history.replaceState(null, '', location.pathname);

if (/Edg\//.test(navigator.userAgent)) browser.value = 'edge';
else if (navigator.brave) browser.value = 'brave';

function updateCommand() {
    command.textContent = `'/Applications/BulkPixel.app/Contents/MacOS/bulkpixel' browser install --extension-id ${chrome.runtime.id} --browser ${browser.value}`;
}
function showStatus(message) { status.hidden = false; status.textContent = message; }
browser.addEventListener('change', updateCommand);
updateCommand();
if (parameters.has('message')) showStatus(parameters.get('message'));
retry.hidden = !url;
retry.addEventListener('click', async () => {
    retry.disabled = true;
    try { await sendImage(chrome, url); showStatus('Opening BulkPixel. Check the app for import progress and results.'); }
    catch (error) { showStatus(error.message); }
    finally { retry.disabled = false; }
});
document.querySelector('#copy').addEventListener('click', async () => {
    try { await navigator.clipboard.writeText(command.textContent); showStatus('Command copied. Run it in Terminal to connect your browser.'); }
    catch { showStatus('Select and copy the command above, then run it in Terminal.'); }
});
