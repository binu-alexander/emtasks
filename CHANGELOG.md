# Changelog

All notable changes to emTasks. Versions follow [semantic versioning](https://semver.org).

## 0.1.1 — 2026-10-14

### Fixed

- **The Show menu is back to three clear choices:** All tasks, Uncompleted and Completed. It also held the "keep completed tasks for 3 days / 1 week" option, so two items looked selected at once and the menu had to scroll. That option is now only in **Settings**.

## 0.1.0 — 2026-10-14

First public release.

### Organize

- **Eisenhower matrix** with an **Inbox** for unsorted tasks, plus a **List** view (⌘L).
- **Add tasks** from the top box or straight into a quadrant with ➕. Paste several lines to create several tasks, and add `#tags` inline.
- **Drag** a task from anywhere on it into another box; a plain click only selects it. Maximize a box with ⛶, and hide the Inbox with ⌘I.
- **Task details:** due date and time or all-day, repeat every N days/weeks/months/years, notes, a linked task, and tags. Click any tag you've already used to add or remove it.
- **Sort and filter** by manual order, title or due date; by status; and by tags. Save combinations as presets. A warning bar appears when a filter hides tasks.
- **Completed tasks** can hide right away, or stay in view for 3 days or 1 week.
- **Two counters:** all-time progress at the top right ("All time: X of Y done"), and what's on screen now in the status bar ("Showing N open · M done").

### Quiet by design

- No notifications, pop-ups or reminders. Overdue tasks are marked in red.
- Tasks move to Urgent by themselves as their deadline gets close, or when a linked task is completed (marked ⚡).

### Your data

- Autosave with atomic writes, a full backup on every launch (newest 20 kept), and undo for the last 100 changes.
- A daily CSV backup in `Documents/emTasks` (newest 30 days kept). CSV export and import, with protection against spreadsheet formulas.
- A recovery screen if the data file is ever damaged, one running copy at a time, and a crash log.

### Look

- Five themes: Classic, Daylight, Paper, Midnight and Graphite. 14pt text and contrast-tested colours.
- `emTasks <version> by B. Alexander` in the bottom-right corner.

### Platform

- macOS 11 or later on Apple Silicon. Works offline, with no account and no network access.
- Signed with the hardened runtime (not yet notarized). Each release comes with a SHA-256 fingerprint.
- Dependencies are audited against the RustSec advisory database.
