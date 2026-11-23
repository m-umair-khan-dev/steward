// Save to Cairn (P9-T04): the toolbar button and a "Save Link to Cairn"
// menu item send an address to the native host, which runs
// `cairn add-link ADDRESS`. Nothing else is sent, and only on a click.
const api = globalThis.browser ?? globalThis.chrome;
const HOST = "dev.cairn.files";

function save(url) {
  if (!url || !/^https?:\/\//.test(url)) return;
  api.runtime.sendNativeMessage(HOST, { url }).then(
    (reply) => badge(reply && reply.ok ? "✓" : "!"),
    () => badge("!")
  );
}

function badge(text) {
  api.action.setBadgeText({ text });
  setTimeout(() => api.action.setBadgeText({ text: "" }), 2000);
}

api.action.onClicked.addListener((tab) => save(tab.url));

api.runtime.onInstalled.addListener(() => {
  api.contextMenus.create({ id: "save-link", title: "Save Link to Cairn", contexts: ["link"] });
});

api.contextMenus.onClicked.addListener((info) => {
  if (info.menuItemId === "save-link") save(info.linkUrl);
});
