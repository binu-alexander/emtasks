//! The data file: atomic saves, rolling backups, versioning and recovery.
//!
//! Layout (macOS): ~/Library/Application Support/emTasks/
//!   emtasks-data.ron                     current data
//!   backups/emtasks-data-YYYYMMDD-HHMMSS.ron   one per launch, newest 20 kept
//!   emtasks-data.damaged-….ron            set aside if it ever failed to load

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::theme::ThemeName;
use crate::timeutil;

/// Bump when the format changes in a way that needs migration.
pub const DATA_VERSION: u32 = 1;
const MAX_BACKUPS: usize = 20;
const DATA_FILE: &str = "emtasks-data.ron";
const BACKUP_PREFIX: &str = "emtasks-data-";
const LOCK_FILE: &str = ".emtasks.lock";
const DAILY_CSV_PREFIX: &str = "emTasks-tasks-";
const MAX_DAILY_CSVS: usize = 30;
pub const CRASH_LOG: &str = "crash.log";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub version: u32,
    pub tasks: Vec<Task>,
    pub next_id: TaskId,
    pub filter: Filter,
    pub presets: Vec<Preset>,
    pub view: View,
    /// The chosen theme; files from before themes existed get Classic.
    pub theme: ThemeName,
    pub settings: Settings,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            // Files without a version field predate versioning.
            version: 0,
            tasks: Vec::new(),
            next_id: 1,
            filter: Filter::default(),
            presets: Vec::new(),
            view: View::Matrix,
            theme: ThemeName::Classic,
            settings: Settings::default(),
        }
    }
}

pub enum LoadResult {
    Missing,
    Loaded(Data),
    Corrupt(String),
    TooNew(u32),
}

pub struct Storage {
    pub dir: PathBuf,
}

impl Storage {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn default_dir() -> PathBuf {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Application Support/emTasks")
    }

    pub fn data_path(&self) -> PathBuf {
        self.dir.join(DATA_FILE)
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    /// Claim the data folder for this process. Returns None if another copy of
    /// emTasks already holds it; two copies saving the same file would lose tasks.
    /// The lock lasts as long as the returned file is open (the OS drops it on exit or crash).
    pub fn lock(&self) -> io::Result<Option<fs::File>> {
        fs::create_dir_all(&self.dir)?;
        lock_in(&self.dir, LOCK_FILE)
    }

    pub fn load(&self) -> LoadResult {
        let path = self.data_path();
        match fs::read_to_string(&path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => LoadResult::Missing,
            Err(e) => LoadResult::Corrupt(format!("Couldn't read {}: {e}", path.display())),
        }
    }

    /// Write to a temp file, flush to disk, then rename over the old file,
    /// so a crash mid-save can never leave a half-written data file.
    pub fn save(&self, data: &Data) -> io::Result<()> {
        let text = ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default())
            .map_err(io::Error::other)?;
        self.write_atomic(&text)
    }

    fn write_atomic(&self, text: &str) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.data_path();
        let tmp = path.with_extension("ron.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &path)?;
        // Flush the rename itself, so a power cut right after can't bring back the old file.
        fs::File::open(&self.dir)?.sync_all()
    }

    /// Copy the current data file into backups/ and prune old ones.
    pub fn backup(&self) -> io::Result<Option<PathBuf>> {
        let src = self.data_path();
        if !src.exists() {
            return Ok(None);
        }
        let dir = self.backups_dir();
        fs::create_dir_all(&dir)?;
        let dest = unique_path(&dir, &format!("{BACKUP_PREFIX}{}", timeutil::fmt_stamp(timeutil::now())), "ron");
        fs::copy(&src, &dest)?;
        for old in self.list_backups().into_iter().skip(MAX_BACKUPS) {
            let _ = fs::remove_file(old);
        }
        Ok(Some(dest))
    }

    /// Newest first.
    pub fn list_backups(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(self.backups_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(BACKUP_PREFIX) && n.ends_with(".ron"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v.reverse();
        v
    }

    /// Rename a data file that failed to load, so it's kept but out of the way.
    pub fn set_aside_damaged(&self) -> io::Result<Option<PathBuf>> {
        let path = self.data_path();
        if !path.exists() {
            return Ok(None);
        }
        let dest = unique_path(
            &self.dir,
            &format!("emtasks-data.damaged-{}", timeutil::fmt_stamp(timeutil::now())),
            "ron",
        );
        fs::rename(&path, &dest)?;
        Ok(Some(dest))
    }

    pub fn restore(&self, backup: &Path) -> io::Result<()> {
        let text = fs::read_to_string(backup)?;
        if !matches!(parse(&text), LoadResult::Loaded(_)) {
            return Err(io::Error::other("that backup can't be read either"));
        }
        self.set_aside_damaged()?;
        self.write_atomic(&text)
    }
}

/// Take the lock file `name` in `dir` without waiting. None if another process holds it.
pub fn lock_in(dir: &Path, name: &str) -> io::Result<Option<fs::File>> {
    use std::os::fd::AsRawFd;
    let f = fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(name))?;
    // SAFETY: flock only operates on the descriptor we own.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(f));
    }
    let e = io::Error::last_os_error();
    if e.kind() == io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(e)
    }
}

/// Write today's CSV backup into `dir` (replacing an earlier one from the same day)
/// and keep only the newest 30 days. Other files in the folder are never touched.
pub fn write_daily_csv(dir: &Path, date: &str, csv: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{DAILY_CSV_PREFIX}{date}.csv"));
    let tmp = dir.join(format!(".{DAILY_CSV_PREFIX}{date}.csv.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(csv.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    let is_daily = |n: &str| {
        n.strip_prefix(DAILY_CSV_PREFIX)
            .and_then(|r| r.strip_suffix(".csv"))
            .is_some_and(|d| d.len() == 10 && d.chars().all(|c| c.is_ascii_digit() || c == '-'))
    };
    let mut daily: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(is_daily))
        .collect();
    daily.sort();
    daily.reverse();
    for old in daily.into_iter().skip(MAX_DAILY_CSVS) {
        let _ = fs::remove_file(old);
    }
    Ok(path)
}

pub fn parse(text: &str) -> LoadResult {
    match ron::from_str::<Data>(text) {
        Ok(d) if d.version > DATA_VERSION => LoadResult::TooNew(d.version),
        Ok(mut d) => {
            migrate(&mut d);
            LoadResult::Loaded(d)
        }
        Err(e) => LoadResult::Corrupt(format!("The data file is damaged ({e}).")),
    }
}

fn migrate(d: &mut Data) {
    // v0 → v1: no structural changes, just start tracking the version.
    // Never hand out an id that's already in use.
    let max_id = d.tasks.iter().map(|t| t.id).max().unwrap_or(0);
    d.next_id = d.next_id.max(max_id + 1);
    d.version = DATA_VERSION;
}

fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage(name: &str) -> Storage {
        let dir = std::env::temp_dir().join(format!("emtasks-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Storage::new(dir)
    }

    #[test]
    fn only_one_copy_can_hold_the_data_folder() {
        let s = temp_storage("lock");
        let first = s.lock().unwrap();
        assert!(first.is_some());
        assert!(s.lock().unwrap().is_none(), "a second copy must be refused");
        drop(first);
        assert!(s.lock().unwrap().is_some(), "the lock is released when the first copy exits");
    }

    #[test]
    fn daily_csv_replaces_today_keeps_30_days_and_leaves_other_files_alone() {
        let dir = temp_storage("daily-csv").dir;
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("my notes.csv"), "mine").unwrap();
        for day in 1..=31 {
            write_daily_csv(&dir, &format!("2026-01-{day:02}"), "old").unwrap();
        }
        let p = write_daily_csv(&dir, "2026-01-31", "newer").unwrap();
        assert_eq!(fs::read_to_string(p).unwrap(), "newer");
        let names: Vec<String> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into()).collect();
        assert_eq!(names.iter().filter(|n| n.starts_with("emTasks-tasks-")).count(), 30);
        assert!(!dir.join("emTasks-tasks-2026-01-01.csv").exists(), "the oldest day is pruned");
        assert!(dir.join("my notes.csv").exists(), "other files are never touched");
        assert!(!names.iter().any(|n| n.ends_with(".tmp")));
    }

    fn sample() -> Data {
        let mut d = Data { version: DATA_VERSION, next_id: 3, ..Default::default() };
        d.tasks.push(Task { id: 1, title: "One".into(), ..Default::default() });
        d.tasks.push(Task { id: 2, title: "Two \"quoted\"".into(), section: Section::Do, ..Default::default() });
        d
    }

    #[test]
    fn save_and_load_roundtrip() {
        let s = temp_storage("roundtrip");
        assert!(matches!(s.load(), LoadResult::Missing));
        s.save(&sample()).unwrap();
        match s.load() {
            LoadResult::Loaded(d) => assert!(d == sample()),
            _ => panic!("expected Loaded"),
        }
        assert!(!s.data_path().with_extension("ron.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_reported_and_left_untouched() {
        let s = temp_storage("corrupt");
        fs::create_dir_all(&s.dir).unwrap();
        fs::write(s.data_path(), "this is not ron {{{").unwrap();
        assert!(matches!(s.load(), LoadResult::Corrupt(_)));
        assert_eq!(fs::read_to_string(s.data_path()).unwrap(), "this is not ron {{{");
    }

    #[test]
    fn newer_version_is_refused() {
        assert!(matches!(parse("(version: 99)"), LoadResult::TooNew(99)));
    }

    #[test]
    fn legacy_file_without_version_migrates() {
        match parse("(tasks: [(id: 7, title: \"Old\")], next_id: 2)") {
            LoadResult::Loaded(d) => {
                assert_eq!(d.version, DATA_VERSION);
                assert_eq!(d.next_id, 8, "next_id must not collide with existing ids");
            }
            _ => panic!("expected Loaded"),
        }
    }

    #[test]
    fn theme_choice_is_saved_and_restored() {
        let s = temp_storage("theme");
        let mut d = sample();
        d.theme = ThemeName::Midnight;
        s.save(&d).unwrap();
        match s.load() {
            LoadResult::Loaded(back) => assert_eq!(back.theme, ThemeName::Midnight),
            _ => panic!("expected Loaded"),
        }
        // Files from before themes existed open in Classic.
        match parse("(version: 1, dark_mode: true)") {
            LoadResult::Loaded(old) => assert_eq!(old.theme, ThemeName::Classic),
            _ => panic!("old file must load"),
        }
    }

    #[test]
    fn files_with_removed_fields_still_load() {
        // Saved by an earlier build that had reminders and notification settings.
        let old = "(version: 1, tasks: [(id: 1, title: \"Old\", reminder: Some(15), reminded: true)], \
                   settings: (auto_urgent: true, urgent_hours: 24, notifications: true))";
        match parse(old) {
            LoadResult::Loaded(d) => assert_eq!(d.tasks[0].title, "Old"),
            _ => panic!("old files must keep loading"),
        }
    }

    #[test]
    fn backups_are_pruned_and_restorable() {
        let s = temp_storage("backups");
        s.save(&sample()).unwrap();
        for _ in 0..MAX_BACKUPS + 5 {
            s.backup().unwrap();
        }
        let backups = s.list_backups();
        assert_eq!(backups.len(), MAX_BACKUPS);

        fs::write(s.data_path(), "garbage").unwrap();
        s.restore(&backups[0]).unwrap();
        assert!(matches!(s.load(), LoadResult::Loaded(_)));
        let damaged = fs::read_dir(&s.dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains("damaged"));
        assert!(damaged, "the bad file should be kept, not deleted");
    }
}
