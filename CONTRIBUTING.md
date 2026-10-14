# Contributing to emTasks

Thanks for helping. This guide covers building, testing and releasing emTasks. For what the app does and why, read [README.md](README.md) and [SPEC.MD](SPEC.MD).

## Reporting a bug

Open an issue describing:

- what you did,
- what you expected, and
- what happened instead.

Include your macOS version and the emTasks version (bottom-right corner of the window). If emTasks crashed, attach `~/Library/Application Support/emTasks/crash.log`. Please **don't** attach your data file; it contains your tasks.

## Building from source

emTasks is written in Rust with [egui/eframe](https://github.com/emilk/egui). The Rust toolchain lives **inside the project** (in `.rust/`, which isn't committed), much like a Python virtual environment. Nothing is installed system-wide.

**First-time setup** (needs internet once, about 300 MB):

```sh
export RUSTUP_HOME="$PWD/.rust/rustup" CARGO_HOME="$PWD/.rust/cargo"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable
cd emtasks && ../.rust/cargo/bin/cargo fetch     # downloads libraries; builds then work offline
```

**Day to day:**

```sh
source ./activate        # use the project's Rust (your prompt shows "(rust)")
cd emtasks
cargo run                # run a development build
cargo test               # run the test suite
cargo clippy --all-targets -- -D warnings
```

In VS Code, **⌘⇧B** runs emTasks, and new terminals use the project's Rust automatically.

> **Your real tasks are used by `cargo run`.** A development build reads and writes the same data folder as the installed app. To experiment safely, give it a throwaway home folder: `HOME=/tmp/emtasks-play cargo run`.

## Tests

The suite covers:

- **Logic and storage:** the task logic, saving and recovery, the one-copy lock, backups, undo, and CSV export/import.
- **The real interface:** headless tests that simulate mouse and keyboard input. They drag between sections, click ➕ and the Inbox buttons, type, paste, drop a CSV file, and check the filter warning bar. Controls such as the Show, Sort and Tags menus, Reset and Settings options are found by name through egui's accessibility tree, clicked, and the tasks on screen are checked.
- **Visual checks:** every symbol renders (no empty boxes), every theme meets contrast standards, and nothing spills off or overlaps at the smallest window size.

Every change should keep `cargo test` and `cargo clippy --all-targets -- -D warnings` clean. CI runs both, plus `cargo audit`, on every push and pull request.

## Project layout

```
.
├── README.md, SPEC.MD, CHANGELOG.md, CONTRIBUTING.md, LICENSE
├── activate                    venv-style script for the project's Rust
├── docs/                      one-page website (GitHub Pages) and screenshots
├── .github/workflows/ci.yml    lint, tests and dependency audit
└── emtasks/
    ├── .cargo/audit.toml       reviewed dependency advisories, with reasons
    ├── src/
    │   ├── main.rs             window setup, crash log
    │   ├── app.rs              app state, task logic, deadlines, shortcuts (+ tests, incl. simulated UI)
    │   ├── model.rs            Task, sections, filters, settings
    │   ├── storage.rs          data file, atomic saves, backups, recovery, one-copy lock, daily CSV (+ tests)
    │   ├── csvio.rs            CSV export / import (+ tests)
    │   ├── history.rs          undo / redo (+ tests)
    │   ├── ui.rs               all screens and dialogs
    │   ├── theme.rs            five themes, fonts (+ contrast tests)
    │   ├── datepicker.rs       calendar popup
    │   ├── timeutil.rs         local dates and repeat maths (+ tests)
    │   └── platform.rs         file picker, Finder, standard folders
    └── packaging/
        ├── package.sh          tests, then builds the .app, .dmg, release .zip and .sha256
        ├── INSTALL.txt         install notes included in the release zip
        ├── make_icon.py        draws the app icon
        ├── icon-1024.png
        └── screenshots/        shoot.sh + sample data for the README images
```

## Making a release

1. Update `version` in `emtasks/Cargo.toml`, then run `cargo build` once so `Cargo.lock` picks up the new version. The release script uses `--locked` and stops if the lock file is out of date.
2. Add the release to [CHANGELOG.md](CHANGELOG.md), and update the version in README.md and SPEC.MD.
3. Run `cargo audit` in `emtasks/`. Any new advisory needs a fix, or a reviewed entry with a reason in `.cargo/audit.toml`.
4. Run `./emtasks/packaging/package.sh`. It runs the tests, then writes `emTasks.app`, `emTasks-<version>.dmg`, `emTasks-<version>.zip` and its `.sha256` to the project root. None of these are committed.
   - **Signed and notarized builds:** you need an Apple Developer account. Store your credentials once with `xcrun notarytool store-credentials emtasks`, then run:
     ```sh
     SIGN_IDENTITY="Developer ID Application: …" NOTARY_PROFILE=emtasks ./emtasks/packaging/package.sh
     ```
     The script then signs, submits, staples and verifies the dmg.
5. If the interface changed, regenerate the screenshots with `./emtasks/packaging/screenshots/shoot.sh`. This needs a release build, and Screen Recording permission for your terminal or editor.
6. Publish the `.zip` and its `.sha256` together. Share the fingerprint separately when you can.

## Design rules worth keeping

These are settled product decisions; see the decisions log in [SPEC.MD](SPEC.MD) for the reasons.

- **Quiet.** No notifications, pop-ups, reminders or login items.
- **Offline.** No networking code. The only copy that can leave the Mac is the daily CSV in Documents, through the user's own iCloud.
- **Never lose data.** Saves are atomic, there's a backup on every launch, and nothing is written from a recovery screen or by a second copy. A damaged file is always kept, never overwritten.
