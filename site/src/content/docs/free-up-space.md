---
title: Free Up Space
description: What is safe to clear, why, and the Trash as a safety net.
order: 11
section: Tools
---

Open **Free Up Space** from the sidebar or with <kbd>Ctrl+Shift+K</kbd>. The ring shows what fills your home disk; the cards below fill in as Cairn looks.

**Safe to clear** (ticked only when clearly spare):

- *Developer caches* — only folders their tools rebuild by themselves: npm, Yarn, pip, Cargo, Go, Gradle, ccache, Homebrew. Nothing else, however big or whatever it is called.
- *Duplicates* — one copy is always kept, a filed one first. Copies in Downloads or on the Desktop, or named like a copy, are ticked.
- *Installers* — `.deb` files for packages already installed.

**Your call** (never ticked): old build output in projects untouched for six months, data left by removed Flatpaks and Snaps, big files, old screen recordings, and old downloads.

Open a card to review it: each item says why it is there. Tick what should go and press **Move to Trash** — one step, with Undo. **Needs administrator** lists system clean-ups (apt's cache, old Snap revisions, the journal, old kernels, Docker images) with their sizes and the command to copy. Cairn never runs them.

There is no "unused apps" card: Linux keeps no trustworthy record of when an app was last opened.
