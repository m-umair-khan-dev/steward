# Translations

Cairn uses gettext (domain `cairn`). Strings in Rust go through `gettext()`,
strings in QML through `p.t()`.

| File | What it is |
|---|---|
| `cairn.pot` | Every message, extracted by `make i18n` (do not edit by hand) |
| `de.po`, `es.po`, `fr.po`, `ja.po`, `zh_CN.po` | **Drafts written by an AI model, not yet reviewed by native speakers.** Each says so in its header (`X-Review-Status`). Review before relying on them |
| `en@pseudo.po` | Generated pseudo-locale: longer, accented English, for finding cut-off text and strings that skip translation |
| `LINGUAS` | The languages that are built and installed |

To see the app in a language: `LANG=de_DE.UTF-8 LANGUAGE=de make app`.
To try the pseudo-locale: `LANG=en_US.UTF-8 LANGUAGE=en@pseudo`.

Some messages are sentence fragments joined in code ("Moved" + "3 items" +
"to the Trash"). Translate them so the pieces read naturally in that order;
a later release will turn them into whole sentences with placeholders.

E6-03 checks that every catalog translates every message and captures the
main screens in each language under `artifacts/i18n/<language>/`.
