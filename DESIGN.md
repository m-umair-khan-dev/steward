# Design and Technical Document: a FileOS-class file manager for Ubuntu

| | |
|---|---|
| **Status** | Draft v3: open source; Qt/QML front end (ADR 0009); look matched to the reference (ADR 0008) |
| **Date** | 15 November 2026 |
| **Reference product** | FileOS 1.0.3 for macOS, [fileosmac.com](https://fileosmac.com/) |
| **Reference capture** | [fileosmac/README.md](fileosmac/README.md), with recordings in [fileosmac/gifs/](fileosmac/gifs/) and stills in [fileosmac/screenshots/](fileosmac/screenshots/) |
| **Working codename** | **Steward** (placeholder; see [§1.3](#13-name-brand-and-what-we-may-not-copy)) |
| **Targets** | Ubuntu 24.04 LTS, Ubuntu 26.04 LTS and the current interim release, on GNOME with Wayland |
| **Licence** | Open source; GPL-3.0-or-later recommended ([§11.2](#112-open-decisions-for-the-product-owner)) |
| **Built by** | An AI coding agent (Claude Opus) phase by phase, with a human maintainer for the steps marked HUMAN ([§10](#10-implementation-plan-phases-and-end-to-end-tests)) |

**How to read this.** Sections 1 and 2 say what was analysed and what was decided. Sections 3 to 5 define the product (interface, features, assets). Sections 6 to 9 define how to build it (stack, backend, architecture, Docker). Section 10 is the implementation plan: ten phases, each with numbered tasks, end-to-end tests and a gate, written so an AI coding agent can execute it. Section 11 lists the risks and the decisions still open. Anything marked **[verify]** is a claim the team must confirm in Phase 0 before relying on it.

---

## Contents

1. [Summary and key decisions](#1-summary-and-key-decisions)
2. [Analysis of the original](#2-analysis-of-the-original)
3. [UI/UX design for Ubuntu](#3-uiux-design-for-ubuntu)
4. [Features: what to keep, adapt, replace and exclude](#4-features-what-to-keep-adapt-replace-and-exclude)
5. [Assets: mapping the scraped material](#5-assets-mapping-the-scraped-material)
6. [Frontend stack](#6-frontend-stack)
7. [Backend architecture](#7-backend-architecture)
8. [Technical architecture](#8-technical-architecture)
9. [Docker-based development and deployment](#9-docker-based-development-and-deployment)
10. [Implementation plan: phases and end-to-end tests](#10-implementation-plan-phases-and-end-to-end-tests)
11. [Risks and open decisions](#11-risks-and-open-decisions)
12. [Appendix A: keyboard map](#appendix-a-keyboard-map)
13. [Appendix B: what was inspected](#appendix-b-what-was-inspected)

---

## 1. Summary and key decisions

### 1.1 What we are building

FileOS is a **native desktop file manager**, with a small web presence around it: a marketing site, a 27-page guide, a changelog, a checkout and licence activation. It is not a web platform. The Ubuntu equivalent therefore has two parts:

- **The desktop app** (most of the work): a native GNOME file manager with the same signature features, namely one-click Downloads tidying, safe space recovery, project workspaces, dual pane, search, and developer tools.
- **The project infrastructure around it** (small): a website with a guide, and a signed apt repository for delivery and updates. The project is open source, so there is no checkout, no licence server and no accounts.

"Frontend" and "backend" in this document map onto that reality:

| Term in the brief | What it means here |
|---|---|
| Frontend | (a) the Qt/QML desktop UI (ADR 0009) and (b) the marketing and docs website |
| Backend | The local core engine and indexer inside the app. There is no server application; the only hosted pieces are static files (website, apt repository) |
| Database | Local SQLite files per user. No server database |
| Docker | Reproducible build, run and test environment for the desktop app; build and preview for the website and the apt repository |

A desktop file manager is not *deployed* in a container. End users install a `.deb`. Docker's job for the desktop app is to build it, run it during development against a safe fake home folder, test it headlessly, and package it.

### 1.2 Key decisions

| # | Decision | Why |
|---|---|---|
| D1 | **Qt 6 / QML front end on a Rust engine** (ADR 0009; originally GTK 4 + libadwaita) | The engine stays Rust on GIO/GVfs for trash, mounts, phones, shares and thumbnails. The front end moved to Qt 6 / QML through cxx-qt so the app can match the reference's look and motion (ADR 0008, ADR 0009). |
| D2 | **Develop on Ubuntu 26.10 (Qt 6.11); older releases at the end** (ADR 0007) | The interface uses Qt 6.5+ features (`QtQuick.Effects`, system colour scheme). 26.04 ships a current Qt; 24.04 ships Qt 6.4, so its support is decided once the project is complete: Qt5Compat effects, or a Flatpak with a current Qt (ADR 0009). |
| D3 | **Two processes: the app and a background indexer**, sharing a SQLite cache | Search and folder sizes must not block the window or die with it. A shared read-mostly database is the simplest contract between them. |
| D4 | **Every file action goes through one operations engine with an undo journal** | "Nothing moves without a preview, everything can be undone" is the original's core promise. It has to be an architectural property, not a per-feature courtesy. |
| D5 | **The app never runs anything as root.** System-level clean-ups are shown as a command to copy | The original does the same for packages ("FileOS only reads"). Root actions cannot go to the Trash, so they cannot honour the Undo promise. |
| D6 | **Ship as a `.deb` from our own signed apt repository**; Flatpak later with reduced features | A file manager needs the whole filesystem, host package tools and `/proc`. Sandboxed formats fight all three. An apt repo also gives updates for free. |
| D7 | **"Your iPhone" becomes "Your phone"**: Android first, iPhone best-effort | Android over MTP is the common case on Ubuntu and is well supported by GVfs. iPhone support on Linux depends on libimobiledevice keeping up with iOS. |
| D8 | **The Drop Shelf is redesigned, not ported** | A Wayland app cannot place itself at the top of the screen, grab a global key on older GNOME, or see a drag that starts in another app. The Ubuntu version is a summonable Shelf window, with an optional GNOME Shell extension for placement. |
| D9 | **No server application.** Static website, static signed apt repository, no accounts, no telemetry | The project is open source, so the reference's checkout and licence activation are dropped. The reference itself runs no backend of its own, so nothing is lost |
| D10 | **Website content lives in Git as Markdown** (Astro static site). No CMS | The content is a landing page, about 27 guide pages and a changelog, all written by the team. A CMS adds a service to run and secure for no gain. |
| D11 | **Own name, icon, words and artwork; the look follows the reference** (ADR 0008) | Layout, colours, shapes and motion match the captures in `fileosmac/`; every image, icon and string is our own. See below. |
| D12 | **End-to-end tests drive the real app in a headless container**, and every phase ends at a gate that runs all of them | An AI agent can only be trusted with a file manager if it can prove, by itself, that files end up where they should. Details in [§10.2](#102-test-layers-and-the-end-to-end-harness) |

### 1.3 Name, brand and what we may not copy

"FileOS", its icon, its purple brand, its site copy, its demo animations and its customer testimonials belong to its maker. The Ubuntu product takes the *ideas* (which are not protected) and must supply its own *expression*:

- A new name and icon. "Steward" is a placeholder; pick a name, check it against existing trademarks, and register a domain so the app ID can be proper reverse-DNS (this document uses `io.github.m_umair_khan_dev.steward`).
- Do not use "Ubuntu" in the product name or logo. That is Canonical's trademark. "for Ubuntu" as a description is fine.
- The six quoted X users praised FileOS for Mac. Reusing their words or avatars for another product would be a fabricated endorsement. They are excluded outright.
- Everything in [fileosmac/](fileosmac/) is **reference material for the team**, never shipped. [§5](#5-assets-mapping-the-scraped-material) maps each item to what replaces it.

---

## 2. Analysis of the original

### 2.1 Product surface

Inventory from the landing page, the guide, the changelog, the privacy page and the page source ([Appendix B](#appendix-b-what-was-inspected)).

| Surface | What the original has | Observed implementation |
|---|---|---|
| App | Native macOS app, SwiftUI, macOS 15+ | Single window with sidebar; Settings with 9 pages; menu bar icon; global Drop Shelf |
| Site | One long landing page, light and dark | Hand-built static HTML (980 KB), one CSS file (189 KB), one JS file. No framework. Every "video" is an animated HTML/SVG mock of the app |
| Guide | 27 pages under `/docs/` | Static pages with drawn mock-ups of the app and a ⌘K page search |
| Changelog | `/changelog/`, per-version notes | Static |
| Checkout | One Mac $11.99, two Macs $23 (launch price) | Hosted checkout at Dodo Payments, redirect to `/thanks/` which shows the key and an "Activate in FileOS" deep link |
| Licensing | Key by email; activate, deactivate, one or two Macs | The app talks **directly to Dodo Payments**. It re-checks once a day; being offline never stops the app |
| Updates | In-app, daily check, asks before installing | Signed updates served from the site |
| Analytics | Cookieless page counts | Self-run collector on a separate host, 30-minute visit number |
| Email | Launch waitlist | Resend |
| Fonts | Instrument Sans for display, system font for body | Google Fonts |
| Languages | English, Spanish, French, German, Japanese, Simplified Chinese | Follows the system language |

**Finding:** the original runs with *no application backend of its own*. Its server side is static hosting plus third parties. The Ubuntu project follows suit and runs none either.

### 2.2 What makes it good (to preserve)

These are the qualities that make the product worth cloning. Each becomes a testable requirement.

1. **Preview before change.** Tidy and Smart Clean show the exact plan first.
2. **Undo everywhere.** Moves, copies, renames and trashing all undo. Deletion always goes via the Trash. Emptying the Trash always asks.
3. **Honest suggestions.** Every clean-up item says *why* it is listed. Items are pre-ticked only when clearly spare. Anything uncertain starts unticked.
4. **Read-only by default.** Git status never fetches. Package lists never remove. Opening a tool moves nothing.
5. **Local only.** Files and file names never leave the machine.
6. **Problem-first framing.** Every feature is introduced by the annoyance it removes ("Five PDFs called Invoice-final. Which one?").
7. **Sizes and previews on everything.** Folders show size and item count before you open them. Files show real content previews.
8. **Works beside the system file manager.** Same folders, compatible copy and paste. It does not demand to replace Finder.
9. **Plain language.** Short sentences, no jargon ("Your call", "Safe to clear", "Look Again").

### 2.3 Observed design language

From `styles.css` and the captures. These describe the *website and its mock-ups*; they are inputs, not a specification for the Ubuntu app.

| Token | Light | Dark | Used for |
|---|---|---|---|
| Brand | `#6B56DC` | `#A897F3` | Buttons, selection, folder tint, git chip |
| Brand pale | `#C9BEFF` | `#4E3BBE` | Hover, soft fills |
| Background | `#F7F6FB` | `#0C0B14` | Page |
| Ink | `#16132A` | `#EEECF7` | Text |
| Muted | `#5D5A72` | `#A6A2BC` | Secondary text |
| Line | ink at 8% | white at 7% | Hairlines |
| Error | `#C0362C` | `#FF9B8F` | Destructive |
| Easing | `cubic-bezier(0.16, 1, 0.3, 1)` | | All motion |

Layout patterns seen across every capture:

- Three-zone window: sectioned sidebar, content, optional preview pane.
- Sidebar sections in fixed order: Workspaces, Bookmarks, Favorites, Tools, Locations. The computer and its free space sit at the bottom.
- Content header with a large title, a one-line summary ("3 folders, 6 files · 20.8 GB · changed 2 hr ago") and filter chips with counts.
- Cards for files (preview on top, name and "kind · size" below) and tinted "glass" folders carrying a symbol, an item count and a size.
- A bottom status line ("21 items · 147.8 GB available").
- Dark toast at the bottom centre for results, always with **Undo**.
- A sticky action bar at the bottom when items are picked ("3 picked · Copy to Holiday 2026 ▾ · Copy").

---

## 3. UI/UX design for Ubuntu

The interface follows the reference captures in [fileosmac/](fileosmac/) closely: layout, proportions, colours, card and folder shapes, motion (ADR 0008). It is built with Qt 6 / QML (ADR 0009) from **our own drawing, icons and words**: nothing is copied from the reference (§1.3). Where Ubuntu conventions differ from macOS (window buttons, keys, file locations), Ubuntu wins.

**Primary references** (open these when building any screen):

| Capture | What to match |
|---|---|
| [screenshots/smart-downloads.png](fileosmac/screenshots/smart-downloads.png) | The whole window: sidebar, header, breadcrumb row, Folders and "By kind" sections, glass folders, right pane, status bar |
| [screenshots/design.png](fileosmac/screenshots/design.png) | File cards, glass folders with counts and sizes, folder-at-a-glance tree |
| [gifs/hero.gif](fileosmac/gifs/hero.gif) | The tour, and the signature motion: files flying into kind folders |
| [gifs/smart-clean.gif](fileosmac/gifs/smart-clean.gif) | Card grid, detail page push, rows collapsing and reflowing, ring animation, Undo toast |
| [gifs/dual-pane.gif](fileosmac/gifs/dual-pane.gif) | Two panes, rows flying across, footer hint bar |
| [gifs/search.gif](fileosmac/gifs/search.gif) | Search overlay, chips with counts, matched-line rows |
| [gifs/iphone.gif](fileosmac/gifs/iphone.gif), [gifs/workspaces.gif](fileosmac/gifs/workspaces.gif), [gifs/developers.gif](fileosmac/gifs/developers.gif), [gifs/peek.gif](fileosmac/gifs/peek.gif), [gifs/bookmarks.gif](fileosmac/gifs/bookmarks.gif), [gifs/about-this-mac.gif](fileosmac/gifs/about-this-mac.gif), [screenshots/ai-agents.png](fileosmac/screenshots/ai-agents.png), [screenshots/installed.png](fileosmac/screenshots/installed.png) | Each later feature's screen |

### 3.1 Principles

1. **Look like the reference, behave like an Ubuntu app.** Same layout, shapes, colours and motion; Ubuntu window buttons, Ctrl-based keys, XDG folders, Trash and drives.
2. **Motion explains change.** Every change to the file system or the view animates from where things were to where they go (files into kind folders, rows across panes, rows collapsing). Never a bare jump.
3. **Calm by default.** Soft lavender surfaces, one accent, generous spacing, short words.
4. **Respect the system where it matters to people:** light or dark, text size, reduced motion, screen readers, keyboard-only use.
5. **No fake macOS.** No traffic lights, no notch, no Apple names or symbols.

### 3.2 From macOS idiom to Ubuntu

| Reference (macOS) | Steward on Ubuntu |
|---|---|
| Traffic lights, top left | Our own window buttons (minimise, maximise, close) at the right of the header, round and quiet. The window is frameless; we draw the whole chrome so it matches (GNOME expects client-side decorations) |
| Sidebar as an inset rounded panel | Same |
| App name row ("FileOS" + icon) | "Steward" + our own app badge |
| Sections Workspaces, Bookmarks, Favorites, Tools, Locations | Same order. Favorites holds Home and the XDG folders; Locations holds drives, phones and network shares. Sections appear as their features land (Workspaces Phase 4, Bookmarks Phase 7, Tools Phases 5 and 7) |
| "MacBook Air · 147.8 GB free" with a bar | "Computer · 147.8 GB free" with a bar; opens About This Computer (Phase 5) |
| ⌘ shortcuts | Ctrl shortcuts ([Appendix A](#appendix-a-keyboard-map)) |
| Quick Look | Peek (Space), our own overlay |
| "Show in Finder" | "Show in Files" |
| Spotlight | GNOME Shell search provider |
| Notch Drop Shelf | Shelf window ([§4.4](#44-redesigned-the-shelf)) |
| Menu bar extra | Tray icon (AppIndicator) |
| macOS system blur behind windows | Not possible on GNOME; blur is used inside the window (glass folder fronts, overlays) where QML can do it |

### 3.3 Window anatomy

Matches [smart-downloads.png](fileosmac/screenshots/smart-downloads.png):

```
╭──────────────╮  ╭─────────────────────────────────────────────────────────────────────────────╮
│ ▣ Steward    ⇤ │  │ (‹ ›)  Downloads        ( None│Kind│Date ) (☰│▦) (▤│▥│◫) ( Filter… ) ⋯  – ▢ ✕ │ header
│              │  │ Computer › home › sam › Downloads                         Ctrl+L to type a path│ crumbs
│ Workspaces   │  ├───────────────────────────────────────────────────────┬─────────────────────┤
│  ▤ Launch    │  │ [All 29] [Folders 3] [Images 6] [Videos 2]   3 folders…│ Downloads           │
│ Bookmarks    │  │                                                       │ 1.87 GB  in all     │
│  ☆ All    18 │  │ Folders 3                                             │ 3 folders · 26 files│
│ Favorites    │  │  ╭▔▔╮____  ╭▔▔╮____  ╭▔▔╮____                         │                     │
│  ⌂ Home      │  │  │5 items│  │3 items│  │12 items                       │ WHAT TAKES THE SPACE│
│ ▸ Downloads  │  │  │  ▤    │  │  ☆    │  │  ✦    │                       │ ● Videos     1.6 GB │
│  ▤ Documents │  │  ╰745 KB╯  ╰5.4 MB╯  ╰48 MB ╯                         │ ▬▬▬▬▬▬▬▬▬▬▬▬▬▬▬     │
│ Tools        │  │  archive   Receipts  Wallpapers                       │ ● Installers 188 MB │
│  ⌁ Free Up…  │  │                                                       │ ▬▬                  │
│ Locations  › │  │ By kind 6                                             │ SUGGESTIONS         │
│              │  │  (teal Videos) (orange Images) (red PDFs) …           │ ┌─────────────────┐ │
│ ▭ Computer   │  │                                                       │ │1 installer·186MB│ │
│ ▬▬▬ 147.8 GB │  │                                                       │ └─────────────────┘ │
╰──────────────╯  │ 21 items · 147.8 GB available                         │ [ Tidy Downloads… ] │
                  ╰─────────────────────────────────────────────────────────────────────────────╯
```

| Element | Specification |
|---|---|
| Window | Lavender background (`bg`). Frameless; 8 px gutter around the sidebar panel; the content pane is white. Drag by any empty header area (system move); resize from the edges (system resize). Remembers size and maximised state |
| Sidebar | Inset rounded panel, 210–240 px, radius 16, `sidebar` colour, 1 px `line` border. App row (badge + "Steward", 19 px bold) with a collapse button. Section headings 12 px semibold, 50 % ink. Rows 34 px, radius 10, purple line icons, label, optional count on the right in `faint`. Selected row: accent at 16 % with bold label. Computer row pinned at the bottom with a thin purple usage bar and "N GB free" |
| Collapsed sidebar | A 56 px icon rail (app badge, one icon per entry, computer icon). Hovering the rail slides the full sidebar over the content (spring, 250 ms); leaving it slides back. The collapse button and F9 toggle |
| Header | 56 px. Left: back/forward in a white pill with a soft shadow; folder title 20 px bold. Right: grouping segmented control (None, Kind, Date); view switch (list, grid); pane toggles (preview pane; later the space chart and dual pane) where active buttons are filled with the accent; "Filter this folder" pill field; ⋯ menu; window buttons |
| Breadcrumb row | Full path from the disk root ("Computer › home › sam › Downloads"), muted segments, current segment bold; "Ctrl+L to type a path" hint on the right. Ctrl+L turns the row into a text field with Tab completion |
| Chip row | "All N" and one chip per kind present, with counts; active chip filled with accent at 16 % and accent text. Folder summary on the right ("3 folders, 26 files · 1.87 GB") |
| Content | Sections, each with a header: bold title plus a `faint` count ("Folders 3", "Files 26", "By kind 6"). Folders as glass folders; files as file cards. List view: a table with Name, Size, Kind, Modified, Created and section headers when grouping |
| Right pane | 300 px, `pane` colour, left border. Nothing selected: the folder overview (name, total size "in all", counts, WHAT TAKES THE SPACE with coloured dots and bars, later SUGGESTIONS and the purple Tidy button). One item selected: its preview and facts (Kind, Size, Modified, Created, Where) and Open. A folder: what is inside (two levels once the index exists) |
| Status bar | "21 items, 1 selected · 147.8 GB available" in `muted`, 12 px |
| Toasts | Dark pill at the bottom centre ("Moved 2 items to …"), with Undo. Slides up with a spring, leaves after 3 s |
| Menus, popovers, dialogs | White, radius 12, soft shadow, 13–14 px text; dialogs centred with a dimmed backdrop |

### 3.4 Components

**Glass folder** (matches the folders in [smart-downloads.png](fileosmac/screenshots/smart-downloads.png) and [design.png](fileosmac/screenshots/design.png)):

- Shape about 200 × 165 at Medium (scales with Small, Medium, Large).
- Back: the tint colour with a tab at the top left. Behind the front, two sheets of white paper, slightly rotated.
- Front: a frosted panel covering the lower 65 %. A real blur of what is behind it, a tint gradient (light at the top left to the tint colour), a 1 px white border at 40 %, and an inner highlight.
- On the front: "N items" (bold, top left), an info button (top right, opens the preview pane), a symbol in the centre (from the folder-symbol rules, §4.1), and the size (bottom left, once the index knows it).
- Name underneath, centred.
- Tints: folders purple; kind folders by kind (Videos teal, Images orange, PDFs red, Documents blue, Installers slate, Archives green, Audio pink, Code indigo, Other grey).
- Hover: lifts 4 px, scales to 1.04, deeper coloured shadow (spring). Selected: accent ring around the card.

**File card** (matches the file cards in [design.png](fileosmac/screenshots/design.png)):

- About 300 × 260 at Medium.
- A preview area on a light grey background showing the real thumbnail (first page, photo, video frame, slide). Without one, a large type icon on a tinted gradient.
- Underneath: a small document glyph with a coloured type label ("PDF", "JPG", "KEY"), the name, then "PDF · 4.2 MB", and a ⋯ button for the item menu.
- Hover: lift, purple border. Selected: 2 px accent border.

**Segmented control:** pill track at 5 % ink; the selected segment is a white pill with a shadow that **slides** to the new option (spring).

**Rows** (lists in the right pane, Smart Clean, Installed): white rounded rows, coloured dot, title and subtitle, value on the right.

### 3.5 Motion

Taken from measuring the GIFs (the drop-shelf capture recorded nothing and needs re-recording before Phase 8):

| Motion | Where | Specification |
|---|---|---|
| Files into kind folders | Group by Kind (hero tour) | Each card shrinks to about 20 % and flies to its kind folder's centre (560 ms, in-out cubic, staggered about 22 ms per card); kind folders grow in from 55 % with a spring once the first cards land. Reverse when ungrouping |
| Page push | Opening a Smart Clean card, settings pages | Slide in from the right with a fade, 300 ms |
| Collapse and reflow | Removing rows or cards (Move to Trash, Tidy) | Leaving item fades and slides out (240 ms); the rest move up with a spring |
| Fly across | Dual pane F5/F6, Shelf drops | Ghost rows travel from source to target (about 1 s), then the target list inserts them |
| Ring | Disk usage | Arc animates to its new value (600 ms) |
| Toast | After any file action | Spring up from below; Undo reverses with the opposite motion |
| Hover | Folders, cards, sidebar rows, buttons | Lift and shadow (spring, about 180 ms) |
| Segmented and chips | Header, chip row | Selection pill slides (spring) |
| Sidebar hover | Collapsed rail | Sidebar slides over the content (spring, 250 ms) |

All motion is off when the system asks for reduced motion, and in end-to-end tests unless a test is about motion.

### 3.6 Colour and type tokens

Defined once in `qml/Theme.qml` (values derived from the reference site's palette, [§2.3](#23-observed-design-language)):

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg` | `#f4f3f9` | `#0c0b14` | Window background |
| `pane` | `#ffffff` | `#15141f` | Content pane, header |
| `sidebar` | `#ecebf4` | `#12111b` | Sidebar panel |
| `card` | `#ffffff` | `#1e1d2b` | File cards, rows, menus |
| `ink` | `#16132a` | `#eeecf7` | Text |
| `muted` | `#5d5a72` | `#a6a2bc` | Secondary text |
| `faint` | `#8a879a` | `#6f6b85` | Counts, hints |
| `line` | ink at 8 % | white at 7 % | Borders |
| `accent` | `#6b56dc` | `#7b66e6` (text `#a897f3`) | Selection, active controls, buttons |
| tints | see glass folder | same | Kind colours |

Type: the system font (Ubuntu Sans) at 13–14 px body, 20–22 px titles, semibold headings. Preferences offer "Use system accent colour" (from the desktop portal) instead of the purple.

### 3.7 Screens

| Screen | Reference | Notes |
|---|---|---|
| Folder (grid, list, grouped) | smart-downloads.png, design.png | §3.3, §3.4 |
| Downloads with Tidy | smart-downloads.gif, hero.gif | Phase 4. Right pane gains SUGGESTIONS and "Tidy Downloads…"; the plan dialog lists moves before anything happens |
| Search overlay | search.gif | Phase 3. Centred panel, chips with counts, matched line with the word in bold, key hints in the footer |
| Free Up Space | smart-clean.gif | Phase 5. Ring, cards in two groups, page push into a card, collapse and reflow, sticky "Move N to Trash" bar |
| Workspace page | workspaces.gif | Phase 4 |
| Dual pane | dual-pane.gif | Phase 4. Active pane has an accent top edge; footer hint "Copy to … F5 · Move F6 · Tab switches pane" |
| Phone | iphone.gif | Phase 8. Day sections, chips, picked bar, "On this computer" badge |
| Peek | peek.gif | Phase 3. Overlay with toolbar (Open, Open With, Show in Folder, Info) |
| Bookmarks, About This Computer, AI Agents, Installed | bookmarks.gif, about-this-mac.gif, ai-agents.png, installed.png | Phases 5 and 7 |
| Welcome | hero.gif opening ("a messy folder, then sorted") | Phase 6 |
| Preferences, About, Shortcuts, Open With | – | QML dialogs in the same style |

### 3.8 Accessibility

- Every interactive element has an accessible name and role (`Accessible.name`, `Accessible.role`) and a press action. Containers that tests or screen readers need to find (Sidebar, Main toolbar, Files, Preview) are named groups.
- Full keyboard reach: arrows in the grid, Tab between areas, every command on a key ([Appendix A](#appendix-a-keyboard-map)).
- No meaning by colour alone. Contrast checked in light and dark. Works at 200 % text scale.
- Reduced motion respected (§3.5).
- Tested with Orca on a real desktop (HUMAN) and through AT-SPI in every end-to-end run.

### 3.9 Languages

Same six as the reference at launch. QML strings use `qsTr()` and Rust strings gettext; Phase 6 wires both to one translation workflow. Layouts must survive German string lengths and CJK glyph heights.

## 4. Features: what to keep, adapt, replace and exclude

**Verdicts.** *Keep*: same behaviour, native implementation. *Adapt*: same goal, different mechanism or scope. *Replace*: the macOS concept has no meaning here; an Ubuntu concept takes its place. *Exclude*: dropped. **Priority**: P1 needed for 1.0, P2 soon after, P3 later.

### 4.1 Core file management

| Feature (reference) | Verdict | Ubuntu implementation | Pri |
|---|---|---|---|
| Browse folders, icon and list views, three sizes | Keep | QML: grid as a `ListView` of section headers and card rows (glass folders, file cards); list as a table; both virtualised. Layout computed in the Rust controller | P1 |
| Column (Miller) view, Gallery view | Exclude for now | Not a GNOME convention, high cost. Revisit on demand | P3 |
| Path bar, type a path or a folder name, Tab completes | Keep | Breadcrumb plus entry on `Ctrl+L`; name lookup uses the index | P1 |
| Copy, cut, paste compatible with the system file manager | Keep | Clipboard formats `text/uri-list` and `x-special/gnome-copied-files`, so it works with Files (Nautilus) both ways | P1 |
| Move by keyboard with cut and paste | Keep | Already the Linux convention | P1 |
| Rename in place | Adapt | `F2` (Ubuntu convention), not Return. Return opens | P1 |
| Trash, Undo, Empty Trash asks | Keep | GIO trash (FreeDesktop spec). Undo restores from Trash by original path | P1 |
| Long copies queue with progress and Cancel; clash dialog | Keep | Operations engine ([§7.1](#71-local-backend-inside-the-desktop-app)) | P1 |
| Folder sizes on every folder, computed in the background | Keep | Indexer computes and caches; invalidated by file monitors | P1 |
| Real previews on cards | Keep | FreeDesktop thumbnail cache, shared with Files; system thumbnailers run sandboxed | P1 |
| Folder at a glance (preview pane tree) | Keep | From the index, two levels | P1 |
| Group by Kind or Date added | Adapt | Linux has no "date added". Use file birth time where the filesystem records it, else modified time, and label the column **Created** | P1 |
| Hidden files | Keep | `Ctrl+H` | P1 |
| Live refresh | Keep | `GFileMonitor` on open folders | P1 |
| Sidebar you arrange; missing favourite shows a question mark and **Locate** | Keep | Stored in the app database | P1 |
| iCloud Drive, Dropbox, external drives in the sidebar | Adapt | Whatever GVfs mounts: external drives, SMB/SFTP/WebDAV shares, and GNOME Online Accounts drives (Google Drive, Nextcloud, OneDrive on newer releases **[verify]**). Dropbox appears as its normal synced folder | P1 |
| Finder tags | Exclude | No system-wide equivalent that other apps honour | – |
| AirDrop, Recents, Shared in sidebar | Exclude | AirDrop is Apple-only. "Recent" may come later from the XDG recently-used list | – |
| Folder symbols (automatic by name, project type or "mostly PDFs"; or chosen) and per-folder colour, saved on the folder | Keep | Same rule order. Stored as user extended attributes (`user.steward.symbol`, `user.steward.color`) so they survive rename and move; database fallback on filesystems without xattrs (FAT, exFAT, some network shares) | P2 |
| Light, Dark or as system | Keep | `Theme.qml` light and dark palettes; "system" follows the desktop through Qt's style hints | P1 |
| Open In (terminal, editor), add your own app | Adapt | Detect installed apps from desktop files. Terminals: the system default (via `xdg-terminal-exec` where present, else `x-terminal-emulator`), GNOME Terminal, Ptyxis, Ghostty, Kitty, Alacritty, WezTerm. Editors: VS Code, Cursor, Zed, Sublime Text, JetBrains IDEs, GNOME Text Editor, GNOME Builder. Xcode, Nova, BBEdit and iTerm are Mac-only and dropped | P2 |
| Choose which folder is Downloads | Keep | Default from `xdg-user-dirs` | P1 |
| Six languages | Keep | gettext | P1 |
| Check for updates in-app | Replace | Updates arrive through apt with the rest of the system. The app only shows "What's New" after an update | P1 |

### 4.2 Signature features

| Feature | Verdict | Ubuntu implementation | Pri |
|---|---|---|---|
| **Smart Downloads: Tidy** (loose files into kind folders, plan first, Undo; skips files still downloading or added in the last 2 minutes; never moves folders) | Keep | Kind from MIME type. "Still downloading" means `.part`, `.crdownload`, `.download`, `.partial`, or modified within 2 minutes. One journal batch, so one Undo puts everything back | P1 |
| Group by kind or date as a *view* (nothing moves) | Keep | Model-level grouping | P1 |
| What takes the space, by kind | Keep | From the index | P1 |
| Suggestions: installer for an app you already have | Adapt | `.deb` whose package is installed at the same or newer version (read with `dpkg-deb -f`, compare with `dpkg-query`); `.flatpakref` for an installed ref. **AppImages are never suggested**: an AppImage is the app, not an installer. `.iso` files go under "big files", unticked | P1 |
| Suggestions: likely duplicates; untouched 90 days | Adapt | Duplicates by size, then partial hash, then full hash (BLAKE3). "Untouched" uses access time; if the mount uses `noatime`, fall back to modified time and say "not changed" instead of "not opened" | P1 |
| **Smart Clean** (disk overview, nine suggestions in two groups, review, Move to Trash, Undo) | Adapt | Same structure and rules. The suggestion list is rebuilt for Ubuntu: see [§4.3](#43-smart-clean-on-ubuntu) | P1 |
| **Workspaces** (folders and files from anywhere on one page; nothing moves; description, colour; Locate for missing items) | Keep | Database references by path plus device ID and inode for re-finding after a rename. Items on an unplugged drive show greyed with the drive name | P1 |
| **Dual pane** (split, copy and move across, drag, queue, clash dialog, panes restored after relaunch) | Keep | `F3` toggles (the long-standing Linux file-manager key), `F5` copies, `F6` moves, `Tab` switches | P1 |
| **Search** (folders by name anywhere, then files, sorted by kind, thumbnails, video length) | Keep | Own index in SQLite FTS5 with a trigram tokenizer ([§7.1](#71-local-backend-inside-the-desktop-app)) | P1 |
| Search inside files, with the matched line | Keep | Text extracted in a sandboxed helper: plain text and code, Markdown, PDF (`pdftotext`), OpenDocument and Office files (zipped XML). Off by default per folder outside Home | P2 |
| Search finds words in screenshots | Adapt | OCR with Tesseract, optional package, only for the Screenshots folder and imported phone screenshots | P3 |
| **Peek** (Space; PDFs scroll, videos play, Info with where a file came from) | Adapt | Images via QML `Image`, PDF via Qt PDF, video and audio via Qt Multimedia, text and code in a QML text view with syntax colouring. "Where from" reads the `user.xdg.origin.url` attribute, which only some downloaders set, so the row is shown only when present | P1 |
| **Markdown** (Preview, Edit, Split, outline) | Keep | QML `TextArea` for editing; preview with QML's own Markdown rendering (no web engine, so no scripts and no network) | P2 |
| **Bookmarks** (links with pictures, collections, tags, four views) | Adapt | GNOME has no system Share menu. Ways in: paste with `Ctrl+V`, drag a link onto the page, `steward add-link URL`, the dock-icon action "Add Link". A browser extension is a later option. Fetching a link's title and picture contacts that site; say so in Settings and allow turning it off | P2 |
| **Git at a glance** (branch and uncommitted count, read-only) | Keep | Pure-Rust Git reader (`gix`), so no dependency on a `git` binary and no chance of writing | P2 |
| Path bar commands (`code .`, `zed`, `terminal` …) | Keep | Maps to Open In targets | P2 |
| **AI Agents** (projects set up for coding agents, their files, MCP servers, token usage from local logs, local dev servers with Stop) | Keep | These tools keep the same files on Linux **[verify paths per tool]**. Local servers come from listening sockets in `/proc` matched to the user's processes. Stop sends a normal terminate signal after asking | P2 |
| **Installed** (packages from several tools in one list, why each is there, caches, save the list) | Adapt | Sources: **apt** (manual versus automatic from `apt-mark`), **Snap**, **Flatpak**, then npm global, pipx and pip user installs, Cargo, Go, RubyGems, and Homebrew if present. Read-only; removal is a command to copy. "Not run lately" is shown only where the access time is trustworthy | P2 |
| **About This Mac** | Replace | **About This Computer**: see [§4.6](#46-about-this-computer) | P2 |
| **Your iPhone** | Adapt | **Your phone**: see [§4.7](#47-your-phone) | P3 |
| **Drop Shelf and Holding** | Adapt | **Shelf**: see [§4.4](#44-redesigned-the-shelf) | P3 |
| Menu bar icon (latest downloads, quick access) | Adapt | Tray icon that toggles the Shelf; the Shelf gains a "Latest downloads" row and quick-access folders | P3 |
| Spotlight integration | Replace | GNOME Shell search provider | P3 |
| Shortcuts and Siri actions (eight actions) | Replace | `GApplication` actions, the `steward` CLI and desktop-file actions | P2 |
| Open at login | Keep | XDG autostart entry, off by default, only needed for the Shelf and tray | P3 |
| Welcome tour, "What's New" after update, Report a Bug | Keep | "Report a Bug" opens the project's issue tracker instead of an email | P1 |
| Licence key: buy, activate, deactivate, move to another computer | Exclude | The project is open source | – |
| Customisable shortcuts | Keep | Settings page; stored in GSettings | P2 |

### 4.3 Smart Clean on Ubuntu

The reference's nine cards, re-derived for Ubuntu. The rules stay: each item says why; pre-ticked only when clearly spare; everything goes to the Trash; Undo restores.

| Group | Card | What it finds on Ubuntu | Pre-ticked? |
|---|---|---|---|
| Safe to clear | Developer caches | A strict list of download caches the tools rebuild: npm, Yarn, pip, Cargo registry cache, Go build cache, Gradle caches, ccache, Homebrew downloads. Nothing else, however big | Yes |
| Safe to clear | Duplicates | Same contents in more than one place. One copy always kept, a filed one first | Only copies in Downloads or Desktop, or named like a copy |
| Safe to clear | Installers | `.deb` files for packages already installed | Only if in Downloads or Desktop, not newer than the installed version, older than a week |
| Your call | Stale build output | `node_modules`, `target`, `.venv`, `build` in projects untouched for 6 months | No |
| Your call | App leftovers | Data folders of Flatpaks and Snaps that are no longer installed (`~/.var/app/<id>`, `~/snap/<name>`), untouched for 3 months. These have unambiguous owners, unlike `~/.config` | No |
| Your call | Big files | Largest files in Home, including disk images | No |
| Your call | Old screen recordings | `~/Videos/Screencasts` older than 90 days | No |
| Your call | Old downloads | Not opened, changed or added for 90 days | No |
| Your call | Unused apps | Apps not launched for 6 months. Linux keeps no reliable last-launched date, so this card ships only if Phase 0 finds a trustworthy source **[verify]**; otherwise it is dropped | No |
| Needs administrator | System clean-up | apt package cache, disabled Snap revisions, system journal over a size, old kernels, unused Docker images. Each row shows the size and **the command to copy**. The app runs none of them | n/a |

*Replaced:* "Mail attachments" is specific to Apple Mail's download folder. "Stale build output" takes its slot (the reference lists old `node_modules` inside the developer card).

*Disk bar segments:* Your files (Home), Apps (Snap, Flatpak and `/usr`), System, Other, In the Trash, Free. Free space is counted the same way Files and Disk Usage Analyzer count it, so numbers agree across apps.

### 4.4 Redesigned: the Shelf

The reference's Drop Shelf drops out of the MacBook notch on ⌥Space, or when a drag starts in any app. Three desktop facts block a direct port:

| Reference behaviour | Why it does not port | Ubuntu design |
|---|---|---|
| Drops from the notch, top centre | A Wayland app cannot choose its position on screen | A small Shelf window the compositor places. With our optional GNOME Shell extension installed, the extension pins it top-centre, above other windows |
| ⌥Space from any app | `Alt+Space` is the window menu on GNOME. Global keys need the GlobalShortcuts portal, which older GNOME lacks **[verify on 24.04]** | Default `Ctrl+Alt+Space`. Registered through the portal where available; otherwise as a GNOME custom keybinding that runs `steward shelf` |
| Opens when a drag starts in any app | An app cannot observe other apps' drags | Not in the base product. The extension may be able to detect drags through the compositor **[verify, spike]** |

What stays the same: **Holding** (a well that remembers files without moving them, persists across restarts, drag out to any app), numbered pinned folders (press 1 to 9), drop to move, `Ctrl` while dropping to copy (the Linux convention, where the reference uses ⌥), the result line with Undo. The Shelf also absorbs the reference's menu-bar panel: a "Latest downloads" row and quick-access folders.

The extension is strictly optional. Every Shelf function works without it.

### 4.5 Excluded, with reasons

| Excluded | Reason |
|---|---|
| Full Disk Access onboarding and Settings › Folder Access | macOS privacy gate. A `.deb` app has ordinary user permissions |
| Xcode caches (DerivedData, device support, Simulator), CocoaPods, `xed .` | Apple developer tools; not on Linux |
| Mail attachments card | Apple Mail behaviour |
| Time Machine snapshots in the disk bar | macOS feature. Timeshift or filesystem snapshots, if present, are listed as information only |
| FileVault, Gatekeeper, System Integrity Protection rows | Replaced by Ubuntu equivalents ([§4.6](#46-about-this-computer)) |
| iCloud Drive | No Linux client |
| AirDrop, Finder tags, "Finder" folder style | Apple-only concepts |
| Siri and the Shortcuts app | Replaced by D-Bus actions and a CLI |
| "Move to Applications" onboarding step | Package installs put the app in place |
| Notch, "pointer rests on the notch" trigger | No notch |
| iTerm, Nova, BBEdit in Open In | Mac-only apps |
| Sound effects | Low value on Ubuntu; revisit later |
| In-app updater | apt does this |
| Checkout, licence keys, activation, the thank-you page, the launch waitlist | The project is open source; there is nothing to sell or activate |
| "Compared with Finder" table as-is | Rewrite as "Files, or Steward?" against Nautilus, with claims checked against the Nautilus version on each supported Ubuntu release |

### 4.6 About This Computer

| Reference row | Ubuntu source |
|---|---|
| Model, chip, cores, architecture | `/sys/class/dmi`, `/proc/cpuinfo` |
| OS version, kernel, uptime, shell, region | `/etc/os-release`, `uname`, `/proc/uptime` |
| "In use" since set-up | Creation time of the root filesystem or the installer log; labelled as an estimate |
| Battery: condition, maximum capacity, cycles, charge, power | UPower over D-Bus, with `/sys/class/power_supply` for cycle count |
| Memory in use, pressure, swap | `/proc/meminfo` and `/proc/pressure/memory` |
| Thermal state | `/sys/class/thermal` |
| Startup disk: free, used, filesystem, encrypted | `statvfs` and UDisks2 (LUKS detection) |
| Displays: size, resolution, refresh, HDR | Mutter's display configuration D-Bus interface |
| Security: FileVault, Firewall, SIP, Gatekeeper | **Disk encryption (LUKS)**, **Firewall (ufw)**, **Secure Boot**, **AppArmor**, **automatic security updates**. Rows that need root to read show "Unknown" rather than prompting |
| Packages count | apt, Snap and Flatpak counts from the Installed feature |
| Hide Names, Copy as Text | Keep |

### 4.7 Your phone

| Reference (iPhone) | Ubuntu version |
|---|---|
| iPhone appears in the sidebar when plugged in | Any phone GVfs exposes. **Android (MTP)** is the primary target. **iPhone** works through the camera (PTP) interface via GVfs, after the phone is unlocked and "Trust" is tapped; it depends on libimobiledevice supporting the phone's iOS version **[verify with current iOS]** |
| Photos, videos, screenshots by day; filter chips | Keep. Read the `DCIM` tree (and `Pictures/Screenshots` on Android); dates from EXIF, else file time |
| "Not on Mac yet" and "On Mac" badge | Keep, as "On this computer". Matched by a fingerprint (size, capture time, name) recorded at import, with a content hash check before skipping |
| Copy to any folder or workspace, drag to the sidebar; always a copy; Undo | Keep. Read-only on the phone by design |
| Battery percentage and model in the header | Shown when the device reports it (UPower lists some phones); otherwise omitted |
| Search words in screenshots | P3, after import, via OCR |

MTP is slow and single-threaded. The design must show thumbnails progressively, never block, and cache the phone's listing per device between sessions.

### 4.8 Ubuntu-only additions

Small features the reference cannot have, which make the product feel native:

- **"Open as default file manager" option**, off by default, implementing `org.freedesktop.FileManager1` so "Show in folder" in browsers can open Steward.
- **Snap, Flatpak and apt awareness** throughout Installed, Smart Clean and About.
- **Dock-icon actions**: New Window, New Workspace, Free Up Space, Add Link.
- **Network places**: connect to server (SMB, SFTP, WebDAV) through GVfs.

---

## 5. Assets: mapping the scraped material

All paths are under [fileosmac/](fileosmac/). **None of these files ship.** Each is a reference with a named replacement.

### 5.1 Site images

| Scraped asset | What it is | Use in the Ubuntu project | Replacement to produce |
|---|---|---|---|
| `images/icon-512.png`, `icon-256.png`, `icon-64.png` | FileOS app icon (purple glass folder) | Reference for icon *role and sizes* only | Original icon following the GNOME app-icon guidelines: full-colour SVG plus a symbolic SVG; installed under `hicolor` |
| `images/share-image.png` | 1200×630 social card | Reference for layout | Own card, generated from a real app screenshot |
| `images/shoji-icon.png` | Sister app icon | None | – |
| `images/proof/*.jpg` | Avatars of six X users | **None. Do not reuse** ([§1.3](#13-name-brand-and-what-we-may-not-copy)) | Real feedback on our app, with permission, once it exists |

### 5.2 Recordings and stills

Each recording doubles as an **interaction specification** for one screen and as the **shot list** for our own marketing captures.

| Capture | Specifies (app) | Site section it becomes | Pri |
|---|---|---|---|
| `gifs/hero.gif`, `screenshots/hero.png` | The welcome demo ("a messy folder, then sorted") and the tour order | Hero | P1 |
| `gifs/features.gif` | – | Feature grid (seven or eight tiles, per [§4](#4-features-what-to-keep-adapt-replace-and-exclude)) | P1 |
| `gifs/smart-downloads.gif` | Downloads view, Tidy plan, side panel | Smart Downloads | P1 |
| `gifs/smart-clean.gif` | Disk bar, cards, review, Trash confirm | Free Up Space | P1 |
| `gifs/workspaces.gif` | Workspace page, card sources | Workspaces | P1 |
| `gifs/dual-pane.gif` | Pane focus, F5/F6, toast | Dual pane | P1 |
| `gifs/search.gif` | Overlay layout, chips, matched-line rows, key hints | Search | P1 |
| `screenshots/design.png` | Card anatomy, folder card, preview-pane tree | Design | P1 |
| `gifs/developers.gif` | Git chip, Open In menu | For developers | P2 |
| `gifs/peek.gif`, `bookmarks.gif`, `about-this-mac.gif`, `screenshots/ai-agents.png`, `installed.png` | The five secondary tools | "More in Steward" tiles | P2 |
| `gifs/iphone.gif` | Phone view, picked bar, badge | Your phone | P3 |
| `gifs/drop-shelf.gif` | Holding, pinned folders, drop feedback | Shelf | P3 |
| `screenshots/keys.png` | Shortcut set (remapped in [Appendix A](#appendix-a-keyboard-map)) | Keyboard section | P1 |
| `gifs/final.gif` | Folder animation style | Closing banner | P2 |
| `screenshots/full-page.jpg` | Section order and rhythm of the landing page | Site information architecture | P1 |

### 5.3 Text and data

| Scraped material | Use |
|---|---|
| Landing-page copy and the 27 guide pages | Outline and tone reference. Rewrite everything in our own words; the guide's page list ([Appendix B](#appendix-b-what-was-inspected)) is a good table of contents for ours |
| Demo data in the mock-ups (file names such as `Invoice-0923.pdf`, `team-offsite.mov`, workspace "Acme Rebrand", the 63-item messy Downloads) | Model for the **fixture home folder**: a script generates a comparable fake home with our own names. It feeds development, automated tests and our screenshots ([§9.4](#94-the-fixture-home-folder)) |
| Comparison table (16 rows) | Structure only. Re-research every row against Files |
| FAQ (11 questions) | Keep the questions that still apply (does it replace the system file manager, cloud drives, which releases, privacy, is Smart Clean safe). Drop the price, licence and refund questions. Add "How do I contribute?" |

### 5.4 How our own captures are made

Screenshots and screen recordings are **generated, not hand-made**: the test container runs the real app under a headless compositor against the fixture home and saves captures for each screen, in light and dark, in each language. The same captures feed the website, the guide and the AppStream metadata. This keeps marketing images true to the shipped app, which the reference's hand-drawn mock-ups cannot guarantee.

---

## 6. Frontend stack

### 6.1 Desktop UI

**Choice: Qt 6 with QML for the interface, on a Rust engine, bridged by cxx-qt** (ADR 0009). The first plan (GTK 4 + libadwaita, below) was built through Phase 1 and replaced after a proof showed Qt/QML could match the reference's look and motion natively (`spikes/qt-proof`).

| Option | Verdict | Reasoning |
|---|---|---|
| **Qt 6 / QML + Rust engine (cxx-qt)** | **Chosen** | QML's animation system (behaviours, states, transitions, list add/remove/displaced transitions, springs) covers every motion in the captures. Real blur and shadows (`QtQuick.Effects`). Fully custom drawing for glass folders and cards. Native Wayland, high-DPI, AT-SPI accessibility, drag and drop with other apps, clipboard with any MIME type. Qt runs GLib's main loop on Linux, so the GIO-based engine runs unchanged |
| GTK 4 + libadwaita (Rust) | Replaced | Excellent file-manager plumbing and the native GNOME look, but no blur behind widgets and motion would be hand-built. Its plumbing advantage is kept: the engine uses GIO directly |
| Flutter | Rejected | Strong animation, but gaps in drag-out to other apps and file-manager plumbing; Dart beside the Rust engine |
| Tauri or Electron | Rejected | Web animation is easy, but native file drags, trash and mounts are limited, and Electron's memory cost suits an always-open utility poorly |
| Slint | Rejected | Native Rust with animations, but no drag and drop with other apps, which a file manager needs |

**How the pieces fit**

```
QML (crates/app/qml)                 Rust (crates/app/src)                     Engine (crates/core)
─────────────────────                ─────────────────────                     ────────────────────
Main.qml, Sidebar, Header, …  ──►  Presenter (cxx-qt QObject)  ──►  Controller (plain Rust)  ──►  FolderModel (GIO),
binds to JSON properties             thin: properties + invokables          state, navigation,          sort, kinds, store,
calls presenter.activate(…)          + signals (toast, quit, …)              selection, layout           places, guard
                                     ▲                                         │
                                     └── GIO actions (org.gtk.Actions) ◄──────┘  exported on D-Bus: menus, keys, CLI, tests
```

- **Presenter.** The only QObject. Publishes the view state as JSON properties (folder, grid rows, list items, selection, thumbnails, sidebar, preview, path bar, navigation, preferences) and takes commands through invokables. JSON keeps the bridge small and lets QML use plain JavaScript objects.
- **Controller.** Owns the state and all logic in plain Rust: the listing, navigation history, view options, filters, selection, grid layout (columns from the viewport width), sidebar, preview, thumbnails queue. Unit-tested without Qt.
- **Actions.** Every user command is a `gio::SimpleAction` (`app.*`, `win.*`), exported on the session bus as `org.gtk.Actions` and activated by QML shortcuts and menus. Same names as before, so tests and the future CLI do not change.
- **Large folders.** QML `ListView`s create delegates only for visible rows (the grid is a list of card rows), so 10,000-item folders stay fast.

**Libraries and choices**

| Concern | Choice | Note |
|---|---|---|
| Bridge | cxx-qt 0.10, cxx-qt-lib (gui, qml, quickcontrols) | The bridge module is the one place `unsafe extern "C++"` is allowed |
| Controls | Qt Quick Controls, Basic style, restyled | Text fields, menus, popups, switches, combo boxes |
| Effects | `QtQuick.Effects` (MultiEffect) | Blur, shadows, colourising icons |
| Icons | Our own line icons as SVG in `qml/icons` (Qt SVG plugin) | Drawn for this project in the reference's line style; recoloured in QML |
| Thumbnails | Existing sandboxed thumbnailer (Rust, `bwrap`); QML shows the cached PNG | Unchanged |
| Peek (Phase 3) | Images: QML `Image`; PDF: Qt PDF; audio and video: Qt Multimedia; text and code: QML `TextArea` with syntax colouring (KSyntaxHighlighting if packaged) | Replaces Poppler/GStreamer/GtkSourceView in the GTK plan |
| Markdown (Phase 7) | Preview with QML `Text.MarkdownText` (no web engine); editor in `TextArea`; outline from the Rust side | No WebKit needed |
| Settings | GSettings through the `gio` crate | Unchanged |
| Light and dark | Setting "system/light/dark"; "system" reads the desktop through Qt's style hints (Qt 6.5+) | |
| Window chrome | Frameless window, our header, `startSystemMove`/`startSystemResize` | Matches the look on GNOME |
| Translation | `qsTr()` in QML, gettext in Rust | Unified in Phase 6 |

**Module structure** (crate `steward`):

```
crates/app/
├─ build.rs              QML module dev.steward.files, cxx-qt bridge, icons, settings schema
├─ qml/
│  ├─ Main.qml           window, layout, shortcuts, dialogs
│  ├─ Theme.qml          colour and size tokens (§3.6)
│  ├─ Sidebar.qml  Rail.qml  Header.qml  PathBar.qml  ChipRow.qml
│  ├─ FolderGrid.qml  FolderList.qml  GlassFolder.qml  FileCard.qml
│  ├─ PreviewPane.qml  StatusPage.qml  Toast.qml  Segmented.qml  Icon.qml  IconButton.qml
│  ├─ Dialog.qml  AboutDialog.qml  PreferencesDialog.qml  ShortcutsDialog.qml  OpenWithDialog.qml  MenuPopup.qml
│  └─ icons/             our SVG icons + icons.qrc
└─ src/
   ├─ main.rs            fixture guard, then app::run
   ├─ app.rs             GLib context, Qt application, QML engine
   ├─ bridge.rs          Presenter (cxx-qt)
   ├─ controller.rs      state and logic
   ├─ dbus.rs            bus name, exported actions, Test1 (e2e builds)
   ├─ sidebar.rs  paths.rs  thumbnails.rs  settings.rs  format.rs  app_data.rs  tint.rs
```

<details><summary>The original GTK 4 plan (kept for the record)</summary>

GTK 4 + libadwaita in Rust (`gtk4-rs`), Blueprint layouts, libadwaita widgets (`AdwOverlaySplitView`, `AdwDialog`, `AdwToastOverlay`, `AdwPreferencesDialog`), `GtkGridView`/`GtkColumnView` for files, GtkSourceView and WebKitGTK for Markdown, Poppler and GStreamer for Peek, CSS with libadwaita named colours. Built through Phase 1 (commit fb3c7b4) and replaced by ADR 0009.

</details>

### 6.2 Website and docs

**Recommendation: Astro, static output, content as Markdown in the repository.**

- The reference's site is 980 KB of hand-written HTML with animated SVG mock-ups. It is impressive and expensive to maintain. We get the same result more cheaply by embedding **real captures** ([§5.4](#54-how-our-own-captures-are-made)) as AVIF stills and short WebM clips.
- Astro suits a content site with a few interactive islands (theme toggle, docs search) and ships no JavaScript elsewhere.
- Docs search: Pagefind, a static index built at build time. No search service.
- Styling: plain CSS with custom properties. The site may use a brand colour of its own; the *app* follows the system accent.
- Fonts: self-hosted, not Google Fonts (the reference's privacy page has to disclose that request; we can avoid it).
- Pages: landing, `/docs/*`, `/changelog/`, `/download/` (apt instructions and a `.deb`), `/contribute/`, `/privacy/`.

### 6.3 Optional GNOME Shell extension

Plain GJS, no build framework, in `shell-extension/`. Scope is deliberately tiny: place and keep-above the Shelf window, and (if the spike succeeds) signal the app when a file drag begins. It talks to the app over the session D-Bus. It must be rebuilt and re-tested for each GNOME version, which is why nothing essential depends on it.

---

## 7. Backend architecture

### 7.1 Local backend (inside the desktop app)

All logic that is not widgets lives in `steward-core`, a Rust library with **no UI-toolkit dependency** (neither Qt nor GTK), so it can be unit-tested headlessly and reused by the indexer and the CLI.

| Module | Responsibility | Key technology |
|---|---|---|
| `vfs` | Listing, metadata, monitors, mounts, MIME, app lookup | GIO |
| `ops` | **Operations engine**: copy, move, rename, trash, restore, batch; queue, progress, cancel, clash policy; **undo journal** | GIO file operations, SQLite journal |
| `plan` | Builds a previewable plan (Tidy, Clean) as a list of operations before anything runs | Pure logic |
| `index` | File and folder name index, content index, folder sizes, kind breakdown | SQLite FTS5 (bundled build), trigram tokenizer |
| `extract` | Text from documents, thumbnails via system thumbnailers | Runs in a `bwrap` sandbox, no network, read-only view of the one file |
| `dupes` | Duplicate finder | Size, then 64 KB partial hash, then full BLAKE3 |
| `clean` | Suggestion providers, one per card, each returning items with a reason and a default tick | Trait `Suggester` |
| `packages` | Readers for apt, Snap, Flatpak, npm, pipx, Cargo, Go, RubyGems, Homebrew | Parse each tool's own files or call its query command; never a mutating command |
| `git` | Branch and dirty count | `gix`, read-only |
| `agents` | Agent config discovery, MCP server lists, token usage from local logs, listening servers | File parsing, `/proc` |
| `sysinfo` | About This Computer | D-Bus (UPower, UDisks2, Mutter), `/proc`, `/sys` |
| `phone` | Device discovery, listing, import, "on this computer" matching | GVfs MTP and PTP mounts behind a `PhoneSource` trait (fake implementation for tests) |
| `workspaces`, `bookmarks`, `shelf` | User data models | SQLite |

**Why our own index rather than the desktop's (LocalSearch/Tracker).** The system indexer covers only configured folders, differs in name and schema between Ubuntu releases, and is tuned for desktop search rather than "every folder with this name, anywhere". We need folder sizes and kind breakdowns from the same crawl anyway. One crawl, one database, same behaviour on every release.

**The indexer** (`steward-indexer`) is a separate small binary run as a **systemd user service**, started by the app on first launch and D-Bus activatable. It crawls at idle priority, watches with inotify, falls back to periodic re-scans when the watch limit is reached, and pauses on battery. It is the only writer of the index database; the app opens it read-only.

**Local data**

| Store | Location | Contents | If deleted |
|---|---|---|---|
| `app.db` (SQLite, WAL) | `~/.local/share/steward/` | Workspaces, bookmarks, sidebar, Shelf, undo journal, phone import records | User data lost: back this up |
| `index.db` (SQLite, WAL) | `~/.cache/steward/` | Names, content, sizes | Rebuilt automatically |
| GSettings | dconf | Preferences, shortcuts | Defaults return |
| Thumbnails | `~/.cache/thumbnails/` (shared FreeDesktop cache) | | Rebuilt |

`app.db` schema (abridged):

```sql
CREATE TABLE workspace (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT,
  color TEXT, position INTEGER NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE workspace_item (
  id INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
  uri TEXT NOT NULL,              -- file:// or a GVfs URI
  fs_id TEXT, inode INTEGER,      -- to re-find after rename on the same filesystem
  volume_label TEXT,              -- shown when the drive is not connected
  is_dir INTEGER NOT NULL, position INTEGER NOT NULL);

CREATE TABLE collection (id INTEGER PRIMARY KEY, name TEXT NOT NULL, position INTEGER);
CREATE TABLE bookmark (
  id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, title TEXT, site TEXT,
  kind TEXT CHECK (kind IN ('article','video','image','file','other')),
  image_path TEXT, favourite INTEGER DEFAULT 0,
  collection_id INTEGER REFERENCES collection(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL);
CREATE TABLE bookmark_tag (bookmark_id INTEGER REFERENCES bookmark(id) ON DELETE CASCADE,
  tag TEXT NOT NULL, PRIMARY KEY (bookmark_id, tag));

CREATE TABLE sidebar_entry (id INTEGER PRIMARY KEY, section TEXT, uri TEXT, label TEXT,
  position INTEGER, hidden INTEGER DEFAULT 0);
CREATE TABLE shelf_pin  (id INTEGER PRIMARY KEY, uri TEXT NOT NULL, position INTEGER);
CREATE TABLE shelf_held (id INTEGER PRIMARY KEY, uri TEXT NOT NULL, added_at INTEGER);

-- Undo journal: one batch per user action, many steps per batch
CREATE TABLE op_batch (
  id INTEGER PRIMARY KEY, kind TEXT NOT NULL,     -- move|copy|rename|trash|tidy|clean|import
  summary TEXT NOT NULL, created_at INTEGER NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('running','done','undone','partial')));
CREATE TABLE op_step (
  id INTEGER PRIMARY KEY, batch_id INTEGER NOT NULL REFERENCES op_batch(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL, action TEXT NOT NULL,
  src_uri TEXT, dst_uri TEXT, trash_uri TEXT,
  state TEXT NOT NULL CHECK (state IN ('pending','done','failed','undone')), error TEXT);

CREATE TABLE phone_import (
  device_id TEXT NOT NULL, remote_path TEXT NOT NULL, size INTEGER, taken_at INTEGER,
  local_uri TEXT NOT NULL, content_hash BLOB, imported_at INTEGER,
  PRIMARY KEY (device_id, remote_path));
```

`index.db` schema (abridged):

```sql
CREATE TABLE entry (
  id INTEGER PRIMARY KEY, parent_id INTEGER, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
  is_dir INTEGER NOT NULL, size INTEGER, mtime INTEGER, btime INTEGER, atime INTEGER,
  mime TEXT, kind TEXT,
  tree_size INTEGER, tree_count INTEGER,      -- folders only, rolled up
  scanned_at INTEGER);
CREATE VIRTUAL TABLE name_fts    USING fts5(name, content='entry', content_rowid='id', tokenize='trigram');
CREATE VIRTUAL TABLE content_fts USING fts5(body, tokenize='unicode61 remove_diacritics 2');
CREATE TABLE content_state (entry_id INTEGER PRIMARY KEY, mtime INTEGER, status TEXT);
```

**Undo semantics** (from the reference's troubleshooting page, kept as rules): undo never overwrites; if a name is now taken in the old place, that file is reported and left where it is. A batch can end `partial`, and the toast says which files could not be put back. The journal keeps the last 50 batches and survives restart.

### 7.2 Project infrastructure (no server application)

The project is open source. There is no checkout, no licence key, no account and no telemetry, so there is **no server application and no server database**. Everything hosted is a static file.

```
  Git repository ──CI──▶ .deb per Ubuntu release ──▶ signed apt repository (static files)
        │         └─CI──▶ website and guide (static files)
        ▼
  issue tracker · pull requests · translations

  Users:   browser ─▶ website        apt ─▶ repository        "Report a Bug" ─▶ issue tracker
```

| Piece | Technology | Purpose |
|---|---|---|
| Website and guide | Astro build, served by any static host. A Caddy container is provided for local preview and for self-hosting | Landing page, guide, changelog, download and contribute pages |
| apt repository | `aptly`, published as static files, signed with the project key | Delivery and updates |
| Release downloads | `.deb` files attached to each tagged release on the code host | First install without adding a repository |
| Source, issues, reviews | The project's Git host | Development in the open |
| Translations | `.po` files in the repository; a hosted translation service can be added later | Community translation |

**Content management.** Site copy, guide pages and release notes are Markdown files under `site/src/content/`, validated by Astro content collections (a typed schema catches a missing title or date at build time). A change is a pull request; merge deploys. Release notes are written once and feed three places: the site changelog, the in-app "What's New" and the Debian changelog.

**What was dropped from the reference, and why.** Checkout, licence keys, activation and deactivation, the thank-you page and the launch waitlist all exist to sell the Mac app. None applies to an open-source project. If the model ever changes, licensing would be added as one new module in `steward-core` plus a small service; nothing else in this design depends on its absence.

### 7.3 Integrations

| Integration | Direction | Used for |
|---|---|---|
| GVfs | App → system | Trash, mounts, phones, network shares |
| UPower, UDisks2, Mutter display config | App → system D-Bus | About This Computer, devices |
| xdg-desktop-portal (GlobalShortcuts, Background) | App → session | Shelf key, autostart, where available |
| GNOME Shell search provider | Shell → app | Workspaces, collections and links in Activities search |
| `org.freedesktop.FileManager1` | Other apps → app, and app → Files | "Show in folder" both ways |
| StatusNotifierItem | App → panel | Tray icon |
| dpkg, apt, snap, flatpak and language package tools | App → tools, read-only | Installed, installers, Smart Clean |
| Network | App → the web | **The app sends no telemetry and contacts no project server.** Its only outbound requests go to the sites of saved bookmarks, and only if that option is on |

---

## 8. Technical architecture

### 8.1 Components

```
┌─────────────────────────────── user session ───────────────────────────────┐
│                                                                             │
│  steward (Qt/QML app + Rust)               steward-indexer (systemd user unit)  │
│  ┌───────────────────────────┐           ┌───────────────────────────┐      │
│  │ QML UI ◄─► Presenter      │           │ crawler · inotify watcher │      │
│  ├───────────────────────────┤  D-Bus    │ size roll-up              │      │
│  │ steward-core                │◀─control─▶│ text extraction ──┐       │      │
│  │  ops engine + undo journal│           └───────┬───────────│───────┘      │
│  │  plan · clean · dupes     │                   │ writes    ▼              │
│  │  packages · git · agents  │   reads   ┌───────▼──────┐  bwrap sandbox    │
│  │  sysinfo · phone          │──────────▶│  index.db    │  (extractors,     │
│  └──────┬─────────┬──────────┘           └──────────────┘   thumbnailers)   │
│         │         │ reads/writes                                            │
│         │         ▼                                                         │
│         │   ┌───────────┐   optional: GNOME Shell extension                 │
│         │   │  app.db   │   (places the Shelf, drag signal) ◀── D-Bus ──▶ app│
│         │   └───────────┘                                                   │
│         ▼                                                                   │
│   GIO / GVfs · D-Bus services · /proc · /sys · package tools (read-only)    │
└─────────────────────────────────────────────────────────────────────────────┘
                                                        │ apt (install and updates)
                                                        ▼
   ┌────────────── static hosting ──────────────────────────────────┐
   │   website and guide  ·  signed apt repository  ·  release .deb  │
   └─────────────────────────────────────────────────────────────────┘
```

### 8.2 Flow: Tidy Downloads (the pattern for every change)

```
User clicks "Tidy Downloads…"
  └▶ plan::tidy(folder)            pure function: reads listing, builds Plan
        ├─ steps:   move A → Images/A, move B → PDFs/B, …
        └─ skipped: still downloading, added < 2 min ago, folders
  └▶ UI shows the Plan dialog      nothing has changed on disk
User clicks "Tidy"
  └▶ ops::run(Plan)                writes op_batch + op_steps (pending) to app.db
        └─ worker executes steps through GIO, marks each done/failed, reports progress
  └▶ Toast: "Tidied 18 files into 6 folders · Undo"
User clicks "Undo" (or Ctrl+Z)
  └▶ ops::undo(batch)              reverses done steps in reverse order; never overwrites;
                                   empty folders it created are removed; batch → undone|partial
```

Smart Clean, phone import, dual-pane copy, rename and trash all use the same three stages: **plan, run, journal**. A feature that changes files without going through `ops` is a defect.

### 8.3 Flow: a release

```
Maintainer pushes tag vX.Y.Z
  └▶ CI builds one .deb per supported Ubuntu release (in the Docker build image)
  └▶ CI installs each .deb in a clean container and runs the smoke end-to-end tests against it
  └▶ CI signs and publishes to the apt suite "stable"; attaches the .deb files to the release
  └▶ CI rebuilds the website (changelog page comes from the same release-notes file)
User's machine: apt upgrade brings the new version with the rest of the system
App, first run after the upgrade: shows "What's New" once
```

### 8.4 Performance

Targets to design and test against (targets, not measurements):

| Scenario | Target |
|---|---|
| Launch to usable window | under 0.5 s warm |
| Open a folder of 10,000 items | first paint under 150 ms; scrolling at display rate |
| Search as you type (names), 1 million indexed entries | results under 50 ms per keystroke |
| First full index of a 500 GB home | in the background at idle priority; app usable throughout |
| Memory, idle with one window | under 150 MB |

How:

- **Virtualised views** bound to list models; thumbnails load for visible rows only and cancel on scroll.
- **No I/O on the UI thread.** All listing, hashing and scanning runs on worker threads and reports through channels.
- **Incremental everything.** The indexer stores modification times and re-reads only what changed. Folder sizes roll up from children.
- **Cheap duplicate detection.** Group by size first; hash only candidates; partial hash before full hash.
- **Slow devices are special.** Phones and network mounts get progressive listing, per-device caches, and no recursive size scans unless asked.
- **inotify limits.** Watch open folders always; watch indexed trees while under the kernel limit; otherwise re-scan on a timer and on window focus.

### 8.5 Security and privacy

| Area | Measure |
|---|---|
| Untrusted file content | Text extraction and thumbnailers run in `bwrap` with no network, a read-only view of the single input file and a size and time limit |
| Markdown preview | QML's Markdown rendering: no scripts, no network; local images only from the document's folder |
| Privilege | No root, no polkit actions in v1. System clean-ups are commands to copy |
| Deletion | Always to Trash. Permanent deletion only through "Empty Trash", which asks |
| Symlinks | Scans never follow links out of the scanned root; operations act on the link, not the target |
| Path handling | Operate on `GFile` objects, not concatenated strings; never pass file names through a shell |
| Stopping local servers | Only the user's own processes; confirmation with the process name and port |
| Network | The app contacts no project server and sends no telemetry. Its only outbound requests go to the sites of saved bookmarks, if enabled; this is disclosed in Preferences |
| Supply chain | `Cargo.lock` committed, `cargo audit` and `cargo deny` in CI, pinned container digests, signed apt repository, signed release tags |
| Hosting | Static files only, over TLS. The apt signing key lives in the CI secret store, never in the repository or an image |

### 8.6 Scalability

- **Desktop.** Scale means big folders and big disks, covered in [§8.4](#84-performance).
- **Hosting.** There is no server application to scale. The website and the apt repository are static files; any static host or CDN carries them.

### 8.7 Maintainability

- **One Cargo workspace**, strict crate boundaries: `steward-core` has no UI; `steward` has no business logic.
- **Platform probes behind traits** (`PhoneSource`, `PackageSource`, `Suggester`, `SysProbe`) with fake implementations, so every feature is testable in a container with no hardware.
- **Two-release build matrix** (24.04 and 26.04) on every pull request catches API drift early.
- **Architecture Decision Records** in `docs/adr/`, one per decision in [§1.2](#12-key-decisions).
- **Testing pyramid**: unit tests in core; integration tests of `ops` and `index` against the fixture home; end-to-end tests that drive the real app under a headless compositor ([§10.2](#102-test-layers-and-the-end-to-end-harness)); an install test of the `.deb` on a clean image.
- **Lint gates**: `rustfmt`, `clippy -D warnings`, `qmllint`, desktop-file and AppStream validation, `lintian`, `hadolint`, site link check.

### 8.8 Packaging and distribution

| Format | Role | Notes |
|---|---|---|
| `.deb` in our signed apt repository | **Primary** | Full filesystem and host-tool access. One-line setup adds the key and source; updates then arrive with system updates. Built per Ubuntu release |
| `.deb` direct download | Secondary | For a first install without adding a repository |
| Flatpak, through Flathub (KDE runtime, current Qt) | Later (Phase 9); possibly the route for 24.04 | Needs broad filesystem permission and still cannot see host packages or `/proc` fully, so Installed, parts of Smart Clean and AI Agents would be reduced |
| Snap | Not planned | A file manager needs classic confinement, which requires store approval **[verify current policy]** |
| Launchpad PPA | Optional second channel | Free builds and hosting for open-source projects, but the builders have no network access, so Rust dependencies must be vendored **[verify]** |

---

## 9. Docker-based development and deployment

**Goal:** a new developer needs only **Docker, Docker Compose and `make`**. No Rust, Qt or Node on the host. (The dev machine has Docker 29.9 and Compose 5.5 and no Rust toolchain, so this is also the practical route.)

### 9.1 Repository layout

```
steward/
├─ Makefile                     every workflow is a make target wrapping docker compose
├─ compose.desktop.yaml         desktop app: dev GUI, tests, end-to-end tests, packaging
├─ compose.yaml                 website preview (Caddy) and apt repository publishing
├─ compose.override.yaml        website dev server with hot reload (auto-loaded)
├─ compose.test.yaml            site checks and the apt-install end-to-end test
├─ compose.prod.yaml            self-hosting the site and repository with TLS (optional)
├─ .env.example                 copy to .env; documented defaults
├─ AGENTS.md                    working rules for AI coding agents (from §10.1)
├─ docker/
│  ├─ desktop.Dockerfile        base → dev → build → package → artifact / install-test
│  ├─ web.Dockerfile            deps → dev → build → prod (Caddy + static site)
│  ├─ aptrepo.Dockerfile        aptly and gnupg, for publishing
│  ├─ caddy/Caddyfile.{preview,prod}
│  ├─ versions.env              pinned image digests and tool versions
│  └─ scripts/                  headless-run, make-fixture-home, publish-apt, apt-install-e2e
├─ rust-toolchain.toml  Cargo.toml  Cargo.lock
├─ crates/{core,app,indexer,cli}/
├─ data/                        desktop file, metainfo, GSettings schema, icons, ui/*.blp, style.css
├─ po/                          translations
├─ tests/e2e/                   end-to-end tests (pytest) and their helpers
├─ fixtures/                    fixture-home recipe and manifest
├─ shell-extension/             optional GNOME Shell extension
├─ site/                        Astro project (src/content/{docs,changelog})
├─ packaging/debian/            cargo-deb metadata, systemd user unit
├─ artifacts/                   git-ignored; test reports, screenshots, logs
├─ secrets/                     git-ignored; dev signing key generated by `make init`
└─ docs/{adr/,PROGRESS.md,perf.md}
```

### 9.2 Environments

There is no server application, so "environment" means *where the desktop build is exercised* and *where the static files are published*.

| | Development | Test (CI) | Staging | Production |
|---|---|---|---|---|
| Compose files | `compose.desktop.yaml`; `compose.yaml` + `compose.override.yaml` | `compose.desktop.yaml` (test, e2e, package); `compose.yaml` + `compose.test.yaml` | `compose.yaml` (preview), or a static host's preview deploy | A static host, or `compose.yaml` + `compose.prod.yaml` when self-hosting |
| Desktop app | Runs on the developer's screen from the dev container, against the fixture home | Runs headless in the container | The `.deb` from the `staging` apt suite, installed on test machines | The `.deb` from the `stable` suite |
| Source | Bind-mounted, rebuild on save | Baked into the image | Built from `main` | Built from a release tag |
| Website | Astro dev server, hot module reload | Built and link-checked | Preview URL per merge | Public site |
| apt suite | `dev`, local volume, throwaway key | `ci`, local volume, throwaway key | `staging`, project key | `stable`, project key |
| Signing key | Generated by `make init` | Generated per run | CI secret store | CI secret store |
| TLS | None, `localhost` | None | Host-provided or Caddy automatic | Host-provided or Caddy automatic |
| Logs and reports | `docker compose logs -f`, `artifacts/` | Uploaded as CI artifacts | Static host logs | Static host logs |

### 9.3 Desktop app containers

**`docker/desktop.Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1.7
ARG UBUNTU_VERSION=24.04
FROM ubuntu:${UBUNTU_VERSION} AS base
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8
# Runtime + build dependencies. BuildKit cache mounts keep apt fast without bloating layers.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean && apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates curl git build-essential pkg-config clang mold \
      libglib2.0-dev gettext desktop-file-utils appstream poppler-utils \
      gvfs gvfs-backends dbus bubblewrap xdg-user-dirs shared-mime-info \
      mutter weston at-spi2-core fonts-ubuntu
# Qt 6 / QML (ADR 0009), a separate layer in the real Dockerfile:
#   qt6-base-dev qt6-declarative-dev qt6-declarative-dev-tools qmake6 qt6-wayland qt6-qpa-plugins
#   qml6-module-qtquick{,-window,-effects,-shapes,-controls,-layouts,-templates} qt6-svg-plugins
# The repository's docker/desktop.Dockerfile is the reference; this listing is the original design.
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH CARGO_TARGET_DIR=/var/cache/cargo-target
COPY rust-toolchain.toml /tmp/toolchain/
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --default-toolchain none && \
    cd /tmp/toolchain && rustup toolchain install     # reads rust-toolchain.toml
WORKDIR /workspace

# ---- dev: interactive development, runs as the host user --------------------
FROM base AS dev
ARG HOST_UID=1000
ARG HOST_GID=1000
ARG WATCHEXEC_VERSION
ARG NEXTEST_VERSION
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends gdb sqlite3 \
      python3-pytest python3-gi gir1.2-atspi-2.0 at-spi2-core wl-clipboard   # end-to-end harness
RUN cargo install --locked watchexec-cli@${WATCHEXEC_VERSION} cargo-nextest@${NEXTEST_VERSION}
# Ubuntu images ship a default user at UID 1000; replace it with one matching the host.
RUN (userdel -r ubuntu 2>/dev/null || true) && \
    groupadd -g ${HOST_GID} dev && useradd -m -u ${HOST_UID} -g ${HOST_GID} -s /bin/bash dev && \
    mkdir -p ${CARGO_TARGET_DIR} && chown -R dev:dev ${CARGO_TARGET_DIR} /usr/local/cargo
USER dev

# ---- build: release binaries -------------------------------------------------
FROM base AS build
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/var/cache/cargo-target \
    cargo build --release --locked -p steward -p steward-indexer -p steward-cli && \
    mkdir -p /out && cp ${CARGO_TARGET_DIR}/release/steward \
       ${CARGO_TARGET_DIR}/release/steward-indexer ${CARGO_TARGET_DIR}/release/steward-cli /out/

# ---- package: .deb -----------------------------------------------------------
FROM build AS package
ARG CARGO_DEB_VERSION
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends dpkg-dev lintian
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/var/cache/cargo-target \
    cargo install --locked cargo-deb@${CARGO_DEB_VERSION} && \
    cargo deb --locked -p steward --no-build --output /out/ && lintian /out/*.deb

# ---- artifact: `docker build --target artifact --output dist/` ---------------
FROM scratch AS artifact
COPY --from=package /out/*.deb /

# ---- install-test: does the .deb install and start on a clean system? --------
FROM ubuntu:${UBUNTU_VERSION} AS install-test
COPY --from=package /out/*.deb /tmp/
RUN apt-get update && apt-get install -y --no-install-recommends /tmp/*.deb && \
    steward --version && steward-cli --help >/dev/null
```

Package names are for 24.04 and should be confirmed on the first image build **[verify]**.

**`compose.desktop.yaml`**

```yaml
name: steward-desktop

x-desktop-build: &desktop-build
  context: .
  dockerfile: docker/desktop.Dockerfile
  args:
    UBUNTU_VERSION: ${UBUNTU_VERSION:-24.04}
    HOST_UID: ${HOST_UID:-1000}
    HOST_GID: ${HOST_GID:-1000}
    WATCHEXEC_VERSION: ${WATCHEXEC_VERSION}
    NEXTEST_VERSION: ${NEXTEST_VERSION}
    CARGO_DEB_VERSION: ${CARGO_DEB_VERSION}

services:
  # Runs the real app in a window on the host's Wayland session (Docker Engine),
  # against a FAKE home folder. Restarts on every source change.
  app:
    build: { <<: *desktop-build, target: dev }
    environment:
      WAYLAND_DISPLAY: ${WAYLAND_DISPLAY:-wayland-0}
      XDG_RUNTIME_DIR: /run/user/${HOST_UID:-1000}
      GDK_BACKEND: wayland
      QML_DEBUG: ${QML_DEBUG:-}             # QML console and binding warnings
      G_MESSAGES_DEBUG: ${G_MESSAGES_DEBUG:-}
      RUST_BACKTRACE: "1"
      STEWARD_DEV: "1"                        # live CSS reload; fixture-home guard on
    tmpfs:
      - /run/user/${HOST_UID:-1000}:uid=${HOST_UID:-1000},gid=${HOST_GID:-1000},mode=0700
    volumes:
      - .:/workspace
      - ${XDG_RUNTIME_DIR}/${WAYLAND_DISPLAY:-wayland-0}:/run/user/${HOST_UID:-1000}/${WAYLAND_DISPLAY:-wayland-0}
      - cargo-registry:/usr/local/cargo/registry
      - cargo-target:/var/cache/cargo-target
      - fixture-home:/home/dev
    devices: ["/dev/dri:/dev/dri"]          # GPU rendering; without a GPU, Mesa renders in software
    cap_add: ["SYS_PTRACE"]                 # debugger only; dev only
    security_opt: ["seccomp=unconfined"]    # bwrap needs user namespaces; dev only
    init: true
    command: >
      dbus-run-session -- watchexec --restart --exts rs,blp,css,xml,toml
      -- cargo run -p steward

  # Unit and integration tests.
  test:
    build: { <<: *desktop-build, target: dev }
    volumes:
      - .:/workspace
      - cargo-registry:/usr/local/cargo/registry
      - cargo-target:/var/cache/cargo-target
    tmpfs: ["/home/dev:uid=${HOST_UID:-1000},gid=${HOST_GID:-1000}"]
    security_opt: ["seccomp=unconfined"]
    init: true
    command: docker/scripts/headless-run cargo nextest run --workspace --locked

  # End-to-end tests: the real app, headless, driven by tests/e2e (see §10.2).
  e2e:
    extends: { service: test }
    environment:
      STEWARD_E2E: "1"
      LANG: C.UTF-8
      G_DEBUG: fatal-criticals
    command: >
      docker/scripts/headless-run pytest tests/e2e
      -m "${E2E_MARK:-not manual}" -k "${E2E_K:-test}"
      --junitxml=artifacts/e2e/junit.xml

  # One-off shell with the full toolchain.
  shell:
    extends: { service: test }
    command: bash
    stdin_open: true
    tty: true

volumes:
  cargo-registry:
  cargo-target:
    name: steward-target-${UBUNTU_VERSION:-24.04}   # one build cache per Ubuntu release
  fixture-home:
```

`docker/scripts/headless-run` starts a private D-Bus session and a headless Wayland compositor (mutter by default, weston as the fallback; `docker/scripts/start-compositor`, ADR 0002), then runs its arguments inside them. End-to-end tests do not use it: the harness starts a fresh session per test. On Docker Desktop, which cannot reach the host's display, `make app` runs the app with Qt's VNC display plugin and publishes it on `127.0.0.1:5900`; open it with Remmina (installed on Ubuntu) or any VNC viewer (ADR 0004). The compose file in the repository is the reference; the listing above is the original design.

**Why the app runs against a fake home.** This is a tool that moves and trashes files. In development it must never touch the developer's real Documents. The container's home is the `fixture-home` volume, and its private D-Bus session gives it its own Trash. A bug costs nothing.

**Hot reloading, honestly.** Rust is compiled, so there is no in-place code swap. What you get:

| Change | Feedback |
|---|---|
| `style.css` | Applied live in the running window (debug builds watch the file) |
| QML files, Rust code | `watchexec` rebuilds and relaunches. Incremental builds with the `mold` linker and a persistent target volume keep this to seconds for UI crates |
| Window state | Debug builds reopen the last folder and view, so a relaunch lands where you were |
| Inspecting the interface | `qmllint` in `make lint`; QML warnings in the log; GammaRay can attach for a live object tree **[verify packaging]** |

Keeping `steward-core` separate from the UI crate matters here: a UI edit recompiles only the thin UI crate.

### 9.4 The fixture home folder

`make fixture` runs `docker/scripts/make-fixture-home` to fill the `fixture-home` volume with a deterministic fake home modelled on the reference's demo data:

- A messy **Downloads**: about 60 loose files of mixed kinds, duplicate names like `Invoice (3).pdf`, an unfinished `.crdownload`, a `.deb` for an installed package, files with old access times.
- **Documents** with project folders that trigger each folder-symbol rule (Invoices, Taxes, a Git repository, a "mostly PDFs" folder).
- A project spread over three places, for a workspace.
- Developer caches of known sizes, an old project with `node_modules`, agent config folders and usage logs.
- Sparse files to simulate gigabytes without using disk.

The same recipe drives integration tests (known inputs, asserted plans) and screenshot generation ([§5.4](#54-how-our-own-captures-are-made)).

### 9.5 Website and repository containers

**`docker/web.Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1.7
ARG NODE_IMAGE            # node:22-alpine@sha256:…   (docker/versions.env)
ARG CADDY_IMAGE           # caddy:2-alpine@sha256:…

FROM ${NODE_IMAGE} AS deps
WORKDIR /site
COPY site/package.json site/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci

FROM deps AS dev                       # Astro dev server with hot module reload
CMD ["npx", "astro", "dev", "--host", "0.0.0.0", "--port", "4321"]

FROM deps AS build
COPY site/ .
RUN npx astro build                    # → /site/dist (includes the Pagefind index)

FROM ${CADDY_IMAGE} AS prod
COPY --from=build /site/dist /srv/site
COPY docker/caddy/Caddyfile.preview /etc/caddy/Caddyfile
```

The Caddyfile serves `/srv/site` at `/`, the repository volume at `/apt/`, and answers `/healthz`. `docker/aptrepo.Dockerfile` is a pinned Ubuntu base with `aptly` and `gnupg`; its entry point is `docker/scripts/publish-apt`, which adds every `.deb` in `/in` to the chosen suite, signs it and publishes the static tree into the `aptrepo` volume.

**`compose.yaml`** (base)

```yaml
name: steward

x-logging: &logging
  driver: json-file
  options: { max-size: "10m", max-file: "5" }

services:
  # Production-like static server: the built site plus the apt repository.
  web:
    image: ${REGISTRY}/steward-web:${TAG}
    build:
      context: .
      dockerfile: docker/web.Dockerfile
      target: prod
      args: { NODE_IMAGE: "${NODE_IMAGE}", CADDY_IMAGE: "${CADDY_IMAGE}" }
    read_only: true
    cap_drop: ["ALL"]
    security_opt: ["no-new-privileges:true"]
    init: true
    restart: unless-stopped
    ports: ["127.0.0.1:8080:8080"]
    volumes:
      - aptrepo:/srv/apt:ro
      - caddy-data:/data
      - caddy-config:/config
    tmpfs: ["/tmp"]
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://127.0.0.1:8080/healthz"]
      interval: 5s
      timeout: 3s
      retries: 10
    logging: *logging

  # One-shot publisher: `docker compose run --rm aptrepo`
  aptrepo:
    profiles: ["publish"]
    build: { context: ., dockerfile: docker/aptrepo.Dockerfile }
    environment:
      APT_SUITE: ${APT_SUITE:-dev}
      APT_ORIGIN: ${APT_ORIGIN:-Steward}
    volumes:
      - ./dist:/in:ro
      - aptrepo:/srv/apt
      - aptly-db:/var/lib/aptly
    secrets: [apt_signing_key]
    restart: "no"

volumes:
  aptrepo:
  aptly-db:
  caddy-data:
  caddy-config:

secrets:
  apt_signing_key: { file: "${SECRETS_DIR:-./secrets}/apt_signing_key.asc" }
```

**`compose.override.yaml`** (development; loaded automatically)

```yaml
services:
  site:                                  # Astro dev server; edit a Markdown page and the browser updates
    build:
      context: .
      dockerfile: docker/web.Dockerfile
      target: dev
      args: { NODE_IMAGE: "${NODE_IMAGE}", CADDY_IMAGE: "${CADDY_IMAGE}" }
    ports: ["127.0.0.1:4321:4321"]
    volumes:
      - ./site:/site
      - site-node-modules:/site/node_modules     # keep the container's modules, not the host's

volumes:
  site-node-modules:
```

**`compose.test.yaml`**

```yaml
services:
  # Builds the site and checks every internal link and the search index.
  site-check:
    build:
      context: .
      dockerfile: docker/web.Dockerfile
      target: build
      args: { NODE_IMAGE: "${NODE_IMAGE}", CADDY_IMAGE: "${CADDY_IMAGE}" }
    command: ["npm", "run", "check"]

  # The real user journey: add the repository, install with apt, start the app.
  apt-install-e2e:
    image: ubuntu:${UBUNTU_VERSION:-24.04}
    depends_on: { web: { condition: service_healthy } }
    environment: { REPO_URL: "http://web:8080/apt", APT_SUITE: "${APT_SUITE:-dev}" }
    volumes:
      - ./docker/scripts:/scripts:ro
      - ./secrets/apt_signing_key.pub:/key.pub:ro
    command: ["/scripts/apt-install-e2e"]
```

**`compose.prod.yaml`** (only when self-hosting instead of using a static host)

```yaml
services:
  web:
    cap_add: ["NET_BIND_SERVICE"]
    ports: ["80:80", "443:443", "443:443/udp"]
    environment: { SITE_DOMAIN: "${SITE_DOMAIN}" }
    volumes:
      - ./docker/caddy/Caddyfile.prod:/etc/caddy/Caddyfile:ro
    deploy: { resources: { limits: { cpus: "1.0", memory: 256M } } }
```

### 9.6 Configuration

**Environment variables** (`.env.example`; none is secret)

| Variable | Example | Used by |
|---|---|---|
| `UBUNTU_VERSION` | `24.04` | Desktop build matrix, apt-install test |
| `HOST_UID`, `HOST_GID` | set by the Makefile from `id -u`, `id -g` | Desktop dev containers (file ownership) |
| `REGISTRY`, `TAG` | `ghcr.io/<org>`, `dev` | Image names |
| `APT_SUITE`, `APT_ORIGIN` | `dev`, `Steward` | Repository publishing |
| `SITE_DOMAIN` | `example.dev` | Self-hosting only |
| `SECRETS_DIR` | `./secrets` | Compose secrets |
| `E2E_MARK` | `not manual` | Which end-to-end tests run |
| `*_IMAGE`, `*_VERSION` | from `docker/versions.env` | Pinned bases and tools |

**Secrets.** There is one: the **apt signing key**. `make init` generates a throwaway key pair for development and CI. The real project key is created once by a maintainer, kept in the CI secret store, and never placed in the repository, an image or a developer machine. Its public half is published on the download page.

**Networking.** One default network. In development only the site (4321) and the preview server (8080) are published, both bound to `127.0.0.1`. The desktop containers publish nothing.

**Volumes.** `aptrepo` (published repository), `aptly-db` (repository state), `caddy-data` and `caddy-config`, and the development caches (`cargo-registry`, `steward-target-<release>`, `site-node-modules`, `fixture-home`). In production only `aptly-db` and `aptrepo` matter, and both can be rebuilt from the released `.deb` files.

**Start-up order.** `apt-install-e2e` waits for `web` to be healthy. `docker compose up --wait` is the readiness check in CI.

### 9.7 Daily workflows

Every command is a `make` target so nobody (human or agent) memorises Compose flags.

| Task | Command | What it runs |
|---|---|---|
| First-time setup | `make init` | Copies `.env.example`, generates the dev signing key, builds images, creates the fixture home |
| Run the desktop app | `make app` | Docker Engine: the window opens on your desktop. Docker Desktop: open `vnc://127.0.0.1:5900` (ADR 0004). Restarts on save |
| Run it as 26.04 | `make app UBUNTU_VERSION=26.04` | Same, different base and build cache |
| QML warnings | `make app QML_DEBUG=1` | |
| Reset the fake home | `make fixture` | Rebuilds the `fixture-home` volume |
| Unit and integration tests | `make test` | `cargo nextest` under a headless session |
| End-to-end tests | `make e2e` | The real app, headless, driven by `tests/e2e` ([§10.2](#102-test-layers-and-the-end-to-end-harness)) |
| One end-to-end test | `make e2e k=E4_03` | pytest `-k` filter |
| Lint | `make lint` | rustfmt, clippy, qmllint, desktop-file and AppStream validation, hadolint, `docker compose config` |
| **The gate** | `make check` | lint + test + e2e on the default release |
| The gate, both releases | `make check-all` | `make check` for 24.04 and 26.04 |
| Toolchain shell | `make shell` | Bash in the dev image |
| Build the `.deb` | `make deb` | `docker build --target artifact --output dist/` |
| Install test | `make deb-test` | Builds the `install-test` stage, then runs the smoke end-to-end tests against the installed binary |
| Website dev server | `make site` | Astro on :4321 with hot reload |
| Preview site and repository | `make preview` | Builds and starts `web` on :8080 |
| Publish to the local repository | `make apt-publish` | Runs the `aptrepo` service on `dist/*.deb` |
| apt-install end-to-end test | `make apt-e2e` | `compose.test.yaml` |
| Regenerate screenshots | `make shots` | Runs the capture scenarios; writes to `site/src/assets/shots/` |
| Follow logs | `make logs s=web` | `docker compose logs -f` |
| Stop and clean | `make down`, `make clean` | `clean` also removes caches and volumes |

**Debugging**

- *Desktop app:* QML warnings in the app log; `G_MESSAGES_DEBUG=all` for GLib logs; `make shell` then `rust-gdb` (the dev service has `SYS_PTRACE`); `sqlite3` on the fixture home's databases; `RUST_BACKTRACE=1` is on by default.
- *End-to-end failures:* each failed test leaves a screenshot, the accessibility tree as text, the app's JSON state and the app log in `artifacts/e2e/<test-id>/`. Read those before changing code.
- *Containers:* `docker compose ps` for health; `docker compose exec web sh` for the preview server.

**Logging.** The app logs to stderr through GLib's structured logging, with a per-module domain (`steward-ops`, `steward-index`, …), so `G_MESSAGES_DEBUG=steward-ops` isolates one area. Never log file contents; log paths only at debug level. Containers log to stdout, rotated by the `json-file` driver (10 MB × 5).

### 9.8 CI and deployment

```
pull request ─▶ lint ─▶ test + e2e matrix (24.04, 26.04) ─▶ deb + install-test + smoke e2e
              └▶ site-check (build, links, search index)
              └▶ image scan (Trivy), cargo audit, cargo deny

merge to main ─▶ all of the above
              ─▶ publish .deb files to apt suite "staging"
              ─▶ deploy the website preview

release tag  ─▶ rebuild from the tag ─▶ full e2e on both releases ─▶ apt-install e2e
              ─▶ sign and publish to apt suite "stable"; attach .deb files to the release
              ─▶ deploy the website (manual approval)
```

- **Deploying the site** is uploading the static build to the host, or, when self-hosting, `docker compose -f compose.yaml -f compose.prod.yaml up -d --wait`.
- **Rollback** of the site is redeploying the previous build. Rollback of a bad app release is publishing a higher version number with the fix or the revert; apt never downgrades on its own.
- **Repository integrity.** The published tree is reproducible from the release `.deb` files and the signing key, so the repository itself needs no backup beyond those two.

### 9.9 Docker practices applied

| Practice | Where |
|---|---|
| Multi-stage builds | Both Dockerfiles; the web image contains only Caddy and the static files, and the `.deb` is exported from a `scratch` stage |
| Small runtime images | The web image is Caddy on Alpine plus static files |
| Layer caching for dependencies | `npm ci` before copying sources; BuildKit cache mounts for apt, Cargo and npm; a persistent Cargo target volume per Ubuntu release in development |
| Reproducible builds | Base images pinned by **digest** in `docker/versions.env`; `rust-toolchain.toml`; `--locked` everywhere; `package-lock.json`; tool versions pinned; `SOURCE_DATE_EPOCH` set from the commit time; consider Ubuntu's apt snapshot service to pin system packages **[verify]** |
| Least privilege | The web container has a read-only root filesystem, all capabilities dropped and `no-new-privileges`. Desktop dev and test containers run as the host user, never root |
| Secrets | One secret, the apt signing key, mounted as a Compose secret file; never in an image, a build argument or the environment |
| Health checks and ordering | `depends_on` with conditions; `up --wait` |
| Least network | Only loopback ports are published in development; desktop containers publish none |
| Scanning and provenance | Trivy on images, hadolint on Dockerfiles, SBOM and provenance attestations from buildx |
| `.dockerignore` | Excludes `target/`, `node_modules/`, `dist/`, `secrets/`, `.git/`, `fileosmac/` |
| Dev-only relaxations are isolated | `SYS_PTRACE`, unconfined seccomp and bind mounts exist only in `compose.desktop.yaml` and `compose.override.yaml`, never in the base or production files |

**Known limits of the Docker workflow**

- USB phones cannot be exercised conveniently in a container. Phone code is developed against the fake `PhoneSource` and verified on a real machine or VM.
- The Shell extension, the tray icon and global shortcuts depend on the host's GNOME Shell. They are verified on the host or in a VM, per Ubuntu release.
- Inside the container the app cannot read the host's light/dark setting through the desktop portal, so "follow the system" shows the light theme there; the explicit Light and Dark settings work everywhere.

---

## 10. Implementation plan: phases and end-to-end tests

This section is written to be executed by an AI coding agent (Claude Opus) working one phase at a time, with a human maintainer for the few steps an agent cannot do. Each phase lists numbered tasks with a "done when" condition, the end-to-end scenarios that must pass, the steps reserved for a human, and a gate.

### 10.1 Working rules for the agent

Copy these into `AGENTS.md` at the repository root in Phase 0.

1. **One phase at a time, tasks in order.** Do not start a phase until the previous phase's gate has passed. One task per commit, with the task ID in the message (`P4-T03: Tidy plan dialog`).
2. **Start and end every session with `docs/PROGRESS.md`.** Read it first, together with the current phase's section here and the ADRs. Before stopping, record what is done, what is next and what is blocked.
3. **Everything runs through `make` in Docker.** Do not install toolchains on the host. Do not run the app outside the dev container.
4. **Never touch a real home folder.** In development and test builds the app refuses to start unless its home contains the marker file `.steward-fixture` (task P0-T05). Do not remove or bypass that guard.
5. **Write the test with the task.** Each end-to-end scenario below is implemented in the same commit series as the feature it covers. A task whose "done when" names a scenario is not done until that scenario passes.
6. **The gate is `make check-all`.** It runs lint, unit and integration tests, and *every* end-to-end scenario written so far, on every supported release: Ubuntu 26.10 during development, with 24.04 and 26.04 added once the project is complete (ADR 0007). Earlier phases' scenarios are the regression suite; they stay green.
7. **Never weaken a test to make it pass.** If a scenario is wrong, fix the scenario and say why in `docs/PROGRESS.md`. No `sleep` in tests; wait for a condition with a timeout.
8. **When a [verify] item turns out false,** write an ADR in `docs/adr/` with what was found, apply the fallback this document names, and continue. If no fallback is named, stop and ask the human.
9. **Stop at steps marked HUMAN.** These need real hardware, a real desktop session, a naming or legal decision, or a secret. List them in `docs/PROGRESS.md` and hand over.
10. **All file changes go through `steward-core::ops`.** No other module may move, copy, rename, trash or delete. A static check enforces this (scenario E2-13).
11. **Keep the test hooks stable.** Every user command is a named `GAction`. Every interactive widget has an accessible name. All user-visible strings go through gettext. Tests run with `LANG=C.UTF-8`.
12. **Do not copy from the reference.** No text, icons, images or demo data from [fileosmac/](fileosmac/). It is there to look at.

### 10.2 Test layers and the end-to-end harness

| Layer | What it proves | Where | Runs with |
|---|---|---|---|
| L1 Unit | Pure logic: planners, parsers, ranking, rules | `crates/*/src`, inline tests | `make test` |
| L2 Integration | `steward-core` against a real filesystem: operations, undo, index, suggesters | `crates/core/tests/`, on a fresh copy of the fixture home | `make test` |
| **L3 End-to-end** | **The real app binary, driven like a user, changes the real (fixture) filesystem correctly and shows the right thing** | `tests/e2e/` | `make e2e` |
| L4 Packaged | The `.deb` installs on a clean system, the installed app passes the smoke scenarios, and `apt install` from the repository works | `install-test` stage, `compose.test.yaml` | `make deb-test`, `make apt-e2e` |
| L5 Manual | What cannot be automated in a container: real phones, the real GNOME Shell, drag and drop between apps, screen reader | Checklist per phase, marked HUMAN | A person, on real Ubuntu 24.04 and 26.04 |

**How an end-to-end test runs**

```
pytest (in the dev container)
  └─ fixture "app": for each test
       1. copy the fixture home into a fresh tmpfs home (with .steward-fixture)
       2. start a private D-Bus session and a headless Wayland compositor
       3. start steward-indexer and steward with STEWARD_E2E=1
       4. yield a driver object; afterwards collect artifacts and stop everything
```

**How a test drives and observes the app**

| Need | Mechanism | Notes |
|---|---|---|
| Trigger a command | Activate the app's or window's `GAction` over D-Bus (`org.gtk.Actions`, exported by the app through GIO, ADR 0009) | The same actions back the menus, shortcuts and the CLI, so this exercises the real code path |
| Click a control, type text, read what is on screen | The accessibility tree (AT-SPI), by role and accessible name | This is also what a screen reader sees, so these tests double as accessibility checks |
| Read precise state | `io.github.m_umair_khan_dev.steward.Test1.State()` returns JSON: current folder, visible items, selection, active pane, open dialog, toast text, queue | Compiled in only with the `e2e` cargo feature; never in release builds |
| Wait for background work | `Test1.WaitIdle(timeout_ms)` returns when the operations queue and the indexer are idle | Replaces sleeps |
| Capture the window | `Test1.Screenshot(path)` renders the window to PNG from inside the app | Needs no screenshot permission from the compositor |
| Check results on disk | Plain Python on the temporary home, plus `home.snapshot()` (paths, sizes, hashes) for before-and-after comparison | The primary assertion for anything that changes files |
| Control time | `STEWARD_FAKE_NOW` shifts the app's clock | File birth time cannot be set, so "older than 90 days" rules are tested by moving "now" forward |
| Stand in for the system | Fake executables on `PATH` (`dpkg-query`, `snap`, `flatpak`, `sudo`, `pkexec`, editors, terminals) that return recorded output and log every call | Proves what the app asked for, and that it never ran a privileged or mutating command |
| Real key presses | `app.press("ctrl+q")`: keys injected through mutter's remote-desktop interface (ADR 0003) | Needs the mutter compositor (the default). Pointer input and drags are not automated yet and stay on the HUMAN checklist |

**Conventions.** Test IDs match this document: `tests/e2e/test_p4_downloads.py::test_E4_03_tidy_then_undo`. Each test is marked with its phase (`@pytest.mark.p4`); a few are also marked `smoke` (run against the installed `.deb`) or `shots` (produce website screenshots). Every test starts from a clean home and may not depend on another test.

**Example**

```python
@pytest.mark.p4
@pytest.mark.smoke
def test_E4_03_tidy_then_undo(app, home):
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")

    app.action("win.tidy")
    plan = app.wait_dialog("Tidy Downloads")
    assert plan.count_rows(group="Will move") == home.manifest["tidy"]["moves"]
    plan.click_button("Tidy")

    app.wait_idle()
    assert app.toast() == "Tidied 18 files into 6 folders"
    assert (home / "Downloads/PDFs/Statement-0923.pdf").exists()
    app.screenshot("tidy-done")

    app.action("win.undo")
    app.wait_idle()
    assert home.snapshot("Downloads") == before      # byte-for-byte back to the start
```

### 10.3 Phase overview

| Phase | Goal | Size | Depends on |
|---|---|---|---|
| 0 | Foundations, test harness, blocking spikes | M | – |
| 1 | Browse: a read-only file manager | L | 0 |
| 2 | File operations and Undo | L | 1 |
| 3 | Index, folder sizes, search, Peek | L | 2 |
| 4 | Smart Downloads, Dual pane, Workspaces | L | 2, 3 |
| 5 | Smart Clean, Installed, About This Computer | L | 2, 3, 4 (duplicate finder) |
| 6 | **Release 1.0**: polish, packaging, repository, website | L | 1–5 |
| 7 | Developer and content tools | L | 6 |
| 8 | Around the desktop: Shelf, tray, search provider, phone | L | 6 |
| 9 | Reach: Flatpak, more languages | M | 6 |

Phases 7, 8 and 9 are independent of each other and can run in any order after 1.0. Sizes are relative (S, M, L).

**Why this order.** The reference shipped browsing, Tidy, Smart Clean, workspaces and dual pane first, and added search and the phone in 1.0.3. We follow that, except that the index comes earlier because folder sizes need it. The operations engine (Phase 2) is the critical path: Tidy, Smart Clean, dual pane and phone import are all built on it.

---

### Phase 0: Foundations, harness and spikes

> Phase 0 was completed with the original GTK plan. The Qt/QML equivalents of its spikes (accessibility tree, real keys, headless rendering) were re-verified in the Qt proof before the switch (ADR 0009).

**Goal.** `make init && make app` shows an empty window from a container; `make check-all` runs a real end-to-end test on both Ubuntu releases; the unknowns that block Phases 1 to 6 are settled.

| ID | Task | Done when |
|---|---|---|
| P0-T01 | Repository skeleton: licence file, README, CONTRIBUTING, `AGENTS.md` (rules from §10.1), `docs/PROGRESS.md`, ADR template, `.gitignore`, `.dockerignore` | Files exist; `AGENTS.md` matches §10.1 |
| P0-T02 | Cargo workspace with empty crates `core`, `app`, `indexer`, `cli`; `rust-toolchain.toml`; shared lint settings | `cargo build --workspace --locked` succeeds in the container |
| P0-T03 | `docker/desktop.Dockerfile`, `compose.desktop.yaml`, `docker/versions.env`, Makefile targets `init`, `app`, `shell`, `test`, `lint`, `check`, `check-all` | Images build for 24.04 and 26.04; every package name confirmed |
| P0-T04 | Hello window (built with GTK; the interface moved to Qt/QML in Phase 1, ADR 0009): `AdwApplication` with app ID, single instance, a header bar, an empty sidebar and content area; Blueprint compiled into a GResource; `style.css` reloads live in debug builds; actions `app.quit` and `app.about` | `make app` shows the window on the host |
| P0-T05 | Fixture home: `docker/scripts/make-fixture-home` builds a deterministic fake home ([§9.4](#94-the-fixture-home-folder)) and writes `manifest.json` describing it (counts, sizes, expected Tidy moves, planted duplicates). The app's fixture guard (rule 4) | Same inputs give byte-identical output; guard has a unit test |
| P0-T06 | `docker/scripts/headless-run` and the end-to-end harness in `tests/e2e/`: `app` and `home` fixtures, action helper, accessibility helpers, the `Test1` D-Bus interface behind the `e2e` feature, artifact collection on failure | Scenarios E0-01 to E0-05 pass |
| P0-T07 | CI workflow: lint, test and e2e on both releases; upload `artifacts/` | Green run on a pull request |
| P0-S1 | **Spike:** exact GTK, libadwaita, GtkSourceView and WebKitGTK versions on 24.04 and 26.04; confirm each widget named in [§6.1](#61-desktop-ui) exists at the baseline | ADR with a version table. *Fallback:* replace a missing widget with its older equivalent |
| P0-S2 | **Spike:** headless compositor command on each release | ADR. *Fallback:* `weston --backend=headless` |
| P0-S3 | **Spike:** is the GTK 4 accessibility tree visible to AT-SPI in the headless session, and can real key and pointer input be injected (Mutter's remote-desktop D-Bus interface)? | ADR. *Fallback for the tree:* drive through `Test1` only. *Fallback for input:* as in §10.2 |
| P0-S4 | **Spike:** GUI from the container on the host's Wayland session, with and without a GPU | ADR. *Fallback:* `GSK_RENDERER=cairo` |
| P0-S5 | **Spike:** `bwrap` inside the dev and CI containers | ADR. *Fallback:* in containers only, run extractors unsandboxed behind an explicit test-only switch; the sandbox stays mandatory in release builds |
| P0-S6 | **Spike:** GIO Trash and file monitors in the container's private session on a tmpfs home | ADR. *Fallback:* use an ext4 loop volume for the test home |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E0-01 | Launch headless | A window exists; the accessibility tree contains a header bar and a sidebar; no critical warnings in the log (`G_DEBUG=fatal-criticals`) |
| E0-02 | Activate `app.about`, then close it | About dialog appears with the app name and version; closes |
| E0-03 | `Test1.Screenshot` | A PNG of the window's size that is not a single flat colour |
| E0-04 | Start with a home that lacks `.steward-fixture` | The app exits with a clear error and touches nothing |
| E0-05 | Activate `app.quit` | Process exits with code 0 within 2 s |

**HUMAN.** Confirm or replace the codename and app ID. Confirm the licence ([§11.2](#112-open-decisions-for-the-product-owner)). Create the remote repository and enable CI. Run `make app` once on the host and confirm the window appears.

**Gate.** `make check-all` green; six spike ADRs merged; `docs/PROGRESS.md` updated.

---

### Phase 1: Browse (a read-only file manager)

**Goal.** Open any folder, see it as cards or a list with real previews, move around by mouse, keyboard and path bar. Nothing in this phase changes a user file.

| ID | Task | Done when |
|---|---|---|
| P1-T01 | `core::folder`: asynchronous folder listing into a `gio::ListStore` of `FileEntry` objects (name, size, times, MIME, kind, icon); sort and hidden-file filter models | L2 test lists the fixture home and matches the manifest |
| P1-T02 | Kind table: MIME type to kind (Images, Videos, PDFs, Documents, Archives, Installers, Code, Other), one place, used everywhere | Unit tests for every MIME type in the fixture |
| P1-T03 | Folder view in QML (§3.3, §3.4): sections "Folders N" and "Files N" with glass folders and file cards (a virtualised list of card rows), list view with columns (Name, Size, Kind, Modified, Created); view switch; three sizes | E1-02, E1-08 |
| P1-T04 | Navigation: open, back, forward, up, history; window title and header follow | E1-03 |
| P1-T05 | Path bar: breadcrumbs; `Ctrl+L` entry accepting paths with `~` and Tab completion | E1-04 |
| P1-T06 | `app.db` with a migration runner; sidebar with Places (XDG folders), Devices (`GVolumeMonitor`, mount and eject), computer row with free space; reorder and hide, persisted | E1-10 |
| P1-T07 | Grouping: None, Kind, Created, with section headers and counts; content header summary; filter chips with counts; status line | E1-01, E1-05 |
| P1-T08 | Thumbnails: read the shared FreeDesktop cache; generate with system thumbnailers inside `bwrap`; load for visible items only, cancel on scroll | E1-09 |
| P1-T09 | Preview pane: file facts; for a folder, its direct children (the two-level tree arrives with the index) | E1-03 asserts pane content |
| P1-T10 | Live refresh with `GFileMonitor`; hidden files toggle; sort options; per-folder view memory | E1-06, E1-07 |
| P1-T11 | Open with the default app; Open With chooser | E1-12 |
| P1-T12 | GSettings schema; Preferences dialog with General and Appearance pages (default view, row size, colour scheme) | E1-02 (persistence), E1-13 |
| P1-T13 | States: empty folder, permission denied, folder gone, slow device spinner | E1-11 |
| P1-T14 | Primary menu, keyboard shortcuts window, About dialog | E1-14 |
| P1-T15 | **Qt/QML front end** (ADR 0009): presenter bridge (cxx-qt), controller in plain Rust, GIO actions exported on D-Bus, bus name and single instance, Test1 interface, frameless window chrome with our own header and window buttons, our own icon set, `Theme.qml` light and dark | All earlier scenarios pass on the Qt build |
| P1-T16 | Reference look (ADR 0008): inset sidebar with app row and section headings, header with nav pill, segmented grouping and view controls, breadcrumb row with hint, chip row and summary, glass folders with counts, file cards with badges, right pane folder overview ("in all", WHAT TAKES THE SPACE) | Screenshots reviewed against `fileosmac/screenshots/smart-downloads.png` and `design.png` (HUMAN sign-off) |
| P1-T17 | "Filter this folder" field (`Ctrl+F`) | E1-16 |
| P1-T18 | Collapsible sidebar: icon rail; slides over the content on hover | E1-17 |
| P1-T19 | Motion (§3.5): files fly into kind folders and back; hover lift; sliding segmented control; toast spring; reduced motion respected | E1-18 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E1-01 | Open Downloads | Item count and status line match the manifest; header summary shows folder and file counts |
| E1-02 | Switch grid and list; change row size; restart the app | Each change is visible in state; all three survive the restart |
| E1-03 | Enter a folder, go back, forward, up | Current folder and breadcrumb segments correct at each step; preview pane shows the selected item's facts |
| E1-04 | `Ctrl+L`, type `~/Doc`, Tab, Enter | Completes to `Documents/` and navigates there |
| E1-05 | Group by Kind | Section names and per-section counts equal the manifest |
| E1-06 | Toggle hidden files | `.config` appears, then disappears |
| E1-07 | The test creates, then deletes, a file in the open folder from outside | The item appears, then disappears, each within 2 s, without a reload |
| E1-08 | Open a folder of 10,000 files | Opens; fewer than 300 row widgets exist (virtualisation); time to first paint is recorded in the report |
| E1-09 | Open a folder of images | Cards carry textures; thumbnail files appear in `~/.cache/thumbnails` |
| E1-10 | Click Documents in the sidebar; hide an entry; restart | Navigates; the entry stays hidden |
| E1-11 | Open a folder with mode 000 | An explanatory page, not a crash or an empty view |
| E1-12 | Open a text file | The fake handler on `PATH` is called with that file's URI |
| E1-13 | Screenshot in light and in dark | Window background differs between the two; no element keeps a light background in dark |
| E1-14 | Open the shortcuts window | Every action in [Appendix A](#appendix-a-keyboard-map) that exists so far is listed |
| E1-15 | Read-only proof: run E1-01 to E1-14, then compare | The fixture home is unchanged apart from the app's own database, settings and caches |
| E1-16 | `Ctrl+F`, type `invoice`, then Escape | Only the matching items remain and the status line counts them; Escape restores the whole folder |
| E1-17 | Collapse the sidebar; open Documents from the rail; expand | The rail lists the same entries as icons; Documents opens; the full sidebar returns. (Hover expansion is on the HUMAN list) |
| E1-18 | In Downloads, group by Kind with motion on; capture frames during the change | Cards move towards their kind folders over several frames (not a jump); the end state shows "By kind N" folders; with reduced motion the change is immediate |

**HUMAN.** Compare the running app with the reference captures (§3) and sign off the look. On a real 26.10 desktop: window move and resize, hover effects and the sidebar hover expansion, text scaling at 200 %. Drag a file out of Steward into Files. Plug in and eject a USB drive.

**Gate.** `make check-all` green.

---

### Phase 2: File operations and Undo

**Goal.** Every way of changing files, all through one engine, all undoable. This is the foundation for Phases 4, 5 and 8.

| ID | Task | Done when |
|---|---|---|
| P2-T01 | `core::ops`: operation types (copy, move, rename, new folder, trash, restore), batches, a queue with one worker, progress, cancel | L2 tests for each operation |
| P2-T02 | Journal tables and migrations ([§7.1](#71-local-backend-inside-the-desktop-app)); undo and redo; the rules "never overwrite" and "partial batch" | L2 tests incl. every undo edge case |
| P2-T03 | Actions and UI: new folder (`Ctrl+Shift+N`), rename in place (`F2`), move to Trash (`Delete`), duplicate | E2-01, E2-02 |
| P2-T04 | Clipboard: copy, cut, paste using `text/uri-list` and `x-special/gnome-copied-files` | E2-03, E2-04, E2-08 |
| P2-T05 | Drag and drop inside the app and with other apps: move by default, `Ctrl` copies | L2 for the logic; HUMAN for the gesture |
| P2-T06 | Clash dialog: keep both, replace, skip, "apply to all". Replace sends the old file to the Trash so it can be undone | E2-05 |
| P2-T07 | Queue UI: progress button in the header, list of running and waiting jobs, Cancel | E2-07 |
| P2-T08 | Toasts with Undo; `Ctrl+Z` and `Ctrl+Shift+Z`; the Edit menu names what will be undone | E2-01 to E2-04 |
| P2-T09 | Trash view: list, restore, Empty Trash with a confirming dialog | E2-09 |
| P2-T10 | Context menus for items, selection and background; multi-selection rules | Covered by scenarios using them |
| P2-T11 | Failure handling: disk full, permission denied, source vanished, cross-filesystem moves, symlinks handled as links | E2-10, L2 tests |
| P2-T12 | Crash safety: a batch left `running` at start-up is marked `partial` and reported | E2-11 |
| P2-T13 | Static check that only `core::ops` calls GIO's mutating file functions; wired into `make lint` | E2-13 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E2-01 | New folder; rename a file; undo both | Each change on disk, then reverted |
| E2-02 | Trash three files; Undo | Files leave the folder and appear in the Trash; Undo restores the same paths and modification times |
| E2-03 | Copy two files, paste into another folder; Undo | Copies exist; Undo removes only the copies |
| E2-04 | Cut and paste; Undo | Files moved, then moved back |
| E2-05 | Paste where the name exists; try each choice | Keep both gives `name (2).ext`. Replace puts the old file in the Trash and Undo brings it back. Skip changes nothing |
| E2-06 | Move a file; the test then creates a file with the old name; Undo | The app reports one file it could not put back; nothing is overwritten; the batch is `partial` |
| E2-07 | Copy a 2 GB sparse file; Cancel midway | Progress is reported; after Cancel no partial file remains; journal consistent |
| E2-08 | Clipboard formats **[verify in headless]** | A copy from the app offers both formats with correct URIs; pasting a `gnome-copied-files` cut entry moves the file. *Fallback:* unit-test the encoder and decoder; live interop goes to HUMAN |
| E2-09 | Empty Trash: cancel, then confirm | Cancel keeps everything. Confirm deletes; Undo is then unavailable for those items |
| E2-10 | Paste into a read-only folder | An error toast; nothing changed anywhere |
| E2-11 | Kill the app during a 200-file move; restart | The app reports the interrupted batch; every file is at either its source or its destination, none lost or duplicated |
| E2-12 | Move a file; restart; Undo | The undo works after the restart |
| E2-13 | Static check | No mutating file call outside `core::ops` |
| E2-14 | A symlink to a folder is trashed | Only the link goes; the target folder is untouched |

**HUMAN.** Drag files between this app and Files (Nautilus) in both directions; copy in one and paste in the other.

**Gate.** `make check-all` green.

---

### Phase 3: Index, folder sizes, search and Peek

**Goal.** Folders show their size and item count; `Ctrl+K` finds any folder or file by name; Space previews.

| ID | Task | Done when |
|---|---|---|
| P3-T01 | `steward-indexer`: crawl configured roots at idle priority; batched writes; exclusion rules | L2: entry count equals the manifest |
| P3-T02 | `index.db` schema with the trigram name index ([§7.1](#71-local-backend-inside-the-desktop-app)); SQLite bundled so both releases behave the same | L2 |
| P3-T03 | Folder size and count roll-up, updated incrementally | E3-02 |
| P3-T04 | Watching with inotify; fall back to timed re-scans when the watch limit is hit; the app tells the indexer about its own operations | E3-05, E3-06 |
| P3-T05 | Indexer control over D-Bus: Status, Pause, Resume, Rescan, WaitIdle; systemd user unit for installed builds (in containers the app starts it directly) | E3-11, E3-12 |
| P3-T06 | Folder cards show count and size; preview pane shows the two-level tree for folders | E3-02 |
| P3-T07 | Search overlay on `Ctrl+K`: field, kind chips with counts, sections (Folders first), thumbnails, footer key hints | E3-03 |
| P3-T08 | Query engine: prefix before substring, recent before old; cancels the previous query on each keystroke | L1 ranking tests; E3-10 |
| P3-T09 | Result keys: arrows, Enter opens, Space peeks, `Ctrl+Enter` shows in its folder | E3-04 |
| P3-T10 | Path bar accepts a folder *name* and lists every match with its location | E3-09 |
| P3-T11 | Peek overlay: images, PDF (scroll, "page n of m"), text and code, audio and video; Info panel; arrows move through the folder | E3-08 |
| P3-T12 | Preferences page "Search and index": roots, exclusions, pause, rebuild, status; pause on battery; removable and network mounts excluded by default | E3-11 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E3-01 | Start, wait idle | Index entry count equals the manifest |
| E3-02 | Open Documents | Three named folder cards show the manifest's size and item count |
| E3-03 | `Ctrl+K`, type `invo` | Expected names present; folders listed before files; chip counts correct |
| E3-04 | Keep typing; Enter on a file; reopen and `Ctrl+Enter` on another | Results narrow; the fake handler opens the first; the app shows the second's folder with it selected |
| E3-05 | The test creates a file outside the app | Search finds it within 5 s |
| E3-06 | Rename a file in the app | The new name is found; the old name is not |
| E3-07 | Delete `index.db` while stopped; start | The index rebuilds by itself; search works |
| E3-08 | Space on an image, a PDF, a text file and a video; right arrow; Escape | Overlay opens for each; the PDF shows its page count; the arrow moves to the next file; Escape closes |
| E3-09 | `Ctrl+L`, type `Invoices` | Both planted folders of that name are offered, each with its location |
| E3-10 | Query a generated index of one million entries (L2, reported) | 95th-percentile query time recorded; fails if over 50 ms |
| E3-11 | Pause, then resume, in Preferences | Indexer status follows |
| E3-12 | Kill the indexer | Browsing still works; folder cards show a dash instead of a size; search explains that the index is not running and offers to start it |

**HUMAN.** Index a real, large home folder: watch CPU, disk activity and battery behaviour.

**Gate.** `make check-all` green.

---

### Phase 4: Smart Downloads, Dual pane and Workspaces

**Goal.** The three features that most define the product.

| ID | Task | Done when |
|---|---|---|
| P4-T01 | `core::plan`: a plan is a list of steps plus a list of skipped items with reasons; it can be shown, then handed to `ops` as one batch. Injectable clock (`STEWARD_FAKE_NOW`) | L1 |
| P4-T02 | Tidy planner with the rules of [§4.2](#42-signature-features): loose files only, kind folders, skip partial downloads and files newer than 2 minutes, never move folders | L1 against the manifest |
| P4-T03 | Tidy plan dialog and run; one toast, one Undo; `Ctrl+Shift+O` | E4-01 to E4-05 |
| P4-T04 | `core::dupes`: size, partial hash, full hash; hash cache keyed by size and modification time | L2 with planted duplicates |
| P4-T05 | Downloads side panel: space by kind; suggestions for installers already installed (through a `PackageDb` trait with a fake), likely duplicates, files untouched for 90 days | E4-06, E4-07 |
| P4-T06 | Preference: which folder is Downloads | E4-08 |
| P4-T07 | Dual pane: `F3`, two independent panes, active-pane indicator, `Tab`, `F5` copy, `F6` move, footer hints, restore on relaunch | E4-09, E4-10 |
| P4-T08 | Workspaces: model and migrations; sidebar section; create, rename, describe, colour, delete; add items by context menu or drop; page with Overview, Folders, Images, Documents; source labels | E4-11 |
| P4-T09 | Workspace resilience: re-find a renamed item by filesystem ID and inode; missing item shows Locate…; unplugged drive shown greyed with its name; `Alt+1` to `Alt+9` | E4-12, E4-13 |
| P4-T10 | Folder symbols and colours: automatic rules, chooser dialog, stored in extended attributes with a database fallback | E4-14 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E4-01 | Open the Tidy plan for Downloads | Moves equal the manifest's expected list; "Left where they are" lists the `.crdownload` file and the file the test touched a moment ago; no folder is listed |
| E4-02 | Cancel the plan | Downloads is byte-for-byte unchanged |
| E4-03 | Run Tidy; Undo | Files are in kind folders; the toast states the counts; one Undo restores the exact starting state, including removing the folders Tidy created |
| E4-04 | Tidy a second time | The plan says there is nothing to tidy |
| E4-05 | Tidy when a kind folder already holds a file of the same name | "Keep both" naming; Undo still restores exactly |
| E4-06 | Fake package database says the app is installed at the same version; then at an older version | The `.deb` is suggested in the first case and not in the second |
| E4-07 | Switch grouping to Kind and back | Nothing on disk changes |
| E4-08 | Set another folder as Downloads | The sidebar entry and Tidy use it |
| E4-09 | `F3`; navigate the right pane; select on the left; `F5`; `F6`; Undo; `Tab` | Copy, then move, land in the right pane's folder; Undo reverts the move; the active pane switches |
| E4-10 | Restart with dual pane open | Both panes return showing the same two folders |
| E4-11 | Create a workspace; add a folder from Documents, a file from Downloads and a folder from a second mounted volume | The page lists all three with where each lives; nothing on disk moved |
| E4-12 | The test renames one source folder; deletes another; then uses Locate… | The renamed one is found again; the deleted one shows Locate…; pointing it at a new folder repairs it |
| E4-13 | `Alt+1` | The first workspace opens |
| E4-14 | Open Documents; choose a symbol for one folder; rename that folder in the app | "Invoices" has the receipt symbol, the Git project the code symbol, the mostly-PDF folder its symbol; the chosen symbol survives the rename; with attributes disabled by the test switch, the database fallback gives the same result |

**HUMAN.** Tidy a copy of a real, messy Downloads folder and judge the result. Check a workspace item on a real external drive after unplugging and replugging.

**Gate.** `make check-all` green.

---

### Phase 5: Smart Clean, Installed and About This Computer

**Goal.** Recover space safely; see what is installed; see the machine's health. These three share the rule "read first, change only what the user ticked, and only through the Trash".

| ID | Task | Done when |
|---|---|---|
| P5-S1 | **Spike:** is there a trustworthy "last launched" date for apps on Ubuntu? Which system clean-up sizes can be read without root? | ADR. *Fallback:* drop the "Unused apps" card; show "size unknown" for unreadable rows |
| P5-T01 | `Suggester` trait and registry; scan orchestration that reports each card as it finishes | L2 |
| P5-T02 | Disk overview: free space and the segmented bar ([§4.3](#43-smart-clean-on-ubuntu)) | E5-01 |
| P5-T03 | Suggesters, one task each: developer caches (allow-list in `data/clean/dev-caches.toml`), duplicates, installers, stale build output, app leftovers, big files, old screen recordings, old downloads | L2 per suggester against planted data; E5-01 to E5-04 |
| P5-T04 | Overview page: cards in two groups with totals, "Looking…" state, Look Again, "this much could go" | E5-01 |
| P5-T05 | Review page: reason per item, tick and untick, tick all, Peek, show in folder; sticky bar with count and size | E5-04 |
| P5-T06 | Move to Trash as one batch; toast with Undo; Trash size and Empty Trash from this page | E5-05 |
| P5-T07 | "Needs administrator" group: size and a command to copy; nothing is executed | E5-07 |
| P5-T08 | `core::packages`: `PackageSource` trait; sources for apt, Snap, Flatpak, npm, pipx and pip, Cargo, Go, RubyGems, Homebrew; each parser tested on recorded output in `tests/data/` | L1 |
| P5-T09 | Installed view: tool chips, filters (all, apps, you installed, came with others), "why it's here", sizes, Save List, copy the remove command | E5-08, E5-09 |
| P5-T10 | `core::sysinfo`: probes behind a `SysProbe` trait (real and fixture implementations) | L1 with fixture data |
| P5-T11 | About This Computer view ([§4.6](#46-about-this-computer)): Hide Names, Copy as Text; opened from the sidebar's computer row | E5-10 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E5-01 | Open Free Up Space | All cards appear at once as "Looking…", then fill; each total equals the manifest's planted size; the headline equals the sum of ticked items |
| E5-02 | Inspect default ticks | Only allow-listed caches, spare-looking duplicates and qualifying installers are ticked; every "Your call" item is unticked |
| E5-03 | A large planted folder named `cache` that is not on the allow-list | Never listed |
| E5-04 | Review duplicates | One copy per set is marked Keep and cannot be ticked; a filed copy is kept in preference to one in Downloads |
| E5-05 | Move four ticked items to the Trash; Undo | The toast states count and size; items are in the Trash; the bar's "In the Trash" part grows; Undo restores everything |
| E5-06 | A planted symlink inside a cache folder points at Documents | After cleaning, Documents is untouched |
| E5-07 | Click Copy on each "Needs administrator" row | The clipboard holds the exact command; the fake `sudo`, `pkexec`, `apt`, `snap` and `journalctl` logs show no mutating call |
| E5-08 | Installed, with fake tools returning recorded output | Per-tool counts and sizes match; the "You installed" filter matches; Save List writes a file equal to the golden file |
| E5-09 | Installed, with some tools absent from `PATH` | Those tools are simply not shown; no error |
| E5-10 | About, with fixture probes | Every row shows the fixture value; Hide Names masks host and user names; Copy as Text equals the golden text |
| E5-11 | Read-only proof: open Free Up Space, Installed and About, tick nothing, quit | The fixture home is unchanged apart from the app's own data |

**HUMAN.** On a real Ubuntu install: compare Smart Clean sizes with `du`; compare About with Settings; read every suggestion's reason for clarity.

**Gate.** `make check-all` green.

---

### Phase 6: Release 1.0

**Goal.** Something a stranger can install with `apt`, in their language, with a website and a guide.

| ID | Task | Done when |
|---|---|---|
| P6-T01 | Welcome flow, first run only: demo (a messy folder, then sorted), short tour, "make it yours" | E6-01 |
| P6-T02 | "What's New" once after an update, from the release-notes file | E6-02 |
| P6-T03 | Preferences complete: General, Appearance, Shortcuts (list), Search and index | Covered by earlier scenarios plus E6-04 |
| P6-T04 | Translations: extract strings; draft `.po` files for Spanish, French, German, Japanese and Simplified Chinese, marked for human review; a pseudo-locale with long strings | E6-03 |
| P6-T05 | Accessibility pass: names and roles on every control, full keyboard reach, focus order, no colour-only signals | E6-04 |
| P6-T06 | Desktop file with actions (New Window, New Workspace, Free Up Space), AppStream metadata, icons (placeholder until the final icon is supplied) | E6-07 |
| P6-T07 | Optional "use as default file manager": the `FileManager1` D-Bus interface and the MIME handler setting | E6-09 |
| P6-T08 | Packaging: `cargo-deb` metadata, systemd user unit for the indexer, one `.deb` per release, `lintian` clean | E6-05, E6-07 |
| P6-T09 | apt repository tooling: `aptrepo` image, `publish-apt`, development key, `apt-install-e2e` script | E6-06 |
| P6-T10 | Website: Astro project; landing page with the sections of [§5.2](#52-recordings-and-stills); guide (outline in [Appendix B](#appendix-b-what-was-inspected), written in our own words for what is built so far); changelog; download; contribute; privacy; search | E6-08 |
| P6-T11 | Screenshot generator: scenarios marked `shots` write light and dark captures for the site and the AppStream metadata | `make shots` output is used by the site build |
| P6-T12 | Release workflow in CI ([§9.8](#98-ci-and-deployment)); `compose.yaml`, `compose.test.yaml`, `compose.prod.yaml` | A dry-run tag publishes to a `ci` suite |
| P6-T13 | "Report a Bug" opens the issue tracker with version and release filled in; `SECURITY.md`, code of conduct | Manual check |
| P6-T14 | Performance pass against [§8.4](#84-performance); results in `docs/perf.md` | Each target met, or the miss explained with an issue filed |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E6-01 | First start; go through the welcome; start again | Welcome shows once; the demo step shows the sorted state; the second start goes straight to the window |
| E6-02 | Start with a stored "last version" lower than the current one; start again | "What's New" shows once |
| E6-03 | For each of the six languages: start, visit the main screens, capture | The app starts; every message has a translation; screenshots saved for human review; the pseudo-locale run produces captures with no crash |
| E6-04 | Accessibility audit over every main screen | No interactive element lacks a name; every action is reachable through the accessibility tree; the shortcuts window lists every bound key |
| E6-05 | Build the `.deb` for each release; install in a clean container; run the `smoke` scenarios against the installed binary | All pass |
| E6-06 | Publish to the local repository; a clean container adds the source and key and runs `apt-get install`; publish a higher version; `apt-get upgrade` | Install succeeds; upgrade brings the new version |
| E6-07 | `desktop-file-validate`, `appstreamcli validate`, `lintian` | No errors |
| E6-08 | Site: build; link check; search for a known word; load the landing page in a headless browser and toggle the theme | No broken links; search returns the page; theme switches |
| E6-09 | With the option on, call `FileManager1.ShowItems` for a fixture file | The app opens that folder with the file selected |
| E6-10 | Full regression | Every scenario from E0 to E5 green on 24.04 and 26.04 |

**HUMAN.** Final name and icon. Review the translations (or mark unreviewed languages as beta). Create the real signing key and put it in the CI secret store. Choose hosting and DNS. Install from the `staging` suite on real 24.04 and 26.04 machines and use it for a day. One pass with the Orca screen reader. Tag the release.

**Gate.** `make check-all`, `make deb-test` and `make apt-e2e` green; HUMAN list signed off.

---

### Phase 7: Developer and content tools

**Goal.** The features that make it a developer's daily tool, plus Markdown, Bookmarks and search inside files.

| ID | Task | Done when |
|---|---|---|
| P7-S1 | **Spike:** where each coding agent keeps its configuration and usage logs on Linux | ADR listing paths per tool. *Fallback:* support only the tools verified |
| P7-T01 | `core::git` with `gix`: branch and count of uncommitted changes; the git chip in the path bar | E7-01, E7-02 |
| P7-T02 | Open In: detect terminals and editors from desktop files; `F4` and `Shift+F4`; Preferences page; add your own app | E7-03 |
| P7-T03 | Path-bar commands (`code .`, `zed`, `terminal`, …) | E7-03 |
| P7-T04 | `core::agents`: projects, agent files, MCP servers, token usage; parsers tested on sample data | L1; E7-04 |
| P7-T05 | Local servers: listening ports matched to the user's processes; Stop with a confirmation | E7-05 |
| P7-T06 | AI Agents view | E7-04, E7-05 |
| P7-T07 | Markdown view: Preview, Edit, Split, outline, Save and Revert; preview with scripts and network disabled | E7-06 |
| P7-T08 | Bookmarks: model, collections, tags, favourites; four views; paste and drop a link; metadata fetch with an off switch | E7-07 |
| P7-T09 | Content search: sandboxed text extraction (text, code, Markdown, PDF, OpenDocument, Office); the Contents switch; matched line with the word in bold | E7-08, E7-09 |
| P7-T10 | `steward` command-line tool and matching app actions: open folder, open workspace, new workspace, add link, open Free Up Space | E7-10 |
| P7-T11 | Customisable shortcuts: change, clear, restore defaults, conflict warning | E7-11 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E7-01 | Open the fixture Git project with three changed files | The chip shows the branch and "3"; the repository is byte-for-byte unchanged afterwards |
| E7-02 | Open a non-repository; open a folder with a corrupt `.git` | No chip; no crash |
| E7-03 | `F4`; then type `code .` in the path bar | The fake terminal, then the fake editor, is started with the current folder |
| E7-04 | Open AI Agents | Projects, MCP server counts and token totals equal the manifest |
| E7-05 | The test starts a local web server; open AI Agents; Stop, cancel; Stop, confirm | The server is listed with its port; cancel leaves it running; confirm ends it |
| E7-06 | Open a Markdown file; Preview, Edit, type, Save, Revert, Split | Outline lists the headings; Save changes the file; Revert restores; a remote image in the document causes no request to the test's local server |
| E7-07 | Add a link pointing at the test's local server; then again with fetching off | First: a card with the page's title and picture. Second: no request is made and the card shows the address only. Both survive a restart |
| E7-08 | Search with Contents on for a word planted inside a PDF, a document and a text file | Each is found with the matching line |
| E7-09 | A planted file that makes extraction hang; a test helper that tries to open a network connection from inside the sandbox | Extraction times out and the app stays responsive; the connection is refused |
| E7-10 | `steward open-workspace`, `steward add-link`, `steward open` against the running app | Each has its effect in the running window |
| E7-11 | Rebind Tidy; restore defaults; bind a key already in use | The new key is in the key-to-action table; defaults return; the conflict is reported |

**HUMAN.** Try Open In with the terminals and editors actually installed. Check the AI Agents numbers against a real machine's usage.

**Gate.** `make check-all` green.

---

### Phase 8: Around the desktop

**Goal.** Reach beyond the window: the Shelf, the tray icon, desktop search and the phone. Much of this depends on the real desktop, so this phase has the largest HUMAN share.

| ID | Task | Done when |
|---|---|---|
| P8-S1 | **Spike (with a human on real 24.04 and 26.04):** global shortcut through the portal versus a GNOME custom keybinding; tray icon behaviour; can a Shell extension place the Shelf window top-centre and keep it above; can it see a file drag begin | ADR per question. *Fallbacks:* custom keybinding; no tray on desktops without the extension; compositor-placed Shelf; no open-on-drag |
| P8-S2 | **Spike (with a human and real phones):** Android over MTP and iPhone over PTP through GVfs: listing speed, thumbnails, dates, reliability | ADR. *Fallback:* ship Android only |
| P8-T01 | Shelf window: Holding, numbered pinned folders with counts, latest downloads, quick access, result line with Undo; `app.shelf` action; `steward shelf` | E8-01 to E8-04 |
| P8-T02 | Shortcut registration by the route chosen in P8-S1; Preferences page "Shelf and tray" | E8-07 |
| P8-T03 | Tray icon (StatusNotifierItem) that toggles the Shelf; open at login (autostart entry), off by default | E8-06 |
| P8-T04 | GNOME Shell search provider for workspaces, collections and links, with a switch per kind | E8-05 |
| P8-T05 | Optional Shell extension in `shell-extension/`, if P8-S1 supports it; D-Bus protocol with the app; the app works identically without it | E8-10; HUMAN |
| P8-T06 | `core::phone`: `PhoneSource` trait, fake source, GVfs source; listing cache per device | L2 with the fake |
| P8-T07 | Phone view ([§4.7](#47-your-phone)): day sections, chips, picked bar, Copy to folder or workspace, "On this computer" badge | E8-08 |
| P8-T08 | Import as an `ops` batch: copy only, Undo, skip already-imported items, survive an unplug | E8-08, E8-09 |

**End-to-end scenarios**

| ID | Scenario | Expected |
|---|---|---|
| E8-01 | Activate `app.shelf`; press `2` | The Shelf window opens listing pinned folders with item counts; the main window opens the second folder |
| E8-02 | Drop a file on a pinned folder (through the Shelf's drop action, since drags cannot be automated); Undo | The file moves; the result line names it; Undo moves it back |
| E8-03 | Put two files in Holding; restart; Clear | Nothing on disk changes; Holding still lists them after the restart; Clear empties the list and the files still exist |
| E8-04 | Latest downloads row | The five newest files in Downloads, excluding folders, hidden files and partial downloads |
| E8-05 | Call the search provider over D-Bus with a workspace's name, then activate the result | The workspace's ID is returned; activating opens it |
| E8-06 | A test stand-in for the panel's tray watcher | The app registers an item; activating it toggles the Shelf |
| E8-07 | With no shortcut portal available | The app writes a custom keybinding to the (container's) settings, and removes it when the feature is turned off |
| E8-08 | Fake phone: open it; pick three items; Copy to a folder; open the phone again; Undo | Items grouped by day with correct chip counts; three files arrive with identical bytes; they now show "On this computer" and a second import skips them; Undo removes the copies; the fake phone recorded no write |
| E8-09 | Fake phone disconnects mid-import | The batch is reported as partial; no truncated file is left |
| E8-10 | Extension: lint and package | Builds for each supported GNOME Shell version |

**HUMAN.** On real 24.04 and 26.04 desktops: the Shelf shortcut from another app; dragging files into Holding and out into another app; the tray icon; Activities search finds a workspace; the extension's placement. With a real Android phone and a real iPhone: connect, list, import, re-import, unplug midway.

**Gate.** `make check-all` green; HUMAN list signed off on both releases.

---

### Phase 9: Reach

**Goal.** More users through more channels.

| ID | Task | Done when |
|---|---|---|
| P9-T01 | Capability detection: when sandboxed, features that cannot work say why instead of showing nothing | E9-01 |
| P9-T02 | Flatpak manifest and build | E9-02 |
| P9-T03 | Hosted translation workflow; further languages | New languages pass E6-03 |
| P9-T04 | Optional: browser extension that sends the current page to Bookmarks through `steward add-link` | Manual check |

| ID | Scenario | Expected |
|---|---|---|
| E9-01 | Run with the sandbox simulated by the test switch | Installed and the affected Smart Clean cards show an explanation; everything else works |
| E9-02 | Build the Flatpak **[verify: needs a privileged builder or a VM in CI]** and run the `smoke` scenarios inside it | Pass, apart from scenarios tagged as unavailable in the sandbox |

**HUMAN.** Flathub submission and review. Decide on other distributions.

**Gate.** `make check-all` green.

---

## 11. Risks and open decisions

### 11.1 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| cxx-qt or the Qt bridge blocks something later (an API we need is missing) | Rework in the bridge | The presenter keeps the bridge small; logic lives in plain Rust; small C++ additions are allowed through cxx-qt when needed |
| A bug in file operations destroys user data | Severe; ends trust | Single operations engine, plan-then-run, Trash-only deletion, journal, never overwrite on undo, the fixture-home guard, end-to-end tests that compare the filesystem before and after |
| The headless end-to-end approach does not work as planned | Agents cannot verify their own work | Settled first, in Phase 0 (spikes P0-S2 and P0-S3), with named fallbacks. If the accessibility tree is unusable, tests drive through the `Test1` interface alone |
| An agent "fixes" a failing test by weakening it | False confidence | Rule 7; filesystem snapshots as the main assertion; human review of any change under `tests/e2e/` |
| Wayland blocks the Shelf's best behaviours | The showcase feature is weaker than on Mac | Scheduled last among desktop features; extension optional; scope decided by spike P8-S1 |
| iPhone import breaks with new iOS versions | Feature unreliable | Android first; iPhone labelled best-effort; isolated behind `PhoneSource` |
| GNOME API drift across Ubuntu releases | Build or runtime breaks | Two-release gate on every task; baseline at 24.04; runtime checks for newer widgets |
| Smart Clean suggests something the user needed | Trust | Strict allow-lists, a reason on every item, conservative ticks, Trash and Undo, no root actions, scenario E5-03 |
| Index cost on huge or network home folders | Battery and disk complaints | Idle priority, pause on battery, network and removable mounts excluded by default, visible status and pause switch |
| Too close to the original's look or wording | Legal and reputational | Own name, icon, copy and captures ([§1.3](#13-name-brand-and-what-we-may-not-copy)); rule 12 |
| Sustaining an open-source project | Stalls after launch | Low running cost by design (static hosting only); contributor docs and the Docker workflow lower the barrier to entry |

### 11.2 Open decisions for the product owner

| # | Question | Recommendation | Effect |
|---|---|---|---|
| O1 | **Source licence** | GPL-3.0-or-later. It suits the Linux desktop, is compatible with Qt (LGPL/GPL), and the PDF text tools are GPL | Licence file and headers in Phase 0 |
| O2 | Final name, domain and app ID | Decide before Phase 6; the codename is fine until then | Replace `steward` and `io.github.m_umair_khan_dev.steward` everywhere |
| O3 | Where the code and the site are hosted | Any Git host with CI and static hosting | CI syntax only |
| O4 | Is the Shell extension in scope | Decide after spike P8-S1 | Phase 8 only |
| O5 | Support GNOME on other distributions | Ubuntu first; Flatpak (Phase 9) is the route to others | Affects the Installed sources and packaging |
| O6 | Add a Launchpad PPA as a second channel | Optional. Its builders have no network access, so Rust dependencies must be vendored into the source package **[verify]** | Extra packaging work in Phase 6 or later |

---

## Appendix A: keyboard map

Where the reference's key conflicts with an Ubuntu convention, the convention wins. All are changeable in Preferences › Shortcuts.

| Action | Reference (macOS) | Ubuntu version | Reason for any change |
|---|---|---|---|
| Find anything | ⌘K | `Ctrl+K` | – |
| Type a path or folder name | ⌘L | `Ctrl+L` | Same as Files |
| Split into two panes | ⌘D | `F3` | `Ctrl+D` is "bookmark this folder" in Files; `F3` is the traditional split key |
| Copy / move to other pane | F5 / F6 | `F5` / `F6` | – |
| Switch pane | Tab | `Tab` | – |
| Tidy this folder | ⌥⌘T | `Ctrl+Shift+O` ("organise") | `Ctrl+Alt+T` opens a terminal system-wide on Ubuntu |
| Undo last file action | ⌘Z | `Ctrl+Z` | – |
| Open a workspace | ⌃1–⌃9 | `Alt+1`–`Alt+9` | `Ctrl+1` and `Ctrl+2` switch list and grid in Files |
| New workspace | ⌥⌘N | `Ctrl+Alt+N` | – |
| Peek | Space | `Space` | – |
| Open folder in terminal | ⌥⇧⌘T | `F4` | Common Linux file-manager key |
| Open folder in editor | – | `Shift+F4` | – |
| The Shelf, from any app | ⌥Space | `Ctrl+Alt+Space` | `Alt+Space` is the window menu |
| Row size | ⌘+ / ⌘− | `Ctrl++` / `Ctrl+−` | – |
| Add link to Bookmarks | ⇧⌘L | `Ctrl+Shift+L` | – |
| Open Smart Clean | ⌥⌘K | `Ctrl+Shift+K` | – |
| Hidden files | ⇧⌘. | `Ctrl+H` | Files convention |
| Rename | Return | `F2` | Files convention |
| Move to Trash | ⌘⌫ | `Delete` | Files convention |
| Back / forward / up | ⌘[ / ⌘] / ⌘↑ | `Alt+←` / `Alt+→` / `Alt+↑` | Files convention |
| New folder | ⇧⌘N | `Ctrl+Shift+N` | – |
| Preferences | ⌘, | `Ctrl+,` | – |
| Show in folder (from search) | ⌥Return | `Ctrl+Return` | `Alt+Return` is Properties in Files |
| Copy instead of move while dragging | hold ⌥ | hold `Ctrl` | Linux drag convention |

---

## Appendix B: what was inspected

| Source | What it gave |
|---|---|
| Landing page HTML (980 KB) and extracted text | All feature copy, comparison table, pricing, FAQ, demo data |
| `styles.css` (189 KB) | Colour tokens, fonts, easing ([§2.3](#23-observed-design-language)) |
| 18 recorded animations and 2× stills | Screen layouts and interactions ([§3.4](#37-screens), [§5.2](#52-recordings-and-stills)) |
| Guide index and pages: install, buy-and-activate, smart-clean, drop-shelf, settings, folder-symbols, around-your-mac, troubleshooting | Onboarding steps, licence behaviour, Smart Clean rules, Shelf and Holding behaviour, Settings structure, folder-symbol rules, menu bar, Spotlight and Shortcuts actions, edge-case rules for Tidy and Undo |
| Changelog | Release order; search and iPhone arrived in 1.0.3 |
| Privacy page and page source | Third parties: Dodo Payments, Resend, Google Fonts, a self-run analytics collector |
| This workstation | Ubuntu 26.10, GNOME Shell 51, Wayland, Qt 6.11 (dev image), GTK 4.24, libadwaita 1.10, Docker 29.9, Compose 5.5; both GNOME Terminal and Ptyxis installed; no Rust toolchain on the host |
| Sibling repositories in this workspace | The team already works in Rust workspaces, GNOME Shell extensions, Makefiles and Docker Compose |

**Guide pages of the reference** (a ready outline for our own guide): install · buy-and-activate · window · browse · peek · markdown · path-bar · copy-move · rename-trash-undo · sidebar · folder-symbols · smart-downloads · smart-clean · workspaces · bookmarks · dual-pane · drop-shelf · git · open-in · packages · ai-agents · about-this-mac · settings · around-your-mac · shortcuts · troubleshooting.

**Not inspected:** the app binary itself (no Mac available here), the guide pages not listed above, and the terms and refunds pages. Where this document describes the original's internals, it is inferring from the site and guide.
