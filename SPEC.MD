# emTasks — Spec (as built, v0.1.1)

emTasks is a personal task organizer for macOS based on the **Eisenhower matrix**. The matrix groups tasks by urgency and importance into four groups: do first, schedule, delegate, and eliminate. It's named after Dwight Eisenhower's remark that the urgent is seldom important, and was popularised by Stephen Covey. The app helps you focus on what matters and let go of what can be delegated or dropped.

> This spec records what was actually built, including changes from the original spec and features added during development. User-facing help is in [README.md](README.md); release history is in [CHANGELOG.md](CHANGELOG.md). Last updated 2026-10-14 (v0.1.1).
>
> **Legend:** ✅ built as specified · ✏️ built differently · ❌ dropped by decision · ➕ added beyond the original spec

## Design principles

Settled during the build:

- **A quiet personal diary.** emTasks never interrupts you. It sends no macOS notifications, shows no pop-ups or reminders, and doesn't open at login. Overdue work is shown only visually.
- **Private and offline.** It needs no account or network access. All data stays in one local folder on the Mac, apart from the optional daily CSV backup in Documents, which iCloud syncs if Documents is in iCloud.
- **Your data is safe.** It's saved automatically, every save is written safely so a crash can't corrupt the file, backups are taken on every launch, and changes can be undone.
- **Mac only**, running natively on Apple Silicon.

## Task Organization

- ✅ Two views: Matrix and List of Sections. Toggle with ⌘L.
  - The **Matrix** shows the 2×2 grid, with the Inbox as a column on the left.
  - The **List** shows all sections stacked vertically.
- ✅ An "Inbox" section (originally called "Unsorted") holds incoming tasks until you categorize them.
- ✏️ Move tasks between sections using drag-and-drop. **A task can be grabbed anywhere on its row**, not only by a handle. A drag begins only once the mouse moves; a plain click just opens the task in the details panel and never moves it. Dropping it onto another task places it at that position when sorting is set to Manual.
- ✅ Maximize sections to focus on specific content (⛶; Esc restores).
- ➕ **Hide the Inbox** with **📥 Hide Inbox / Show Inbox (N)** at the far left of the toolbar, View → Show Inbox, or ⌘I. The quadrants expand to the full width, the toolbar button shows how many tasks are waiting, and the setting is saved.
- ➕ Add tasks directly to a quadrant with the **➕** button in its header. Enter adds the task, the box stays open for rapid entry, and Esc closes it. ⌘N adds to the maximized quadrant.
- ➕ Quick triage of the Inbox: 🗑, or selecting a task and pressing Delete, removes it instantly. ⌘Z restores it. Other sections ask before deleting.
- ➕ Move the selected task with ⌘0 (Inbox) and ⌘1–⌘4 (Do / Schedule / Delegate / Eliminate).
- ➕ Two counters with different jobs:
  - At the top right, next to ⚙: **all-time progress**, "All time: X of Y done", for the four quadrants.
  - In the status bar: **what's on screen**, "Showing N open · M done", for the visible sections after filters, so it includes recently completed tasks that are still shown.

## Task Scheduling and Reminders

- ✅ Assign due dates to tasks, by typing the date or using the 📅 calendar picker with Today / Tomorrow / Next week shortcuts.
- ✅ Set "All day task" for whole-day planning. An all-day task becomes overdue only after its day ends.
- ✅ Create recurring tasks with customizable repeat options: every N days, weeks, months or years.
  - Completing one creates the next occurrence, skipping any dates already past.
  - Month-end dates are clamped, so 31 Jan repeats to 28 Feb.
  - The new occurrence returns to the quadrant the task originally came from.
- ❌ ~~Set reminders to never miss an important deadline.~~ **Dropped.** Reminders conflicted with the "quiet diary" principle. They were replaced by quiet visual indicators of overdue tasks:
  - a faint red tint and a thin red bar on the task
  - the due date in red, followed by "· overdue"
  - an "N overdue" count in the section header and the status bar

## Advanced Task Features

- ✏️ Automatically move a task to 'Urgent' as its deadline approaches. **This happens silently, with no notifications.** The task is marked ⚡ instead.
  - The window defaults to 24 hours and can be changed in Settings, where the feature can also be turned off.
  - Importance is kept: *Schedule* moves to *Do first*, and *Eliminate* moves to *Delegate*.
  - Checks run while the app is open. Anything that came due while it was closed is handled on the next launch.
- ✅ Automatically move a task to 'Urgent' when a linked task is completed. Set this in the editor under "Linked task". The move is silent and marked ⚡.
- ✅ Track task creation and completion dates, shown in the task editor.
- ✅ Paste multiline text as individual tasks to speed up input. Leading bullets (`-` `*` `•`) are removed, and this works in both the main add box and the ➕ quadrant boxes.
- ➕ Inline tags while typing: `Call the bank #finance`.
- ➕ Undo and redo (⌘Z / Shift⌘Z) for the last 100 changes. Typing a task's title counts as a single step.

## Sorting, Filtering, and Tags

- ✅ Sort tasks by title, due date, or manually. Completed tasks always sink to the bottom.
- ✅ Use tags to organize tasks effectively. In the editor, every tag already in use is offered as a chip; click to add or remove it.
- ✅ Filter tasks by tags (any of the selected tags) and by status (all / uncompleted / completed).
- ➕ **Completed tasks linger:** in the Uncompleted view, a completed task can hide right away (default), show for 3 days, or show for 1 week after it was completed, counted from its completion time. This is a saved setting, not a filter, so it never triggers the filter warning bar. Set it in Settings.
- ✅ Save and quickly apply custom filter presets for easy navigation (⭐ Presets).

## Appearance ➕

- **Five themes**, chosen under View → Theme or in Settings, which shows a preview card for each:
  - **Classic**: the original look, kept unchanged
  - **Daylight** and **Paper**: light
  - **Midnight** and **Graphite**: dark
- The choice is **saved with the user's data and restored on every launch**. Data files from before themes existed open in Classic.
- Text is 14pt in every theme, including Classic (egui's default of 12.5 read too small).
- The new themes use card-style quadrants with soft shadows and coloured top strips, hover highlights, more generous spacing, and the macOS system font. egui's own fonts remain as a fallback so every icon renders.
- **Readability is tested:** body text has at least 7:1 contrast, and quadrant headings and overdue red at least 3:1, in every new theme.

## Data and reliability ➕

- **Storage:** one file at `~/Library/Application Support/emTasks/emtasks-data.ron`, kept separate from the app so updates never touch it.
- **Safe saving:** changes are saved about a second after editing and again on quit. Each save goes to a temporary file that's flushed to disk and then renamed over the old one.
- **Backups:** one is made on every launch, the newest 20 are kept, and *File → Back up now* makes one on demand.
- **Daily CSV backup:** `~/Documents/emTasks/emTasks-tasks-YYYY-MM-DD.csv`, written the first time the app runs each day and refreshed on quit. The newest 30 days are kept, and other files in the folder are left alone. It's on by default with a Settings toggle. It's never written from a recovery screen, by a second copy, or for an empty task list, so it can't overwrite a good backup with nothing. If Documents syncs with iCloud, so do these, by the owner's choice, for an off-Mac copy.
- **Versioning:** the data file records its format version, and older files are upgraded automatically, including files from earlier builds that had reminders. A file from a newer version is never overwritten.
- **CSV export and import:** File menu, Settings, or drag a `.csv` onto the window.
  - Export writes every task, unaffected by filters, to `~/Downloads/emTasks-tasks-<stamp>.csv`, a backup the user can open and verify themselves.
  - Import only adds tasks: it skips duplicates (same title, quadrant and due date), keeps original created and completed dates, reports problem rows, and can be undone.
  - This is in addition to the automatic backups, which are unchanged.
- **Filter warning:** when a tag or status filter hides tasks, a bar says how many, with **Show all tasks**, so a forgotten filter can't look like lost data.
- **Recovery:** if the file can't be read, a recovery screen offers to restore a backup or start fresh. The damaged file is always kept.
- **One copy at a time:** the data folder is locked while emTasks runs. A second copy shows "emTasks is already open" and never reads or saves the tasks, so two windows can't overwrite each other.
- **Crash log:** if emTasks ever crashes, the details are appended to `crash.log` in the data folder.

## Security ➕

- No networking code or frameworks are in the app. External commands are limited to two fixed calls: the macOS file picker and "Show in Finder". No user text reaches them.
- The app is signed with the hardened runtime, even though the signature is ad-hoc.
- CSV export guards against spreadsheet formula injection: cells starting with `=` `+` `-` `@` are prefixed with `'`, and import reverses it. CSV import refuses files over 20 MB.
- Each release includes a `.sha256` fingerprint file. INSTALL.txt explains how to verify a download.
- Dependencies audited with `cargo audit` (2026-10-13). No advisory affects the macOS build; the reviewed exceptions are recorded in `emtasks/.cargo/audit.toml`, and CI re-runs the audit on every push.

## Exact rules

These are the precise behaviours, as implemented and tested.

| Area | Rule |
| --- | --- |
| **Auto-urgent** | Checked at launch and every 10 seconds while open. An uncompleted task in the Inbox, *Schedule* or *Eliminate* whose deadline is within the window (default 24 h, 1–720 h in Settings) moves to its urgent box: Inbox and *Schedule* go to *Do first*, *Eliminate* goes to *Delegate*. It's marked ⚡ and remembers where it came from. |
| **Deadline** | A timed task's deadline is its due time. An all-day task's deadline is 23:59:59 on its day. A typed date with no time means 09:00, unless the task is all-day. |
| **Overdue** | Uncompleted, and the deadline has passed. |
| **Repeat** | Completing a repeating task creates the next occurrence in the box the task originally came from, skipping occurrences already in the past. Month-end dates are clamped (31 Jan → 28 Feb). |
| **Linked task** | When the linked task is completed, the waiting task moves to its urgent box and is marked ⚡. |
| **Completed tasks** | They always sink to the bottom of their box. In *Uncompleted*, they hide right away, or stay for 3 or 7 days (72 h or 168 h) after completion, depending on the setting. *All tasks* and *Completed* show every completed task. The setting isn't a filter, so it never triggers the warning bar. |
| **Counters** | **Top, all time:** "All time: X of Y done". Y is every task in the four quadrants, completed or not, whatever the filter; X is the completed ones. The Inbox isn't counted. **Bottom, on screen:** "Showing N open · M done". These are the tasks in the sections currently displayed: the maximized one, or every shown section (including the Inbox when it's visible), after the tag/status filter and the completed-tasks setting. Overdue tasks get a separate red count. |
| **Show menu** | Exactly three choices: *All tasks* (every task, completed or not), *Uncompleted* (open tasks, plus recently completed ones if the completed-tasks setting keeps them) and *Completed* (every completed task). Nothing else is in this menu; how long completed tasks stay in *Uncompleted* is set only in Settings. |
| **Filter warning** | Shown when tags or a status other than *Uncompleted* hide tasks that the default view would show. |
| **Autosave** | About 0.8 s after the last change, and on quit. Written to a temporary file, flushed, renamed over the data file, then the folder is flushed. |
| **Full backups** | A copy of the data file at every launch and on *Back up now*. The newest 20 are kept. |
| **Daily CSV** | `~/Documents/emTasks/emTasks-tasks-YYYY-MM-DD.csv`. Written on the first check each day, and rewritten on quit. The newest 30 are kept, and only files matching that exact name pattern are ever pruned. Skipped for an empty task list, from a recovery screen, or by a second copy. |
| **Undo** | The last 100 changes. Typing a title counts as one step. |
| **CSV import** | Only adds tasks. A row is a duplicate if it has the same trimmed, case-insensitive title, the same box and the same due time. Files over 20 MB are refused. |
| **One copy** | The data folder is locked with `flock` while running. A copy that can't get the lock shows "emTasks is already open" and never reads or writes data. |
| **Data versions** | The data file records its format version (currently 1). Older files are upgraded on load. A file from a newer version is never written; the app says so and offers only Quit. |

## Platform and distribution ➕

- **Platform:** macOS 11+ on Apple Silicon. Built in Rust with egui/eframe.
- **Packaging:** `emTasks.app` with its own icon, plus `emTasks-0.1.1.dmg`, both built offline by `emtasks/packaging/package.sh`. The release zip `emTasks-0.1.1.zip` contains the dmg, INSTALL.txt, README.md, SPEC.MD, CHANGELOG.md and LICENSE.
- **Release gate:** `package.sh` runs the full test suite and refuses to build if any test fails. It builds with the exact library versions in `Cargo.lock`.
- **Signing:** ad-hoc signed with the hardened runtime, and not notarized. `package.sh` can sign with a Developer ID and notarize once an Apple Developer account is available (`SIGN_IDENTITY`, `NOTARY_PROFILE`). On another Mac, the first launch needs approval under System Settings → Privacy & Security → Open Anyway.
- **Keyboard:** the full shortcut list is under Help → Keyboard shortcuts.
- **Version:** shown in the bottom-right corner of the window as `emTasks <version> by B. Alexander`. There's no About window.

## Decisions log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-10-12 | macOS only, buildable offline | The owner's requirement. The project's toolchain and libraries are kept inside the project so builds don't need the network. |
| 2026-10-12 | No macOS notifications | The app should be a quiet diary that doesn't "blast notifications". |
| 2026-10-12 | Reminders dropped entirely | Quiet visual indicators of overdue tasks only. |
| 2026-10-12 | No scheduling with macOS and no Calendar integration | Nothing should happen outside the app. |
| 2026-10-12 | No "open at login" option | It existed only to support reminders. |
| 2026-10-12 | Drag from anywhere on a task | Dragging only by the small left-hand handle was hard to find and use. |
| 2026-10-12 | Instant delete in the Inbox | Fast inbox triage, with undo as the safety net. |
| 2026-10-12 | ➕ add-to-quadrant | Tasks whose quadrant is already known shouldn't have to pass through the Inbox. |
| 2026-10-12 | "Unsorted" renamed to "Inbox" | A clearer name. Saved data is unchanged, since only the on-screen name changed. |
| 2026-10-12 | Option to hide the Inbox (toolbar only) | Lets you focus on the four quadrants. A second Hide button in the Inbox header duplicated the toolbar button, so it was removed. |
| 2026-10-12 | Filter warning bar | A forgotten #work filter hid 14 of 19 tasks and looked like data loss. |
| 2026-10-12 | CSV export and import, alongside the existing backups | A plain file the user can open is the backup they trust. |
| 2026-10-12 | Security hardening (CSV guard, import limit, hardened runtime, SHA-256) | Raised in a security review; fixed everything possible offline. |
| 2026-10-12 | Themes: Classic + 2 light + 2 dark, remembered | The stock look felt "dingy". The current look is kept as Classic, and the user picks the theme. |
| 2026-10-13 | Only one running copy at a time (folder lock) | Two copies saving the same file would silently lose whichever copy's changes were saved first. |
| 2026-10-13 | Crash log in the data folder | A released app has no visible console, so crashes otherwise leave no trace. |
| 2026-10-13 | Dependency audit, with reviewed exceptions | The flagged advisories are in Linux-only libraries, or concern an egui font library that's unmaintained but has no known flaw. They're recorded with reasons in `emtasks/.cargo/audit.toml`. |
| 2026-10-13 | Daily CSV backup to Documents, synced by iCloud | The owner wanted an off-Mac copy. They accepted that it can leave the Mac through their own iCloud; it can be turned off. |
| 2026-10-14 | Completed tasks: hide right away, 3 days or 1 week | Seeing what you've just finished is motivating. The default stays "hide right away", so nothing changes until the user chooses. |
| 2026-10-14 | Top counter is all-time progress; bottom counter is what's on screen | The owner's model: the top is the overall history, and the bottom counts what's in the visible quadrants, open and recently closed. Labels say which is which ("All time:", "Showing"). |
| 2026-10-14 | About window replaced by `emTasks <version> by B. Alexander` in the bottom-right corner | Simpler: the version is always visible, and a menu item and window fewer. |
| 2026-10-14 | Pick existing tags with one click in the editor | Retyping tags was slow and caused near-duplicates. |
| 2026-10-14 | Show menu holds only the three status choices | Putting the completed-tasks option in the same menu showed two highlighted items at once, which made the filters look broken. That option lives only in Settings. |
| 2026-10-14 | MIT license | A permissive, widely understood license for a small open-source app. |

## Not in scope yet

These are possible future work:

- Notarization with an Apple Developer ID, to remove the first-launch warning for other users
- An Intel build, as a universal binary
- Automatic updates
- Sync between Macs
- Import and export of linked-task connections
