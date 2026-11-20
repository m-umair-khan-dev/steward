# ADR 0011: No "Unused apps" card; system clean-up sizes read without root

- **Status:** accepted
- **Date:** 2026-11-20
- **Task:** P5-S1
- **Design reference:** DESIGN.md §4.3 (Smart Clean on Ubuntu), Phase 5

## Context

Two questions had to be answered before Smart Clean could be built:

1. Is there a trustworthy "last launched" date for apps on Ubuntu? The
   "Unused apps" card depends on it **[verify]**.
2. Which sizes in the "Needs administrator" group can a normal user read
   without root? A row that cannot be measured must say so rather than
   prompt for a password.

## Findings

Checked on Ubuntu 26.10 (kernel 7.3, GNOME 51), as a desktop user in the
default groups (`adm`, `sudo`, …).

**Last launched.**

- The kernel keeps no launch time. Access times of program files are
  useless: most systems mount with `relatime`, and libraries are read by many
  programs.
- GNOME Shell keeps `~/.local/share/gnome-shell/application_state`, with a
  `score` and a `last-seen` time (Unix seconds) for each app. This is the
  closest thing to a launch date. It is not trustworthy enough to suggest
  removing an app:
  - It counts only graphical apps whose windows the Shell tracked, and only in
    GNOME sessions. Programs started from a terminal and other desktops are
    missing.
  - An app with no entry is ambiguous: never opened, opened before the file
    existed, or opened in another session. The card would suggest removing
    apps that were simply never recorded.
  - The file is written when the session ends. A crash loses the latest
    times.
- Snap, Flatpak and apt record when something was installed or refreshed,
  never when it was last run.

**System clean-up sizes without root.**

| Row | How it is measured | Readable without root? |
|---|---|---|
| apt package cache | `stat` on `/var/cache/apt/archives/*.deb` (the folder is 0755, the files 0644) | Yes |
| Disabled Snap revisions | `snap list --all` (read-only), then `stat` on `/var/lib/snapd/snaps/<name>_<rev>.snap` (files are 0600, but the folder is readable, so `stat` works) | Yes |
| System journal | `journalctl --disk-usage` | Only for members of `adm` or `systemd-journal`. Ubuntu's first user is in `adm`; other users get only their own journal, so the row shows "size unknown" |
| Old kernels | `dpkg-query` for installed `linux-image-*` packages other than the running and newest ones, sizes from `Installed-Size` | Yes |
| Unused Docker images | `docker system df` | Only for members of the `docker` group; otherwise "size unknown" |

## Decision

- **The "Unused apps" card is dropped** (the fallback DESIGN.md names). The
  Shell's usage file may come back later as a hint inside Installed ("not
  opened lately"). It will never be grounds for a suggestion.
- The "Needs administrator" rows are measured as in the table above. When a
  tool is missing, or its output cannot be read, the row shows "size unknown"
  and still offers its command to copy. Nothing in this group is ever run;
  only the read-only queries listed above are.

## Consequences

- Smart Clean has eight cards: three under "Safe to clear" and five under
  "Your call".
- E5-07 checks that the read-only queries (`snap list --all`,
  `journalctl --disk-usage`, `dpkg-query`) are the only calls the fake tools
  record.
- DESIGN.md §4.3's "Unused apps" row stands as written: it said the card
  ships only if this spike found a source.
