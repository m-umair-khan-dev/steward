# Save to Cairn (browser extension)

Saves the page you are on to Cairn's Bookmarks (DESIGN.md P9-T04). Click the
toolbar button, or right-click a link and choose **Save Link to Cairn**. The
address goes to Cairn on this computer; nothing is sent anywhere else.

## How it works

A browser extension cannot start programs, so it talks to a small "native
messaging host" that the browser starts for it:

- `extension/`: the extension (Manifest V3; Firefox and Chromium browsers).
- `host/cairn-native-host`: the host. It reads one message (`{"url": …}`),
  accepts only `http` and `https` addresses, and runs
  `cairn add-link ADDRESS` (one argument, no shell).
- `host/dev.cairn.files.*.json`: tells each browser where the host is and
  which extension may use it.

The Cairn package installs the host and the Firefox manifest. The Chromium
manifest needs the extension's ID, which exists only once the extension is
published in the Chrome Web Store.

## Trying it locally (Firefox)

1. Install Cairn from its package (the host is in `/usr/libexec`).
2. Open `about:debugging#/runtime/this-firefox`, choose **Load Temporary
   Add-on…** and pick `extension/manifest.json`.
3. Open any web page and click the Cairn button. The link appears in
   Bookmarks; a ✓ on the button means it was saved.

## To publish (HUMAN)

- Sign the extension on addons.mozilla.org (ID `save@cairn.dev`).
- Publish in the Chrome Web Store, then put its ID in
  `host/dev.cairn.files.chromium.json` and package that file for
  `/etc/opt/chrome/native-messaging-hosts/` and
  `/etc/chromium/native-messaging-hosts/`.
