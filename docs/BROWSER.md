# Send to BulkPixel

The bundled Manifest V3 extension adds **Send to BulkPixel** to the image context menu in Google Chrome, Chromium, Brave, and Microsoft Edge on macOS. It sends the selected image's `srcUrl`, including signed query parameters, to BulkPixel. The app opens or comes forward, downloads the image into the existing queue, and shows the usual progress, cancel, retry, and error feedback. You choose conversion settings and export in the app.

## Setup

1. Build/install the BulkPixel app containing this feature in `/Applications/BulkPixel.app`.
2. Open `chrome://extensions` (Chrome/Chromium), `brave://extensions` (Brave), or `edge://extensions` (Edge), enable **Developer mode**, and choose **Load unpacked**.
3. Select `/Applications/BulkPixel.app/Contents/Resources/browser-extension`. In a source checkout, you can instead select the repository's `browser-extension` folder. Keep that folder in place while using the extension.
4. The extension opens its setup page with your actual extension ID and a copyable connection command. Select your browser and run that command in Terminal:

```sh
'/Applications/BulkPixel.app/Contents/MacOS/bulkpixel' browser install \
  --extension-id <32-letter-extension-id> --browser chrome
```

Use `chromium`, `brave`, or `edge` for the other browsers. Connect each browser separately, using the ID shown in that browser. This creates one per-user Native Messaging manifest allowing only that extension ID; it does not require administrator privileges. Custom browser profile roots and beta/dev/canary editions are not currently supported.

The extension is included in the app and source tree; it is not automatically installed or published to a browser store. Reconnect after moving the app or changing the extension ID. Replacing the app at the same location keeps the connection. Browsers may require developer mode to remain enabled for unpacked extensions.

## Usage and limits

Right-click an image and choose **Send to BulkPixel**. BulkPixel starts if needed, restores its window, and imports the image. Existing images and conversion settings are retained. Requests arriving during another import, conversion, or open dialog wait until it finishes/closes; duplicate queued URLs are combined. Each pending queue is limited to 100 URLs, with an explicit error when full.

Only the clicked image is imported. Extensionless image URLs are supported; HTML/login pages produce an image error instead of importing an entire website. The same image-only behavior is available in the terminal:

```sh
bulkpixel convert --url 'https://example.com/image?id=42' \
  --url-images-only --output-dir ./exports --format webp
```

Downloads use the existing native importer: public HTTP/HTTPS destinations only, DNS/address and redirect validation, no cookies or proxy, a 30-second source timeout, and the current **Add from URL** size setting (50 MB by default). Private-network, `file:`, `blob:`, and `data:` sources are unsupported. Images requiring browser authentication or hotlink headers may fail; save those images locally and add the files instead. The extension does not transmit browser cookies, page content, browsing history, or the referring page URL. Conversion runs locally. Downloads alone do not update conversion statistics.

If the connection is missing or the app cannot open, the extension opens its setup page with a retry button and an actionable error. A successful native handoff confirms only that the app was opened; download results appear inside BulkPixel. Click the extension toolbar icon to reopen setup at any time.

## Disconnect

```sh
bulkpixel browser uninstall --browser chrome
```

Remove the extension from the browser's extensions page separately. Disconnecting removes only BulkPixel's Native Messaging manifest for the selected browser; presets, statistics, images, and other browsers are unaffected.

## Implementation and validation

The extension uses only `contextMenus` and `nativeMessaging` permissions and has no content scripts or host permissions. The existing app binary also serves as the native host: Chromium passes its `chrome-extension://<id>/` origin as the first process argument, which selects a bounded, length-prefixed JSON protocol before normal CLI dispatch. Only the `import-image` action is accepted. The host validates the source and opens its own app bundle with a `bulkpixel://import?url=<encoded-image-url>` link. macOS delivers it through Tauri's existing `RunEvent::Opened` handler; the native queue retains requests until the frontend drains them, including startup and webview reloads. The deep-link handler validates the URL again, and downloads still validate every network destination. No local HTTP server or second database is created.

API references: [Chrome image context menus](https://developer.chrome.com/docs/extensions/reference/api/contextMenus), [Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging), [Tauri macOS Info.plist configuration](https://v2.tauri.app/reference/config/#macconfig).

Run the repository's Rust and JavaScript tests and macOS package checks from `AGENTS.md`. For native acceptance, install the newly built app and unpacked extension, connect the browser, and check: an image from a closed app; a second image with the app running/minimized; an image sent during conversion; an extensionless/signed image URL; an unsupported blob/data URL; an HTML/login response; and missing-host setup/retry. Verify that existing queue items and export settings survive and failed imports can be retried. Native Messaging tests use isolated temporary homes and do not connect your actual browser automatically.
