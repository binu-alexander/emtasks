use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::history::{History, Snapshot};
use crate::model::*;
use crate::storage::{Data, LoadResult, Storage, DATA_VERSION};
use crate::theme::ThemeName;
use crate::timeutil::{self, now};

/// Largest CSV file we'll import (protects against freezing on huge files).
const MAX_IMPORT_BYTES: u64 = 20_000_000;
/// How often deadlines are checked (seconds).
const TICK_SECS: i64 = 10;
/// Save this long after the last change.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(800);

pub struct App {
    pub tasks: Vec<Task>,
    pub next_id: TaskId,
    pub filter: Filter,
    pub presets: Vec<Preset>,
    pub view: View,
    pub theme: ThemeName,
    pub settings: Settings,

    pub ui: UiState,
    /// None in tests.
    pub storage: Option<Storage>,
    pub history: History,
    /// What's on disk, and what we last saw in memory (for debounced saving).
    saved: Option<Data>,
    seen: Option<Data>,
    changed_at: Instant,
    /// Held while running so a second copy of emTasks can't save over our data.
    instance_lock: Option<std::fs::File>,
    /// Daily CSV backups (~/Documents/emTasks); None in tests.
    pub documents_backup_dir: Option<std::path::PathBuf>,
    /// The day the last daily CSV backup was attempted.
    documents_backup_day: Option<String>,
}

/// Transient UI state, not saved.
#[derive(Default)]
pub struct UiState {
    pub selected: Option<TaskId>,
    pub maximized: Option<Section>,
    pub quick_add: String,
    pub focus_quick_add: bool,
    /// Theme currently installed in egui (re-applied when `App::theme` changes).
    pub applied_theme: Option<ThemeName>,
    /// Section whose inline "add task" box is open, and its text.
    pub adding_to: Option<Section>,
    pub section_input: String,
    pub focus_section_input: bool,
    pub status: String,
    pub edit: EditBuf,
    /// Where a dragged task would land: section, and the task to insert before.
    pub drop_target: Option<(Section, Option<TaskId>)>,
    /// Where things were drawn this frame (used by the drag tests).
    pub row_rects: std::collections::HashMap<TaskId, egui::Rect>,
    pub prev_row_rects: std::collections::HashMap<TaskId, egui::Rect>,
    pub section_rects: std::collections::HashMap<Section, egui::Rect>,
    pub add_buttons: std::collections::HashMap<Section, egui::Rect>,
    /// Tag chips in the editor, for tests.
    pub tag_chips: Vec<(String, egui::Rect)>,
    pub inbox_toggle: Option<egui::Rect>,
    /// Rects of every item in each section header (for the overlap test).
    pub header_parts: std::collections::HashMap<Section, Vec<egui::Rect>>,
    pub quick_add_button: Option<egui::Rect>,
    pub settings_button: Option<egui::Rect>,
    pub show_all_button: Option<egui::Rect>,
    /// The quadrant total next to ⚙, and where the toolbar's left-hand items end.
    pub quadrant_total: Option<egui::Rect>,
    pub toolbar_left_end: f32,
    /// Result of the last CSV import, shown until dismissed.
    pub import_report: Option<ImportReport>,
    pub last_tick: i64,
    pub show_settings: bool,
    pub show_shortcuts: bool,
    pub show_save_preset: bool,
    pub preset_name: String,
    pub confirm_delete: Option<TaskId>,
    pub recovery: Option<Recovery>,
    pub save_error: Option<String>,
}

pub struct ImportReport {
    pub file: String,
    pub added: usize,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

pub enum Recovery {
    Corrupt(String),
    TooNew(u32),
    AlreadyRunning,
}

/// Text buffers for editor fields that need parsing.
#[derive(Default)]
pub struct EditBuf {
    pub id: Option<TaskId>,
    pub date: String,
    pub time: String,
    pub tags: String,
    pub error: Option<String>,
}

pub enum Action {
    Select(TaskId),
    SetDone(TaskId, bool),
    StartDrag(TaskId),
    ToggleMaximize(Section),
    Delete(TaskId),
}

impl Default for App {
    fn default() -> Self {
        Self::from_data(Data::default())
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = Storage::new(Storage::default_dir());
        // If the lock can't even be created, saving will fail too and say so; carry on.
        let lock = storage.lock();
        let stop = |recovery| {
            let mut app = Self::default();
            app.ui.recovery = Some(recovery);
            crate::theme::apply(&cc.egui_ctx, app.theme);
            app
        };
        if matches!(lock, Ok(None)) {
            return stop(Recovery::AlreadyRunning);
        }
        let mut app = match storage.load() {
            LoadResult::Loaded(d) => {
                let _ = storage.backup();
                let mut app = Self::from_data(d);
                app.saved = Some(app.to_data());
                app
            }
            LoadResult::Missing => Self::with_samples(),
            LoadResult::Corrupt(e) => {
                let mut app = Self::default();
                app.ui.recovery = Some(Recovery::Corrupt(e));
                app
            }
            LoadResult::TooNew(v) => {
                let mut app = Self::default();
                app.ui.recovery = Some(Recovery::TooNew(v));
                app
            }
        };
        app.storage = Some(storage);
        app.instance_lock = lock.ok().flatten();
        app.documents_backup_dir = Some(crate::platform::documents_backup_dir());
        crate::theme::apply(&cc.egui_ctx, app.theme);
        app.ui.applied_theme = Some(app.theme);
        app
    }

    pub fn from_data(d: Data) -> Self {
        let history = History::new(Snapshot { tasks: d.tasks.clone(), next_id: d.next_id });
        Self {
            tasks: d.tasks,
            next_id: d.next_id,
            filter: d.filter,
            presets: d.presets,
            view: d.view,
            theme: d.theme,
            settings: d.settings,
            ui: UiState::default(),
            storage: None,
            history,
            saved: None,
            seen: None,
            changed_at: Instant::now(),
            instance_lock: None,
            documents_backup_dir: None,
            documents_backup_day: None,
        }
    }

    pub fn to_data(&self) -> Data {
        Data {
            version: DATA_VERSION,
            tasks: self.tasks.clone(),
            next_id: self.next_id,
            filter: self.filter.clone(),
            presets: self.presets.clone(),
            view: self.view,
            theme: self.theme,
            settings: self.settings.clone(),
        }
    }

    /// Compare without cloning.
    fn same_as(&self, d: &Data) -> bool {
        self.tasks == d.tasks
            && self.next_id == d.next_id
            && self.filter == d.filter
            && self.presets == d.presets
            && self.view == d.view
            && self.theme == d.theme
            && self.settings == d.settings
    }

    /// First launch: a few example tasks so the matrix isn't empty.
    fn with_samples() -> Self {
        let mut app = Self::default();
        let today = timeutil::start_of_day(now());
        let samples: [(&str, Section, Option<i64>, &str); 6] = [
            ("Drag me into a quadrant", Section::Unsorted, None, "tutorial"),
            ("Paste several lines into the add box to create many tasks", Section::Unsorted, None, "tutorial"),
            ("Finish quarterly report", Section::Do, Some(today + 17 * 3600), "work"),
            ("Plan next week", Section::Schedule, Some(today + 3 * 86_400), "work"),
            ("Book dentist appointment", Section::Delegate, None, "home"),
            ("Sort old photos", Section::Eliminate, None, "home"),
        ];
        for (title, section, due, tag) in samples {
            let id = app.add_task(title, section);
            if let Some(t) = app.task_mut(id) {
                t.due = due;
                t.tags.insert(tag.to_string());
            }
        }
        app.history = History::new(app.snapshot());
        app.ui.status = "Welcome to emTasks".into();
        app
    }

    pub fn set_theme(&mut self, t: ThemeName) {
        self.theme = t;
        self.ui.status = format!("Theme: {}", t.label());
    }

    // ── Saving ──────────────────────────────────────────────────

    pub fn can_save(&self) -> bool {
        self.ui.recovery.is_none() && self.storage.is_some()
    }

    /// Save shortly after changes stop.
    fn autosave(&mut self, ctx: &egui::Context) {
        if !self.can_save() {
            return;
        }
        if !self.seen.as_ref().is_some_and(|d| self.same_as(d)) {
            self.seen = Some(self.to_data());
            self.changed_at = Instant::now();
        }
        let pending = self.saved != self.seen;
        if pending && self.changed_at.elapsed() >= SAVE_DEBOUNCE {
            self.save_now();
        } else if pending {
            ctx.request_repaint_after(SAVE_DEBOUNCE);
        }
    }

    pub fn save_now(&mut self) {
        if !self.can_save() {
            return;
        }
        let data = self.to_data();
        if self.saved.as_ref() == Some(&data) {
            return;
        }
        let result = self.storage.as_ref().map(|s| s.save(&data));
        match result {
            Some(Ok(())) => {
                self.saved = Some(data);
                self.ui.save_error = None;
            }
            Some(Err(e)) => self.ui.save_error = Some(format!("Couldn't save: {e}")),
            None => {}
        }
    }

    pub fn backup_now(&mut self) {
        self.save_now();
        let result = self.storage.as_ref().map(|s| s.backup());
        self.ui.status = match result {
            Some(Ok(Some(p))) => format!("Backed up to {}", p.display()),
            Some(Ok(None)) => "Nothing to back up yet".into(),
            Some(Err(e)) => format!("Backup failed: {e}"),
            None => String::new(),
        };
    }

    /// Daily CSV copy in ~/Documents/emTasks: once a day while open, and again on quit
    /// (`refresh`). Never written from a recovery screen or for an empty list.
    pub fn documents_backup(&mut self, refresh: bool) {
        if !self.can_save() || !self.settings.documents_backup || self.tasks.is_empty() {
            return;
        }
        let Some(dir) = self.documents_backup_dir.clone() else { return };
        let today = timeutil::fmt_date(now());
        if !refresh && self.documents_backup_day.as_deref() == Some(today.as_str()) {
            return;
        }
        // Mark the day even on failure, so a denied permission isn't retried every few seconds.
        self.documents_backup_day = Some(today.clone());
        if let Err(e) = crate::storage::write_daily_csv(&dir, &today, &crate::csvio::export(&self.tasks)) {
            self.ui.status = format!("Couldn't back up to Documents/emTasks: {e}");
        }
    }

    /// Replace everything with freshly loaded data (after a restore).
    pub fn reload_from(&mut self, d: Data) {
        let storage = self.storage.take();
        let lock = self.instance_lock.take();
        let documents = self.documents_backup_dir.take();
        *self = Self::from_data(d);
        self.storage = storage;
        self.instance_lock = lock;
        self.documents_backup_dir = documents;
        self.saved = Some(self.to_data());
    }

    // ── Undo ────────────────────────────────────────────────────

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { tasks: self.tasks.clone(), next_id: self.next_id }
    }

    pub fn undo(&mut self) {
        match self.history.undo(self.snapshot()) {
            Some(s) => {
                self.restore(s);
                self.ui.status = "Undone".into();
            }
            None => self.ui.status = "Nothing to undo".into(),
        }
    }

    pub fn redo(&mut self) {
        match self.history.redo(self.snapshot()) {
            Some(s) => {
                self.restore(s);
                self.ui.status = "Redone".into();
            }
            None => self.ui.status = "Nothing to redo".into(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.tasks = s.tasks;
        // Never reuse ids.
        self.next_id = self.next_id.max(s.next_id);
        if self.ui.selected.is_some_and(|id| self.task(id).is_none()) {
            self.ui.selected = None;
        }
        self.ui.edit.id = None;
        self.history.set_committed(self.snapshot());
    }

    // ── Task operations ─────────────────────────────────────────

    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    fn index_of(&self, id: TaskId) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    pub fn add_task(&mut self, line: &str, section: Section) -> TaskId {
        let (title, tags) = parse_quick_add(line);
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task { id, title, tags, section, created_at: now(), ..Default::default() });
        id
    }

    /// Adds one task per non-empty line to `section`. Returns how many were added.
    pub fn add_lines(&mut self, text: &str, section: Section) -> usize {
        let lines: Vec<&str> = text
            .lines()
            .map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim())
            .filter(|l| !l.is_empty())
            .collect();
        for l in &lines {
            self.add_task(l, section);
        }
        lines.len()
    }

    pub fn set_completed(&mut self, id: TaskId, done: bool) {
        self.set_completed_at(id, done, now());
    }

    pub fn set_completed_at(&mut self, id: TaskId, done: bool, ts: i64) {
        let Some(idx) = self.index_of(id) else { return };
        {
            let t = &mut self.tasks[idx];
            if t.completed == done {
                return;
            }
            t.completed = done;
            t.completed_at = done.then_some(ts);
        }
        if !done {
            self.ui.status = "Task reopened".into();
            return;
        }
        self.ui.status = "Task completed".into();

        // Recurring: spawn the next occurrence right after this one.
        let t = self.tasks[idx].clone();
        if let (Some(due), true) = (t.due, t.repeat != RepeatUnit::None) {
            // Skip occurrences that are already past, so overdue repeats don't pile up.
            let slack = if t.all_day { 86_399 } else { 0 };
            let mut next_due = timeutil::advance(due, t.repeat, t.repeat_every);
            while next_due + slack < ts {
                next_due = timeutil::advance(next_due, t.repeat, t.repeat_every);
            }
            let next = Task {
                id: self.next_id,
                completed: false,
                completed_at: None,
                created_at: ts,
                due: Some(next_due),
                section: t.auto_from.unwrap_or(t.section),
                auto_from: None,
                wait_for: None,
                ..t.clone()
            };
            self.next_id += 1;
            self.tasks.insert(idx + 1, next);
            self.ui.status = format!(
                "Completed — next \"{}\" due {}",
                t.title,
                timeutil::fmt_due(next_due, t.all_day, ts)
            );
        }

        // Linked tasks waiting on this one become urgent (quietly, ⚡ marker).
        for w in self.tasks.iter_mut().filter(|w| w.wait_for == Some(id) && !w.completed) {
            w.wait_for = None;
            if !w.section.is_urgent() {
                w.auto_from = Some(w.section);
                w.section = w.section.urgent_version();
            }
        }
    }

    pub fn delete_task(&mut self, id: TaskId) {
        let title = self.task(id).map(|t| t.title.clone()).unwrap_or_default();
        self.tasks.retain(|t| t.id != id);
        for t in &mut self.tasks {
            if t.wait_for == Some(id) {
                t.wait_for = None;
            }
        }
        if self.ui.selected == Some(id) {
            self.ui.selected = None;
        }
        self.ui.status = format!("Deleted \"{title}\" — ⌘Z to undo");
    }

    pub fn open_add(&mut self, s: Section) {
        if self.ui.adding_to != Some(s) {
            self.ui.section_input.clear();
        }
        self.ui.adding_to = Some(s);
        self.ui.focus_section_input = true;
    }

    pub fn close_add(&mut self) {
        self.ui.adding_to = None;
        self.ui.section_input.clear();
    }

    /// Keep the keyboard flow going: select the next task in on-screen order.
    fn select_next_after_delete(&mut self, id: TaskId) {
        let order = self.visible_order();
        let next = order
            .iter()
            .position(|x| *x == id)
            .and_then(|p| order.get(p + 1).or(p.checked_sub(1).and_then(|q| order.get(q))))
            .copied();
        self.delete_task(id);
        self.ui.selected = next;
    }

    /// Move a task to `section`, before `before` (manual order) or at the end.
    pub fn move_task(&mut self, id: TaskId, section: Section, before: Option<TaskId>) {
        if before == Some(id) {
            return;
        }
        let Some(idx) = self.index_of(id) else { return };
        let mut t = self.tasks.remove(idx);
        if t.section != section {
            t.section = section;
            t.auto_from = None;
            self.ui.status = format!("Moved to {}", section.title());
        }
        let at = before.and_then(|b| self.index_of(b)).unwrap_or_else(|| {
            self.tasks.iter().rposition(|x| x.section == section).map_or(self.tasks.len(), |i| i + 1)
        });
        self.tasks.insert(at, t);
    }

    /// Tasks for one section, filtered and sorted for display.
    pub fn visible_tasks(&self, section: Section) -> Vec<Task> {
        let mut v: Vec<Task> = self
            .tasks
            .iter()
            .filter(|t| t.section == section && self.filter.matches(t, self.completed_since()))
            .cloned()
            .collect();
        match self.filter.sort {
            SortMode::Manual => {}
            SortMode::Title => v.sort_by_key(|t| t.title.to_lowercase()),
            SortMode::Due => v.sort_by_key(|t| (t.due.is_none(), t.due)),
        }
        // Completed tasks sink to the bottom.
        v.sort_by_key(|t| t.completed);
        v
    }

    /// Completed tasks newer than this still show in the Uncompleted view.
    pub fn completed_since(&self) -> Option<i64> {
        let keep = self.settings.keep_completed.seconds();
        (keep > 0).then(|| now() - keep)
    }

    /// Task ids in on-screen order (for arrow-key navigation).
    pub fn visible_order(&self) -> Vec<TaskId> {
        let sections: Vec<Section> = match self.ui.maximized {
            Some(s) => vec![s],
            None => self.shown_sections(),
        };
        sections.into_iter().flat_map(|s| self.visible_tasks(s)).map(|t| t.id).collect()
    }

    /// Sections on screen (the Inbox can be hidden).
    pub fn shown_sections(&self) -> Vec<Section> {
        Section::ALL.into_iter().filter(|s| self.settings.show_inbox || *s != Section::Unsorted).collect()
    }

    pub fn inbox_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.section == Section::Unsorted && !t.completed).count()
    }

    pub fn set_inbox_visible(&mut self, show: bool) {
        self.settings.show_inbox = show;
        if !show {
            if self.ui.maximized == Some(Section::Unsorted) {
                self.ui.maximized = None;
            }
            if self.ui.adding_to == Some(Section::Unsorted) {
                self.close_add();
            }
        }
        self.ui.status = if show { "Inbox shown".into() } else { "Inbox hidden — ⌘I to show it".into() };
    }

    /// Open tasks a tag/status filter is currently hiding (that would show with no filter).
    pub fn hidden_by_filter(&self) -> usize {
        let plain = Filter { sort: self.filter.sort, ..Filter::default() };
        let since = self.completed_since();
        let shown = self.shown_sections();
        self.tasks
            .iter()
            .filter(|t| {
                shown.contains(&t.section) && plain.matches(t, since) && !self.filter.matches(t, since)
            })
            .count()
    }

    /// Clear tag and status filters (keeps the sort order).
    pub fn show_all_tasks(&mut self) {
        self.filter.tags.clear();
        self.filter.status = StatusFilter::default();
        self.ui.status = "Showing all tasks".into();
    }

    // ── CSV export / import ─────────────────────────────────────

    /// Export every task (whatever the filter) to a CSV file.
    pub fn export_csv_to(&self, path: &std::path::Path) -> std::io::Result<usize> {
        std::fs::write(path, crate::csvio::export(&self.tasks))?;
        Ok(self.tasks.len())
    }

    pub fn export_csv(&mut self) {
        let dir = crate::platform::downloads_dir();
        let path = dir.join(format!("emTasks-tasks-{}.csv", timeutil::fmt_stamp(now())));
        match self.export_csv_to(&path) {
            Ok(n) => {
                self.ui.status = format!("Exported {n} tasks to {}", path.display());
                crate::platform::reveal_in_finder(&path);
            }
            Err(e) => self.ui.status = format!("Export failed: {e}"),
        }
    }

    /// Add tasks from CSV text. Never removes or changes existing tasks;
    /// rows matching an existing task (same title, quadrant and due date) are skipped.
    pub fn import_csv_text(&mut self, text: &str, file: &str) -> ImportReport {
        let parsed = crate::csvio::import(text, now());
        let key = |t: &Task| (t.title.trim().to_lowercase(), t.section, t.due);
        let mut existing: std::collections::HashSet<_> = self.tasks.iter().map(key).collect();
        let (mut added, mut skipped) = (0, 0);
        for mut t in parsed.tasks {
            if !existing.insert(key(&t)) {
                skipped += 1;
                continue;
            }
            t.id = self.next_id;
            self.next_id += 1;
            self.tasks.push(t);
            added += 1;
        }
        self.ui.status = format!("Imported {added} tasks from {file} — ⌘Z to undo");
        ImportReport { file: file.to_string(), added, skipped, warnings: parsed.warnings }
    }

    pub fn import_csv_file(&mut self, path: &std::path::Path) {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size > MAX_IMPORT_BYTES {
            self.ui.import_report = Some(ImportReport {
                file: name,
                added: 0,
                skipped: 0,
                warnings: vec![format!(
                    "The file is {} MB. emTasks imports CSV files up to {} MB, so nothing was imported.",
                    size / 1_000_000,
                    MAX_IMPORT_BYTES / 1_000_000
                )],
            });
            return;
        }
        let report = match std::fs::read(path) {
            Ok(bytes) => self.import_csv_text(&String::from_utf8_lossy(&bytes), &name),
            Err(e) => ImportReport { file: name, added: 0, skipped: 0, warnings: vec![format!("Couldn't read the file: {e}")] },
        };
        self.ui.import_report = Some(report);
    }

    /// (done, total) for tasks in the four quadrants; the Inbox isn't counted.
    pub fn quadrant_progress(&self) -> (usize, usize) {
        let quads = self.tasks.iter().filter(|t| t.section != Section::Unsorted);
        let (done, all) = quads.fold((0, 0), |(d, a), t| (d + t.completed as usize, a + 1));
        (done, all)
    }

    /// (open, completed) tasks on screen now: the shown sections (or the maximized one), after filters.
    pub fn on_screen_counts(&self) -> (usize, usize) {
        let sections = match self.ui.maximized {
            Some(s) => vec![s],
            None => self.shown_sections(),
        };
        sections
            .into_iter()
            .flat_map(|s| self.visible_tasks(s))
            .fold((0, 0), |(o, d), t| if t.completed { (o, d + 1) } else { (o + 1, d) })
    }

    pub fn all_tags(&self) -> BTreeSet<String> {
        self.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect()
    }

    // ── Deadlines (checked quietly while emTasks is open) ───────

    /// Move tasks to Urgent as their deadlines approach. No alerts of any kind;
    /// moved tasks just get the ⚡ marker.
    pub fn tick_at(&mut self, ts: i64) {
        if !self.settings.auto_urgent {
            return;
        }
        let window = self.settings.urgent_hours as i64 * 3600;
        for t in self.tasks.iter_mut().filter(|t| !t.completed && !t.section.is_urgent()) {
            if t.deadline().is_some_and(|d| ts >= d - window) {
                t.auto_from = Some(t.section);
                t.section = t.section.urgent_version();
            }
        }
    }

    // ── Keyboard ────────────────────────────────────────────────

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let press = |k| KeyboardShortcut::new(Modifiers::NONE, k);
        let hit = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        let typing = ctx.wants_keyboard_input();

        if self.ui.confirm_delete.is_some() {
            return; // the dialog handles its own keys
        }
        if hit(cmd(Key::N)) {
            // In a maximized quadrant, add straight into it.
            match self.ui.maximized {
                Some(s) if s != Section::Unsorted => self.open_add(s),
                _ => self.ui.focus_quick_add = true,
            }
        }
        if hit(cmd(Key::Comma)) {
            self.ui.show_settings = true;
        }
        if hit(cmd(Key::I)) {
            self.set_inbox_visible(!self.settings.show_inbox);
        }
        if hit(cmd(Key::L)) {
            self.view = if self.view == View::Matrix { View::List } else { View::Matrix };
        }
        if !typing {
            // Check redo first: ⌘Z also matches ⌘⇧Z logically.
            if hit(cmd_shift(Key::Z)) {
                self.redo();
            } else if hit(cmd(Key::Z)) {
                self.undo();
            }
        }

        if let Some(id) = self.ui.selected {
            if !typing && (hit(press(Key::Delete)) || hit(press(Key::Backspace)) || hit(cmd(Key::Backspace))) {
                // Triage: Unsorted tasks go straight away (⌘Z brings them back).
                if self.task(id).is_some_and(|t| t.section == Section::Unsorted) {
                    self.select_next_after_delete(id);
                } else {
                    self.ui.confirm_delete = Some(id);
                }
            }
            if hit(cmd(Key::Enter)) {
                let done = self.task(id).is_some_and(|t| t.completed);
                self.set_completed(id, !done);
            }
            let targets = [
                (Key::Num0, Section::Unsorted),
                (Key::Num1, Section::Do),
                (Key::Num2, Section::Schedule),
                (Key::Num3, Section::Delegate),
                (Key::Num4, Section::Eliminate),
            ];
            for (k, s) in targets {
                if hit(cmd(k)) {
                    self.move_task(id, s, None);
                }
            }
        }

        if !typing {
            let down = hit(press(Key::ArrowDown));
            let up = hit(press(Key::ArrowUp));
            if down || up {
                let order = self.visible_order();
                let pos = self.ui.selected.and_then(|id| order.iter().position(|x| *x == id));
                let next = match (pos, down) {
                    (None, _) => order.first(),
                    (Some(p), true) => order.get(p + 1).or(order.get(p)),
                    (Some(p), false) => order.get(p.saturating_sub(1)),
                };
                self.ui.selected = next.copied();
            }
        }

        if hit(press(Key::Escape)) {
            if self.ui.adding_to.is_some() {
                self.close_add();
            } else if self.ui.maximized.is_some() {
                self.ui.maximized = None;
            } else {
                self.ui.selected = None;
            }
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_now();
        // Drop the pre-1.0 copy of the data from eframe's own storage.
        storage.set_string(eframe::APP_KEY, String::new());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_now();
        self.documents_backup(true);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl App {
    /// One frame of UI + logic (separate from eframe so tests can drive it).
    pub fn frame(&mut self, ctx: &egui::Context) {
        if self.ui.recovery.is_some() {
            self.ui_recovery(ctx);
            return;
        }

        if self.ui.applied_theme != Some(self.theme) {
            crate::theme::apply(ctx, self.theme);
            self.ui.applied_theme = Some(self.theme);
        }

        let ts = now();
        if ts - self.ui.last_tick >= TICK_SECS {
            self.ui.last_tick = ts;
            self.tick_at(ts);
            self.documents_backup(false);
        }
        ctx.request_repaint_after(Duration::from_secs(TICK_SECS as u64));

        self.handle_shortcuts(ctx);

        // A CSV file dropped onto the window is imported.
        let dropped: Vec<std::path::PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for path in dropped {
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv")) {
                self.import_csv_file(&path);
            } else {
                self.ui.status = "Only .csv files can be imported".into();
            }
        }

        self.ui.drop_target = None;
        self.ui.prev_row_rects = std::mem::take(&mut self.ui.row_rects);
        self.ui.section_rects.clear();
        self.ui.add_buttons.clear();
        self.ui.tag_chips.clear();
        self.ui.inbox_toggle = None;
        self.ui.header_parts.clear();
        self.ui.quick_add_button = None;
        self.ui.settings_button = None;
        self.ui.show_all_button = None;
        self.ui.quadrant_total = None;
        let mut actions = Vec::new();

        self.ui_menu(ctx);
        self.ui_toolbar(ctx);
        self.ui_status_bar(ctx);
        self.ui_editor(ctx);
        let central = if self.theme.is_classic() {
            egui::Frame::central_panel(&ctx.style())
        } else {
            egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::same(12.0))
        };
        egui::CentralPanel::default().frame(central).show(ctx, |ui| {
            self.ui_filter_banner(ui);
            match self.view {
                View::Matrix => self.ui_matrix(ui, &mut actions),
                View::List => self.ui_list(ui, &mut actions),
            }
        });
        self.ui_settings_window(ctx);
        self.ui_save_preset_window(ctx);
        self.ui_shortcuts_window(ctx);
        self.ui_confirm_delete(ctx);
        self.ui_import_report(ctx);

        for action in actions {
            match action {
                Action::Select(id) => self.ui.selected = Some(id),
                Action::SetDone(id, done) => self.set_completed(id, done),
                Action::StartDrag(id) => egui::DragAndDrop::set_payload(ctx, id),
                Action::Delete(id) => self.delete_task(id),
                Action::ToggleMaximize(s) => {
                    self.ui.maximized = if self.ui.maximized == Some(s) { None } else { Some(s) }
                }
            }
        }

        self.handle_drag(ctx);

        let editing = ctx.wants_keyboard_input() || ctx.input(|i| i.pointer.any_down());
        self.history.observe(&self.tasks, self.next_id, editing);
        self.autosave(ctx);
    }
}

impl App {
    fn handle_drag(&mut self, ctx: &egui::Context) {
        let Some(id) = egui::DragAndDrop::payload::<TaskId>(ctx).map(|p| *p) else { return };

        // Floating label that follows the pointer.
        if let (Some(pos), Some(t)) = (ctx.pointer_hover_pos(), self.task(id)) {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            egui::Area::new(egui::Id::new("drag_ghost"))
                .order(egui::Order::Tooltip)
                .fixed_pos(pos + egui::vec2(12.0, 4.0))
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(&t.title));
                });
        }

        if ctx.input(|i| i.pointer.any_released()) {
            if let Some((section, before)) = self.ui.drop_target {
                let before = if self.filter.sort == SortMode::Manual { before } else { None };
                self.move_task(id, section, before);
            }
            egui::DragAndDrop::clear_payload(ctx);
        }
    }
}

/// Parse the editor's date/time fields into the task's due timestamp.
/// A date without a time means 09:00 (unless the task is all-day).
pub fn apply_due(t: &mut Task, date: &str, time: &str) -> Result<(), String> {
    if date.trim().is_empty() {
        t.due = None;
        return Ok(());
    }
    let (y, m, d) = timeutil::parse_date(date).ok_or("Date must be YYYY-MM-DD")?;
    let (h, min) = if t.all_day {
        (0, 0)
    } else if time.trim().is_empty() {
        (9, 0)
    } else {
        timeutil::parse_time(time).ok_or("Time must be HH:MM (24h)")?
    };
    t.due = Some(timeutil::from_local(y, m, d, h, min));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeutil::from_local;

    const HOUR: i64 = 3600;

    fn app() -> App {
        App::default()
    }

    #[test]
    fn add_lines_makes_one_task_per_line() {
        let mut a = app();
        let n = a.add_lines("- Buy milk #home\n\n* Call Bob\n• Email Ann #work #urgent\n   ", Section::Unsorted);
        assert_eq!(n, 3);
        let titles: Vec<_> = a.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Buy milk", "Call Bob", "Email Ann"]);
        assert!(a.tasks[0].tags.contains("home"));
        assert_eq!(a.tasks[2].tags.len(), 2);
        assert!(a.tasks.iter().all(|t| t.section == Section::Unsorted));
        let ids: std::collections::BTreeSet<_> = a.tasks.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 3, "ids must be unique");
    }

    #[test]
    fn completing_recurring_task_creates_next_occurrence() {
        let mut a = app();
        let id = a.add_task("Water plants", Section::Schedule);
        let due = from_local(2026, 1, 10, 8, 0);
        {
            let t = a.task_mut(id).unwrap();
            t.due = Some(due);
            t.repeat = RepeatUnit::Week;
            t.repeat_every = 1;
        }
        a.set_completed_at(id, true, due - HOUR);
        assert_eq!(a.tasks.len(), 2);
        let next = &a.tasks[1];
        assert!(!next.completed);
        assert_eq!(next.due, Some(from_local(2026, 1, 17, 8, 0)));
        assert_eq!(next.section, Section::Schedule);
        assert_ne!(next.id, id);
    }

    #[test]
    fn overdue_recurring_task_skips_past_occurrences() {
        let mut a = app();
        let id = a.add_task("Standup", Section::Do);
        let t = a.task_mut(id).unwrap();
        t.due = Some(from_local(2026, 1, 1, 9, 0));
        t.repeat = RepeatUnit::Day;
        a.set_completed_at(id, true, from_local(2026, 1, 5, 12, 0));
        assert_eq!(a.tasks[1].due, Some(from_local(2026, 1, 6, 9, 0)));
    }

    #[test]
    fn recurring_task_returns_to_its_original_quadrant() {
        let mut a = app();
        let id = a.add_task("Report", Section::Schedule);
        let due = from_local(2026, 1, 10, 8, 0);
        let t = a.task_mut(id).unwrap();
        t.due = Some(due);
        t.repeat = RepeatUnit::Month;
        a.tick_at(due - HOUR); // auto-moved to Urgent
        assert_eq!(a.task(id).unwrap().section, Section::Do);
        a.set_completed_at(id, true, due - HOUR);
        assert_eq!(a.tasks[1].section, Section::Schedule);
    }

    #[test]
    fn reopening_does_not_spawn() {
        let mut a = app();
        let id = a.add_task("x", Section::Do);
        a.task_mut(id).unwrap().due = Some(from_local(2026, 1, 1, 9, 0));
        a.task_mut(id).unwrap().repeat = RepeatUnit::Day;
        a.set_completed_at(id, true, 0);
        a.set_completed_at(id, false, 0);
        assert_eq!(a.tasks.len(), 2);
        assert!(a.task(id).unwrap().completed_at.is_none());
    }

    #[test]
    fn completing_linked_task_makes_waiter_urgent() {
        let mut a = app();
        let blocker = a.add_task("Get approval", Section::Do);
        let waiter = a.add_task("Ship it", Section::Schedule);
        let low = a.add_task("Tidy", Section::Eliminate);
        a.task_mut(waiter).unwrap().wait_for = Some(blocker);
        a.task_mut(low).unwrap().wait_for = Some(blocker);
        a.set_completed_at(blocker, true, 0);
        assert_eq!(a.task(waiter).unwrap().section, Section::Do);
        assert_eq!(a.task(low).unwrap().section, Section::Delegate);
        assert_eq!(a.task(waiter).unwrap().wait_for, None);
        assert_eq!(a.task(waiter).unwrap().auto_from, Some(Section::Schedule));
    }

    #[test]
    fn deadline_moves_task_to_urgent_once() {
        let mut a = app();
        let now = from_local(2026, 3, 1, 12, 0);
        let soon = a.add_task("Soon", Section::Schedule);
        let later = a.add_task("Later", Section::Schedule);
        let done = a.add_task("Done", Section::Schedule);
        let already = a.add_task("Already urgent", Section::Delegate);
        a.task_mut(soon).unwrap().due = Some(now + 2 * HOUR);
        a.task_mut(later).unwrap().due = Some(now + 72 * HOUR);
        a.task_mut(done).unwrap().due = Some(now + HOUR);
        a.task_mut(done).unwrap().completed = true;
        a.task_mut(already).unwrap().due = Some(now + HOUR);

        a.tick_at(now);
        assert_eq!(a.task(soon).unwrap().section, Section::Do);
        assert_eq!(a.task(soon).unwrap().auto_from, Some(Section::Schedule));
        assert_eq!(a.task(later).unwrap().section, Section::Schedule);
        assert_eq!(a.task(done).unwrap().section, Section::Schedule);
        assert_eq!(a.task(already).unwrap().section, Section::Delegate);
        assert_eq!(a.task(already).unwrap().auto_from, None);

        let before = a.tasks.clone();
        a.tick_at(now + 60);
        assert!(a.tasks == before, "nothing changes on the next check");
        assert!(a.ui.status.is_empty(), "no announcements");
    }

    #[test]
    fn auto_urgent_can_be_turned_off() {
        let mut a = app();
        a.settings.auto_urgent = false;
        let now = from_local(2026, 3, 1, 12, 0);
        let id = a.add_task("x", Section::Schedule);
        a.task_mut(id).unwrap().due = Some(now + HOUR);
        a.tick_at(now);
        assert_eq!(a.task(id).unwrap().section, Section::Schedule);
    }

    #[test]
    fn all_day_tasks_are_late_only_after_the_day_ends() {
        let t = Task { all_day: true, due: Some(from_local(2026, 3, 1, 0, 0)), ..Default::default() };
        assert!(!t.is_overdue(from_local(2026, 3, 1, 23, 0)));
        assert!(t.is_overdue(from_local(2026, 3, 2, 0, 1)));
    }

    #[test]
    fn move_task_reorders_and_changes_section() {
        let mut a = app();
        let x = a.add_task("x", Section::Do);
        let y = a.add_task("y", Section::Do);
        let z = a.add_task("z", Section::Schedule);
        a.move_task(z, Section::Do, Some(x));
        let order: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
        assert_eq!(order, [z, x, y]);

        a.task_mut(x).unwrap().auto_from = Some(Section::Schedule);
        a.move_task(x, Section::Eliminate, None);
        assert_eq!(a.task(x).unwrap().section, Section::Eliminate);
        assert_eq!(a.task(x).unwrap().auto_from, None, "manual move clears auto flag");

        a.move_task(y, Section::Do, Some(y)); // dropping on itself is a no-op
        assert_eq!(a.visible_tasks(Section::Do).len(), 2);
    }

    #[test]
    fn delete_clears_links_and_selection() {
        let mut a = app();
        let x = a.add_task("x", Section::Do);
        let y = a.add_task("y", Section::Do);
        a.task_mut(y).unwrap().wait_for = Some(x);
        a.ui.selected = Some(x);
        a.delete_task(x);
        assert!(a.task(x).is_none());
        assert_eq!(a.task(y).unwrap().wait_for, None);
        assert_eq!(a.ui.selected, None);
    }

    // ── Simulated mouse tests (drive the real UI headlessly) ────

    struct Sim {
        ctx: egui::Context,
        time: f64,
        size: egui::Vec2,
    }

    impl Sim {
        fn new() -> Self {
            Self::with_size(1200.0, 800.0)
        }

        fn with_size(w: f32, h: f32) -> Self {
            Self { ctx: egui::Context::default(), time: 0.0, size: egui::vec2(w, h) }
        }

        fn frame(&mut self, app: &mut App, events: Vec<egui::Event>) {
            self.time += 1.0 / 60.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let _ = self.ctx.run(input, |ctx| app.frame(ctx));
        }

        fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }
        }

        /// Press at `from`, move to `to` in small steps, release.
        fn drag(&mut self, app: &mut App, from: egui::Pos2, to: egui::Pos2) {
            self.frame(app, vec![egui::Event::PointerMoved(from)]);
            self.frame(app, vec![Self::button(from, true)]);
            for i in 1..=12 {
                let p = from + (to - from) * (i as f32 / 12.0);
                self.frame(app, vec![egui::Event::PointerMoved(p)]);
            }
            self.frame(app, vec![Self::button(to, false)]);
            self.frame(app, vec![]);
        }

        fn key(&mut self, app: &mut App, key: egui::Key, modifiers: egui::Modifiers) {
            let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
            self.frame(app, vec![ev(true)]);
            self.frame(app, vec![ev(false)]);
            self.frame(app, vec![]);
        }

        fn drop_file(&mut self, app: &mut App, path: std::path::PathBuf) {
            self.time += 1.0 / 60.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                time: Some(self.time),
                dropped_files: vec![egui::DroppedFile { path: Some(path), ..Default::default() }],
                ..Default::default()
            };
            let _ = self.ctx.run(input, |ctx| app.frame(ctx));
        }

        fn type_text(&mut self, app: &mut App, text: &str) {
            self.frame(app, vec![egui::Event::Text(text.into())]);
        }

        fn click(&mut self, app: &mut App, at: egui::Pos2) {
            self.frame(app, vec![egui::Event::PointerMoved(at)]);
            self.frame(app, vec![Self::button(at, true)]);
            self.frame(app, vec![Self::button(at, false)]);
            self.frame(app, vec![]);
        }
    }

    /// A point on the first word of a row's title.
    fn on_title(row: egui::Rect) -> egui::Pos2 {
        egui::pos2(row.left() + 60.0, row.top() + 11.0)
    }

    fn sim_app() -> (App, Sim, TaskId, TaskId) {
        let mut a = app();
        let inbox = a.add_task("Inbox task", Section::Unsorted);
        let other = a.add_task("Another one", Section::Unsorted);
        let mut sim = Sim::new();
        sim.frame(&mut a, vec![]);
        sim.frame(&mut a, vec![]);
        (a, sim, inbox, other)
    }

    #[test]
    fn clicking_an_existing_tag_in_the_editor_adds_and_removes_it() {
        let (mut a, mut sim, id, other) = sim_app();
        a.task_mut(other).unwrap().tags.insert("home".into());
        a.ui.selected = Some(id);
        sim.frame(&mut a, vec![]);
        sim.frame(&mut a, vec![]);
        let chip = |a: &App| a.ui.tag_chips.iter().find(|(t, _)| t == "home").map(|(_, r)| r.center());
        let at = chip(&a).expect("tags already in use are offered in the editor");
        sim.click(&mut a, at);
        assert!(a.task(id).unwrap().tags.contains("home"), "click adds the tag");
        assert_eq!(a.ui.edit.tags, "home", "the text field shows it too");
        let at = chip(&a).unwrap();
        sim.click(&mut a, at);
        assert!(a.task(id).unwrap().tags.is_empty(), "a second click removes it");
    }

    /// Every named widget on screen this frame: (name, rect). Uses egui's accessibility tree.
    fn widgets(sim: &mut Sim, app: &mut App, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        sim.ctx.enable_accesskit();
        sim.time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, sim.size)),
            time: Some(sim.time),
            events,
            ..Default::default()
        };
        let out = sim.ctx.run(input, |ctx| app.frame(ctx));
        out.platform_output
            .accesskit_update
            .map(|u| {
                u.nodes
                    .iter()
                    .filter_map(|(_, n)| {
                        let b = n.bounds()?;
                        let r = egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32));
                        Some((n.name()?.to_string(), r))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn show_menu_has_three_choices_and_each_one_works_by_clicking() {
        let (mut a, mut sim, _, _) = sim_app();
        let open = a.add_task("Open thing", Section::Do);
        let done = a.add_task("Finished thing", Section::Do);
        a.set_completed_at(done, true, now() - 60);
        let shown = |a: &App| a.visible_tasks(Section::Do).iter().map(|t| t.id).collect::<Vec<_>>();
        for (choice, status, expect) in [
            ("All tasks", StatusFilter::All, vec![open, done]),
            ("Completed", StatusFilter::Completed, vec![done]),
            ("Uncompleted", StatusFilter::Active, vec![open]),
        ] {
            let w = widgets(&mut sim, &mut a, vec![]);
            let label = w.iter().find(|(n, _)| n == "Show").unwrap().1;
            sim.click(&mut a, egui::pos2(label.right() + 50.0, label.center().y));
            let w = widgets(&mut sim, &mut a, vec![]);
            // The menu holds only the three status choices: nothing else is highlighted alongside them.
            for other in ["Hide right away", "Show for 3 days", "Show for 1 week"] {
                assert!(!w.iter().any(|(n, _)| n == other), "{other:?} must not be in the Show menu");
            }
            let item = w.iter().find(|(n, r)| n == choice && r.top() > label.bottom()).expect(choice).1;
            sim.click(&mut a, item.center());
            assert_eq!(a.filter.status, status, "clicking {choice:?}");
            assert_eq!(shown(&a), expect, "tasks shown for {choice:?}");
        }
    }

    #[test]
    fn when_completed_tasks_hide_is_a_setting_chosen_by_clicking() {
        let (mut a, mut sim, _, _) = sim_app();
        let open = a.add_task("Open thing", Section::Do);
        let yesterday = a.add_task("Done yesterday", Section::Do);
        let last_month = a.add_task("Done last month", Section::Do);
        a.set_completed_at(yesterday, true, now() - 86_400);
        a.set_completed_at(last_month, true, now() - 30 * 86_400);
        let shown = |a: &App| a.visible_tasks(Section::Do).iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(shown(&a), [open], "default: completed tasks hide right away");

        a.ui.show_settings = true;
        // A new window measures itself on its first frames; let it settle before clicking.
        for _ in 0..3 {
            sim.frame(&mut a, vec![]);
        }
        let w = widgets(&mut sim, &mut a, vec![]);
        let three_days = w.iter().find(|(n, _)| n == "Show for 3 days").expect("option is in Settings").1;
        sim.click(&mut a, three_days.center());
        assert_eq!(a.settings.keep_completed, KeepCompleted::ThreeDays);
        assert_eq!(shown(&a), [open, yesterday], "recently completed stays visible; older ones hide");

        a.filter.status = StatusFilter::All;
        assert_eq!(shown(&a), [open, yesterday, last_month], "All tasks always shows every completed task");
        a.filter.status = StatusFilter::Completed;
        assert_eq!(shown(&a), [yesterday, last_month], "Completed shows every completed task");
    }

    #[test]
    fn sort_tags_and_reset_work_by_clicking() {
        let (mut a, mut sim, _, _) = sim_app();
        let b = a.add_task("Banana #food", Section::Do);
        let c = a.add_task("apple #food", Section::Do);
        let d = a.add_task("Car #auto", Section::Do);
        let shown = |a: &App| a.visible_tasks(Section::Do).iter().map(|t| t.id).collect::<Vec<_>>();
        let find = |w: &[(String, egui::Rect)], name: &str| w.iter().find(|(n, _)| n == name).map(|x| x.1);

        // Sort → Title
        let w = widgets(&mut sim, &mut a, vec![]);
        let sort = find(&w, "Sort").unwrap();
        sim.click(&mut a, egui::pos2(sort.right() + 50.0, sort.center().y));
        let w = widgets(&mut sim, &mut a, vec![]);
        let title = w.iter().find(|(n, r)| n == "Title" && r.top() > sort.bottom()).expect("Title in Sort menu").1;
        sim.click(&mut a, title.center());
        assert_eq!(a.filter.sort, SortMode::Title);
        assert_eq!(shown(&a), [c, b, d], "sorted by title");

        // Tags → #food
        let w = widgets(&mut sim, &mut a, vec![]);
        sim.click(&mut a, find(&w, "🏷 All tags").expect("tags button").center());
        let w = widgets(&mut sim, &mut a, vec![]);
        let tags_btn = find(&w, "🏷 All tags").expect("tags button");
        let food = w
            .iter()
            .filter(|(n, r)| n == "#food" && r.top() >= tags_btn.bottom())
            .min_by(|x, y| x.1.top().total_cmp(&y.1.top())) // the menu item sits right under the button; task tags are lower down
            .expect("#food in tags menu")
            .1;
        sim.click(&mut a, food.center());
        assert!(a.filter.tags.contains("food"));
        assert_eq!(shown(&a), [c, b], "only #food tasks");
        sim.key(&mut a, egui::Key::Escape, egui::Modifiers::NONE);

        // ✖ Reset brings everything back
        let w = widgets(&mut sim, &mut a, vec![]);
        sim.click(&mut a, find(&w, "✖ Reset").expect("Reset shows while filtered").center());
        assert_eq!(a.filter, Filter::default());
        assert_eq!(shown(&a), [b, c, d]);
    }

    #[test]
    fn drag_by_title_moves_task_between_boxes() {
        let (mut a, mut sim, id, _) = sim_app();
        let target = a.ui.section_rects[&Section::Schedule].center();
        // Grab right on the title words (not the empty space, not the handle).
        let from = on_title(a.ui.row_rects[&id]);
        sim.drag(&mut a, from, target);
        assert_eq!(a.task(id).unwrap().section, Section::Schedule);
    }

    #[test]
    fn drag_by_handle_still_works() {
        let (mut a, mut sim, id, _) = sim_app();
        let row = a.ui.row_rects[&id];
        let handle = egui::pos2(row.left() + 8.0, row.top() + 10.0);
        let target = a.ui.section_rects[&Section::Eliminate].center();
        sim.drag(&mut a, handle, target);
        assert_eq!(a.task(id).unwrap().section, Section::Eliminate);
    }

    #[test]
    fn drag_works_in_list_view_too() {
        let (mut a, mut sim, id, _) = sim_app();
        a.view = View::List;
        sim.frame(&mut a, vec![]);
        sim.frame(&mut a, vec![]);
        let row = a.ui.row_rects[&id];
        let target = a.ui.section_rects[&Section::Do].center();
        sim.drag(&mut a, row.center(), target);
        assert_eq!(a.task(id).unwrap().section, Section::Do);
    }

    #[test]
    fn drop_on_a_task_inserts_before_it() {
        let (mut a, mut sim, first, second) = sim_app();
        // Grab the second task by its title text and drop it on the first.
        let from = on_title(a.ui.row_rects[&second]);
        let to = a.ui.row_rects[&first].center();
        sim.drag(&mut a, from, to);
        let order: Vec<_> = a.visible_tasks(Section::Unsorted).iter().map(|t| t.id).collect();
        assert_eq!(order, [second, first]);
    }

    #[test]
    fn clicking_title_selects_without_moving() {
        let (mut a, mut sim, id, _) = sim_app();
        let row = a.ui.row_rects[&id];
        sim.click(&mut a, egui::pos2(row.left() + 60.0, row.top() + 10.0));
        assert_eq!(a.ui.selected, Some(id));
        assert_eq!(a.task(id).unwrap().section, Section::Unsorted);
    }

    #[test]
    fn a_plain_click_never_starts_a_drag_or_moves_the_task() {
        for t in ThemeName::ALL {
            for view in [View::Matrix, View::List] {
                let mut a = app();
                a.theme = t;
                a.view = view;
                let first = a.add_task("First", Section::Do);
                let second = a.add_task("Second", Section::Do);
                let third = a.add_task("Third", Section::Do);
                let mut sim = Sim::new();
                sim.frame(&mut a, vec![]);
                sim.frame(&mut a, vec![]);
                let before: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
                let at = on_title(a.ui.row_rects[&first]);

                // Press: no drag should begin until the mouse actually moves.
                sim.frame(&mut a, vec![egui::Event::PointerMoved(at)]);
                sim.frame(&mut a, vec![Sim::button(at, true)]);
                sim.frame(&mut a, vec![]);
                assert!(
                    egui::DragAndDrop::payload::<TaskId>(&sim.ctx).is_none(),
                    "{t:?} {view:?}: pressing a task started a drag"
                );
                sim.frame(&mut a, vec![Sim::button(at, false)]);
                sim.frame(&mut a, vec![]);

                let after: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
                assert_eq!(before, after, "{t:?} {view:?}: clicking moved the task");
                assert_eq!(a.ui.selected, Some(first), "{t:?} {view:?}: click selects");

                // Clicking the empty part of another row opens that one in the details panel.
                let row = a.ui.row_rects[&third];
                sim.click(&mut a, egui::pos2(row.right() - 40.0, row.center().y));
                assert_eq!(a.ui.selected, Some(third), "{t:?} {view:?}: click on empty row space selects");
                let after: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
                assert_eq!(before, after, "{t:?} {view:?}: still nothing moved");
                let _ = second;
            }
        }
    }

    #[test]
    fn checkbox_still_completes_task() {
        let (mut a, mut sim, id, _) = sim_app();
        let row = a.ui.row_rects[&id];
        // Checkbox sits right after the drag handle.
        sim.click(&mut a, egui::pos2(row.left() + 28.0, row.top() + 11.0));
        assert!(a.task(id).unwrap().completed, "checkbox click should complete, not drag");
    }

    #[test]
    fn trash_button_deletes_unsorted_task() {
        let (mut a, mut sim, id, other) = sim_app();
        let row = a.ui.row_rects[&id];
        sim.click(&mut a, egui::pos2(row.right() - 10.0, row.top() + 11.0));
        assert!(a.task(id).is_none());
        assert!(a.task(other).is_some());
    }

    #[test]
    fn every_icon_exists_in_the_bundled_fonts() {
        // Everything the user can see is written in these files (comments skipped).
        let sources = [include_str!("ui.rs"), include_str!("app.rs"), include_str!("datepicker.rs"), include_str!("model.rs")];
        let visible: String = sources
            .iter()
            .flat_map(|src| src.lines())
            .filter(|l| !l.trim_start().starts_with("//"))
            .map(|l| l.split(" // ").next().unwrap_or(""))
            .collect();
        for t in ThemeName::ALL {
            let (mut a, mut sim, _, _) = sim_app();
            a.theme = t;
            sim.frame(&mut a, vec![]);
            sim.frame(&mut a, vec![]);
            let font = egui::FontId::proportional(14.0);
            let missing: Vec<char> = visible
                .chars()
                .filter(|c| !c.is_ascii())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .filter(|c| !sim.ctx.fonts(|f| f.has_glyph(&font, *c)))
                .collect();
            assert!(missing.is_empty(), "{t:?}: glyphs missing (would render as boxes): {missing:?}");
        }
    }

    #[test]
    fn plus_button_adds_tasks_straight_into_a_quadrant() {
        let (mut a, mut sim, _, _) = sim_app();
        let plus = a.ui.add_buttons[&Section::Schedule].center();
        sim.click(&mut a, plus);
        assert_eq!(a.ui.adding_to, Some(Section::Schedule));

        sim.type_text(&mut a, "Plan the trip #travel");
        sim.key(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
        let t = a.tasks.iter().find(|t| t.title == "Plan the trip").expect("task added");
        assert_eq!(t.section, Section::Schedule);
        assert!(t.tags.contains("travel"));

        // Box stays open and focused for the next one.
        assert_eq!(a.ui.adding_to, Some(Section::Schedule));
        sim.type_text(&mut a, "Book hotel");
        sim.key(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(a.tasks.iter().find(|t| t.title == "Book hotel").unwrap().section, Section::Schedule);

        sim.key(&mut a, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(a.ui.adding_to, None);
    }

    #[test]
    fn pasting_lines_into_a_quadrant_adds_them_all_there() {
        let (mut a, mut sim, _, _) = sim_app();
        let plus = a.ui.add_buttons[&Section::Delegate].center();
        sim.click(&mut a, plus);
        sim.frame(&mut a, vec![egui::Event::Paste("Email Ann\nCall Bob\n".into())]);
        let in_delegate: Vec<_> = a.visible_tasks(Section::Delegate).iter().map(|t| t.title.clone()).collect();
        assert_eq!(in_delegate, ["Email Ann", "Call Bob"]);
    }

    #[test]
    fn cmd_n_in_maximized_quadrant_adds_there() {
        let (mut a, mut sim, _, _) = sim_app();
        a.ui.maximized = Some(Section::Do);
        sim.frame(&mut a, vec![]);
        sim.key(&mut a, egui::Key::N, egui::Modifiers::COMMAND);
        assert_eq!(a.ui.adding_to, Some(Section::Do));
        sim.type_text(&mut a, "Urgent thing");
        sim.key(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(a.tasks.iter().find(|t| t.title == "Urgent thing").unwrap().section, Section::Do);
    }

    #[test]
    fn choosing_a_theme_applies_it_and_saves_it() {
        let (mut a, mut sim, _, _) = sim_app();
        a.set_theme(ThemeName::Paper);
        sim.frame(&mut a, vec![]);
        assert_eq!(sim.ctx.theme(), egui::Theme::Light);
        assert_eq!(sim.ctx.style().visuals.panel_fill, crate::theme::palette(ThemeName::Paper).bg);
        assert_eq!(a.to_data().theme, ThemeName::Paper, "goes into the saved data");
        let reopened = App::from_data(a.to_data());
        assert_eq!(reopened.theme, ThemeName::Paper, "and comes back on next launch");
    }

    #[test]
    fn drag_and_add_work_in_every_theme() {
        for t in ThemeName::ALL {
            let (mut a, mut sim, id, _) = sim_app();
            a.theme = t;
            sim.frame(&mut a, vec![]);
            sim.frame(&mut a, vec![]);
            let row = a.ui.row_rects[&id];
            let target = a.ui.section_rects[&Section::Do].center();
            sim.drag(&mut a, row.center(), target);
            assert_eq!(a.task(id).unwrap().section, Section::Do, "{t:?}: drag");
            let plus = a.ui.add_buttons[&Section::Schedule].center();
            sim.click(&mut a, plus);
            sim.type_text(&mut a, "themed task");
            sim.key(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
            assert!(a.tasks.iter().any(|x| x.title == "themed task" && x.section == Section::Schedule), "{t:?}: add");
        }
    }

    #[test]
    fn inbox_can_be_hidden_and_quadrants_widen() {
        let (mut a, mut sim, _, _) = sim_app();
        let narrow = a.ui.section_rects[&Section::Do].width();
        sim.key(&mut a, egui::Key::I, egui::Modifiers::COMMAND);
        assert!(!a.settings.show_inbox);
        sim.frame(&mut a, vec![]);
        assert!(!a.ui.section_rects.contains_key(&Section::Unsorted), "inbox not drawn");
        assert!(a.ui.section_rects[&Section::Do].width() > narrow + 50.0, "quadrants use the space");
        assert!(a.visible_order().iter().all(|id| a.task(*id).unwrap().section != Section::Unsorted));
        assert!(!a.to_data().settings.show_inbox, "choice is saved");

        sim.key(&mut a, egui::Key::I, egui::Modifiers::COMMAND);
        sim.frame(&mut a, vec![]);
        assert!(a.ui.section_rects.contains_key(&Section::Unsorted), "and comes back");
    }

    #[test]
    fn hiding_inbox_also_hides_it_in_list_view_and_closes_its_views() {
        let (mut a, mut sim, _, _) = sim_app();
        a.view = View::List;
        a.ui.maximized = Some(Section::Unsorted);
        a.set_inbox_visible(false);
        assert_eq!(a.ui.maximized, None);
        sim.frame(&mut a, vec![]);
        sim.frame(&mut a, vec![]);
        assert!(!a.ui.section_rects.contains_key(&Section::Unsorted));
        assert!(a.ui.section_rects.contains_key(&Section::Eliminate));
    }

    #[test]
    fn adding_while_inbox_hidden_says_where_it_went() {
        let (mut a, mut sim, _, _) = sim_app();
        a.set_inbox_visible(false);
        sim.key(&mut a, egui::Key::N, egui::Modifiers::COMMAND);
        sim.type_text(&mut a, "Hidden add");
        sim.key(&mut a, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(a.tasks.iter().any(|t| t.title == "Hidden add" && t.section == Section::Unsorted));
        assert!(a.ui.status.contains("hidden"), "status: {}", a.ui.status);
    }

    #[test]
    fn toolbar_inbox_button_is_drawn_and_clickable() {
        let (mut a, mut sim, _, _) = sim_app();
        let r = a.ui.inbox_toggle.expect("toolbar inbox button drawn");
        assert!(r.left() < 30.0, "first thing on the left of the toolbar: {r:?}");
        sim.click(&mut a, r.center());
        assert!(!a.settings.show_inbox, "toolbar button hides");
        let r = a.ui.inbox_toggle.expect("still drawn while hidden");
        sim.click(&mut a, r.center());
        assert!(a.settings.show_inbox, "and shows it again");
    }

    #[test]
    fn nothing_spills_off_a_small_window() {
        // 820 x 520 is the smallest the window can be (see main.rs).
        for t in ThemeName::ALL {
            for view in [View::Matrix, View::List] {
                let mut a = app();
                a.theme = t;
                a.view = view;
                a.add_task("A task", Section::Eliminate);
                let mut sim = Sim::with_size(820.0, 520.0);
                for _ in 0..3 {
                    sim.frame(&mut a, vec![]);
                }
                let screen = 820.0;
                let add = a.ui.quick_add_button.expect("➕ Add drawn");
                assert!(add.right() <= screen, "{t:?} {view:?}: ➕ Add ends at {} > {screen}", add.right());
                let gear = a.ui.settings_button.expect("⚙ drawn");
                assert!(gear.right() <= screen, "{t:?} {view:?}: ⚙ ends at {}", gear.right());
                assert!(gear.left() > add.left() - 1.0 || gear.top() > add.bottom(), "⚙ on the row below");
                let total = a.ui.quadrant_total.expect("total drawn");
                assert!(total.right() <= gear.left() + 0.5, "{t:?} {view:?}: total overlaps ⚙");
                assert!(total.left() >= a.ui.toolbar_left_end - 0.5, "{t:?} {view:?}: total runs into the filters ({total:?})");
                for (s, parts) in &a.ui.header_parts {
                    let card = a.ui.section_rects[s];
                    for (i, p) in parts.iter().enumerate() {
                        assert!(p.right() <= card.right() + 0.5, "{t:?} {view:?}: {s:?} header item past the card edge");
                        for q in &parts[i + 1..] {
                            let overlap = p.intersect(*q);
                            assert!(
                                !overlap.is_positive() || overlap.width() < 1.0,
                                "{t:?} {view:?}: {s:?} header items overlap: {p:?} and {q:?}"
                            );
                        }
                    }
                }
                for (s, b) in &a.ui.add_buttons {
                    let card = a.ui.section_rects[s];
                    assert!(
                        b.right() <= card.right() && b.left() >= card.left(),
                        "{t:?} {view:?}: {s:?} header ➕ {b:?} outside its card {card:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn filter_hiding_tasks_shows_a_banner_with_show_all() {
        let (mut a, mut sim, _, _) = sim_app();
        assert!(a.ui.show_all_button.is_none(), "no banner without a filter");
        let tagged = a.add_task("Work thing #work", Section::Do);
        a.filter.tags.insert("work".into());
        sim.frame(&mut a, vec![]);
        assert_eq!(a.hidden_by_filter(), 2, "the two untagged inbox tasks are hidden");
        let b = a.ui.show_all_button.expect("banner with Show all is visible");
        sim.click(&mut a, b.center());
        assert!(a.filter.tags.is_empty());
        assert_eq!(a.hidden_by_filter(), 0);
        assert!(a.task(tagged).is_some());
        sim.frame(&mut a, vec![]);
        assert!(a.ui.show_all_button.is_none(), "banner gone");
    }

    #[test]
    fn completed_only_view_also_warns() {
        let (mut a, mut sim, _, _) = sim_app();
        a.filter.status = StatusFilter::Completed;
        sim.frame(&mut a, vec![]);
        assert!(a.ui.show_all_button.is_some());
    }

    #[test]
    fn csv_export_then_import_into_an_empty_app_restores_everything() {
        let mut a = app();
        let x = a.add_task("Pay rent #home", Section::Do);
        a.task_mut(x).unwrap().due = Some(crate::timeutil::from_local(2026, 11, 1, 18, 30));
        a.add_task("Read, \"carefully\"", Section::Schedule);
        let done = a.add_task("Finished thing", Section::Eliminate);
        a.set_completed_at(done, true, 1_700_000_000);
        a.filter.tags.insert("home".into()); // filters must not limit the export

        let dir = std::env::temp_dir().join(format!("emtasks-csv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("backup.csv");
        assert_eq!(a.export_csv_to(&path).unwrap(), 3);

        let mut fresh = app();
        fresh.import_csv_file(&path);
        let r = fresh.ui.import_report.as_ref().unwrap();
        assert_eq!((r.added, r.skipped), (3, 0), "{:?}", r.warnings);
        let titles: Vec<_> = fresh.tasks.iter().map(|t| (t.title.as_str(), t.section, t.completed)).collect();
        assert_eq!(
            titles,
            [("Pay rent", Section::Do, false), ("Read, \"carefully\"", Section::Schedule, false), ("Finished thing", Section::Eliminate, true)]
        );
        assert_eq!(fresh.tasks[0].due, a.task(x).unwrap().due);
    }

    #[test]
    fn importing_twice_does_not_duplicate_and_never_removes() {
        let mut a = app();
        a.add_task("Keep me", Section::Delegate);
        let csv = "title,quadrant\nNew one,Do\nKeep me,Delegate\n";
        let r1 = a.import_csv_text(csv, "x.csv");
        assert_eq!((r1.added, r1.skipped), (1, 1));
        let r2 = a.import_csv_text(csv, "x.csv");
        assert_eq!((r2.added, r2.skipped), (0, 2));
        assert_eq!(a.tasks.len(), 2);
    }

    #[test]
    fn oversized_csv_is_refused_without_reading_it() {
        let mut a = app();
        let dir = std::env::temp_dir().join(format!("emtasks-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("huge.csv");
        let f = std::fs::File::create(&path).unwrap();
        f.set_len(MAX_IMPORT_BYTES + 1).unwrap(); // sparse: instant, no real disk use
        a.import_csv_file(&path);
        let r = a.ui.import_report.as_ref().unwrap();
        assert_eq!(r.added, 0);
        assert!(r.warnings[0].contains("up to 20 MB"), "{:?}", r.warnings);
        assert!(a.tasks.is_empty());
    }

    #[test]
    fn import_can_be_undone() {
        let mut a = app();
        a.add_task("Existing", Section::Do);
        a.history.observe(&a.tasks, a.next_id, false);
        a.import_csv_text("A\nB\nC\n", "list.csv");
        a.history.observe(&a.tasks, a.next_id, false);
        assert_eq!(a.tasks.len(), 4);
        a.undo();
        assert_eq!(a.tasks.len(), 1);
    }

    #[test]
    fn dropping_a_csv_on_the_window_imports_it() {
        let (mut a, mut sim, _, _) = sim_app();
        let dir = std::env::temp_dir().join(format!("emtasks-drop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("drop.csv");
        std::fs::write(&path, "title,quadrant\nDropped task,Schedule\n").unwrap();
        sim.drop_file(&mut a, path);
        assert!(a.tasks.iter().any(|t| t.title == "Dropped task" && t.section == Section::Schedule));
        assert!(a.ui.import_report.is_some(), "a report is shown");
    }

    #[test]
    fn total_shows_done_of_all_in_the_four_quadrants() {
        let (mut a, mut sim, _, _) = sim_app(); // two Inbox tasks: not counted
        a.add_task("one", Section::Do);
        a.add_task("two", Section::Schedule);
        a.add_task("three", Section::Eliminate);
        let done = a.add_task("finished", Section::Delegate);
        a.set_completed(done, true);
        sim.frame(&mut a, vec![]);
        assert!(a.ui.quadrant_total.is_some());
        assert_eq!(a.quadrant_progress(), (1, 4), "1 done of 4; Inbox not counted");
    }

    #[test]
    fn deleting_from_unsorted_selects_the_next_task() {
        let mut a = app();
        let x = a.add_task("x", Section::Unsorted);
        let y = a.add_task("y", Section::Unsorted);
        a.ui.selected = Some(x);
        a.select_next_after_delete(x);
        assert!(a.task(x).is_none());
        assert_eq!(a.ui.selected, Some(y));
        a.select_next_after_delete(y);
        assert_eq!(a.ui.selected, None);
    }

    #[test]
    fn filter_and_sort() {
        let mut a = app();
        let b = a.add_task("Banana #food", Section::Do);
        let c = a.add_task("apple #food", Section::Do);
        let d = a.add_task("Car #auto", Section::Do);
        a.task_mut(d).unwrap().due = Some(100);
        a.task_mut(b).unwrap().due = Some(200);
        a.set_completed_at(c, true, 0);

        a.filter.sort = SortMode::Due;
        a.filter.status = StatusFilter::All;
        let ids: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
        assert_eq!(ids, [d, b, c], "by due, undated last, completed at bottom");

        a.filter.sort = SortMode::Title;
        a.filter.status = StatusFilter::Active;
        let ids: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
        assert_eq!(ids, [b, d]);

        a.filter.tags.insert("food".into());
        a.filter.status = StatusFilter::Completed;
        let ids: Vec<_> = a.visible_tasks(Section::Do).iter().map(|t| t.id).collect();
        assert_eq!(ids, [c]);
    }

    #[test]
    fn completed_tasks_stay_visible_for_the_chosen_time() {
        let mut a = app();
        let open = a.add_task("still open", Section::Do);
        let today = a.add_task("done just now", Section::Do);
        let four_days = a.add_task("done 4 days ago", Section::Do);
        let old = a.add_task("done 8 days ago", Section::Do);
        a.set_completed_at(today, true, now() - 60);
        a.set_completed_at(four_days, true, now() - 4 * 86_400);
        a.set_completed_at(old, true, now() - 8 * 86_400);
        a.filter.status = StatusFilter::Active;
        let shown = |a: &App| a.visible_tasks(Section::Do).iter().map(|t| t.id).collect::<Vec<_>>();

        a.settings.keep_completed = KeepCompleted::Hide;
        assert_eq!(shown(&a), [open]);
        a.settings.keep_completed = KeepCompleted::ThreeDays;
        assert_eq!(shown(&a), [open, today]);
        a.settings.keep_completed = KeepCompleted::Week;
        assert_eq!(shown(&a), [open, today, four_days], "completed ones sink to the bottom");
        assert_eq!(a.hidden_by_filter(), 0, "the setting is not a filter, so no warning bar");

        a.filter.status = StatusFilter::Completed;
        assert_eq!(shown(&a).len(), 3, "the Completed view always shows every completed task");
    }

    #[test]
    fn top_counts_all_time_and_bottom_counts_what_is_on_screen() {
        let mut a = app();
        a.add_task("unsorted", Section::Unsorted);
        a.add_task("open", Section::Do);
        let recent = a.add_task("done today", Section::Do);
        let old = a.add_task("done long ago", Section::Schedule);
        a.set_completed_at(recent, true, now() - 3600);
        a.set_completed_at(old, true, now() - 30 * 86_400);
        a.filter.status = StatusFilter::Active;
        a.settings.keep_completed = KeepCompleted::Week;
        assert_eq!(a.quadrant_progress(), (2, 3), "all time: every quadrant task, Inbox not counted");
        assert_eq!(a.on_screen_counts(), (2, 1), "on screen: Inbox + open, and the recently done one");
        a.set_inbox_visible(false);
        assert_eq!(a.on_screen_counts(), (1, 1), "a hidden Inbox isn't on screen");
        a.ui.maximized = Some(Section::Schedule);
        assert_eq!(a.on_screen_counts(), (0, 0));
    }

    #[test]
    fn undo_restores_deleted_task_without_reusing_ids() {
        let mut a = app();
        let x = a.add_task("x", Section::Do);
        a.history.observe(&a.tasks, a.next_id, false);
        a.delete_task(x);
        a.history.observe(&a.tasks, a.next_id, false);
        a.undo();
        assert!(a.task(x).is_some());
        a.redo();
        assert!(a.task(x).is_none());
        a.undo();
        let y = a.add_task("y", Section::Do);
        assert_ne!(x, y);
    }

    #[test]
    fn due_parsing() {
        let mut t = Task::default();
        apply_due(&mut t, "2026-05-04", "").unwrap();
        assert_eq!(t.due, Some(from_local(2026, 5, 4, 9, 0)));
        apply_due(&mut t, "2026-05-04", "17:30").unwrap();
        assert_eq!(t.due, Some(from_local(2026, 5, 4, 17, 30)));
        t.all_day = true;
        apply_due(&mut t, "2026-05-04", "17:30").unwrap();
        assert_eq!(t.due, Some(from_local(2026, 5, 4, 0, 0)));
        assert!(apply_due(&mut t, "05/04/2026", "").is_err());
        t.all_day = false;
        assert!(apply_due(&mut t, "2026-05-04", "5pm").is_err());
        apply_due(&mut t, "", "").unwrap();
        assert_eq!(t.due, None);
    }

    #[test]
    fn documents_backup_runs_once_a_day_and_never_from_recovery() {
        let dir = std::env::temp_dir().join(format!("emtasks-test-documents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App {
            storage: Some(Storage::new(dir.join("data"))),
            documents_backup_dir: Some(dir.join("Documents")),
            ..Default::default()
        };
        a.add_task("keep me", Section::Do);
        let file = dir.join("Documents").join(format!("emTasks-tasks-{}.csv", timeutil::fmt_date(now())));

        a.documents_backup(false);
        assert!(std::fs::read_to_string(&file).unwrap().contains("keep me"));
        a.add_task("added later", Section::Do);
        a.documents_backup(false);
        assert!(!std::fs::read_to_string(&file).unwrap().contains("added later"), "only once a day");
        a.documents_backup(true);
        assert!(std::fs::read_to_string(&file).unwrap().contains("added later"), "quitting refreshes today's copy");

        // A recovery screen holds an empty list; it must never overwrite the backup.
        let mut r = App { storage: a.storage.take(), documents_backup_dir: a.documents_backup_dir.clone(), ..Default::default() };
        r.ui.recovery = Some(Recovery::AlreadyRunning);
        r.documents_backup(true);
        assert!(std::fs::read_to_string(&file).unwrap().contains("added later"));
    }

    #[test]
    fn a_second_copy_shows_already_open_and_never_saves() {
        let dir = std::env::temp_dir().join(format!("emtasks-test-second-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = App::default();
        a.ui.recovery = Some(Recovery::AlreadyRunning);
        a.storage = Some(Storage::new(dir.clone()));
        let mut sim = Sim::new();
        sim.frame(&mut a, vec![]);
        sim.frame(&mut a, vec![]);
        a.add_task("must not reach disk", Section::Do);
        a.save_now();
        assert!(!a.can_save());
        assert!(!dir.join("emtasks-data.ron").exists());
    }
}


#[cfg(test)]
mod real_data_check {
    use super::*;

    /// Read-only check against a real data file: EMTASKS_REAL_DATA=path cargo test -- --ignored
    #[test]
    #[ignore]
    fn csv_round_trip_of_real_data() {
        let Ok(path) = std::env::var("EMTASKS_REAL_DATA") else { return };
        let text = std::fs::read_to_string(&path).unwrap();
        let LoadResult::Loaded(d) = crate::storage::parse(&text) else { panic!("can't parse") };
        let original = App::from_data(d);
        let csv = crate::csvio::export(&original.tasks);
        let mut restored = App::default();
        let r = restored.import_csv_text(&csv, "real.csv");
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert_eq!(restored.tasks.len(), original.tasks.len());
        for (o, b) in original.tasks.iter().zip(&restored.tasks) {
            assert_eq!(
                (&o.title, o.section, o.completed, o.due, o.all_day, o.repeat, &o.tags, &o.notes),
                (&b.title, b.section, b.completed, b.due, b.all_day, b.repeat, &b.tags, &b.notes),
            );
        }
        println!("REAL DATA: {} tasks exported and restored identically", original.tasks.len());
    }

}
