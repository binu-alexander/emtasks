//! Export tasks to a simple CSV, and import them back.
//!
//! Columns (header row, any order, case-insensitive; only `title` is required):
//!   title, quadrant, completed, due_date, due_time, all_day, repeat,
//!   repeat_every, tags, notes, created, completed_at
//! A file without a recognisable header is read as one task title per row.

use std::collections::BTreeSet;

use crate::model::*;
use crate::timeutil;

const HEADER: [&str; 12] = [
    "title", "quadrant", "completed", "due_date", "due_time", "all_day", "repeat", "repeat_every", "tags", "notes",
    "created", "completed_at",
];

pub fn quadrant_name(s: Section) -> &'static str {
    match s {
        Section::Unsorted => "Inbox",
        Section::Do => "Do",
        Section::Schedule => "Schedule",
        Section::Delegate => "Delegate",
        Section::Eliminate => "Eliminate",
    }
}

fn parse_quadrant(s: &str) -> Option<Section> {
    let k = s.trim().to_lowercase();
    if k.is_empty() {
        return Some(Section::Unsorted);
    }
    Section::ALL.into_iter().find(|sec| {
        k == quadrant_name(*sec).to_lowercase() || k == sec.title().to_lowercase() || k == sec.subtitle().to_lowercase()
    })
    .or(match k.as_str() {
        "unsorted" => Some(Section::Unsorted),
        "do first" | "urgent" => Some(Section::Do),
        _ => None,
    })
}

fn repeat_name(r: RepeatUnit) -> &'static str {
    match r {
        RepeatUnit::None => "",
        RepeatUnit::Day => "daily",
        RepeatUnit::Week => "weekly",
        RepeatUnit::Month => "monthly",
        RepeatUnit::Year => "yearly",
    }
}

fn parse_repeat(s: &str) -> Option<RepeatUnit> {
    match s.trim().to_lowercase().as_str() {
        "" | "never" | "none" | "no" => Some(RepeatUnit::None),
        "daily" | "day" | "days" => Some(RepeatUnit::Day),
        "weekly" | "week" | "weeks" => Some(RepeatUnit::Week),
        "monthly" | "month" | "months" => Some(RepeatUnit::Month),
        "yearly" | "year" | "years" | "annually" => Some(RepeatUnit::Year),
        _ => None,
    }
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "yes" | "y" | "true" | "1" | "x" | "done" | "✓")
}

// ── Writing ─────────────────────────────────────────────────────

/// Cells starting with these are treated as formulas by Excel/Numbers.
const FORMULA_STARTS: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

/// Guard against spreadsheet formula injection: prefix risky cells with `'`,
/// which spreadsheets show as plain text. `unguard` reverses it on import.
fn guard(s: &str) -> String {
    if s.starts_with(FORMULA_STARTS) { format!("'{s}") } else { s.to_string() }
}

fn unguard(s: String) -> String {
    match s.strip_prefix('\'') {
        Some(rest) if rest.starts_with(FORMULA_STARTS) => rest.to_string(),
        _ => s,
    }
}

fn field(s: &str) -> String {
    let s = &guard(s);
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn export(tasks: &[Task]) -> String {
    let mut out = HEADER.join(",");
    out.push('\n');
    for t in tasks {
        let (date, time) = match t.due {
            Some(d) if t.all_day => (timeutil::fmt_date(d), String::new()),
            Some(d) => (timeutil::fmt_date(d), timeutil::fmt_time(d)),
            None => (String::new(), String::new()),
        };
        let row = [
            t.title.clone(),
            quadrant_name(t.section).to_string(),
            yes(t.completed).to_string(),
            date,
            time,
            if t.due.is_some() { yes(t.all_day).to_string() } else { String::new() },
            repeat_name(t.repeat).to_string(),
            if t.repeat == RepeatUnit::None { String::new() } else { t.repeat_every.to_string() },
            t.tags.iter().cloned().collect::<Vec<_>>().join(" "),
            t.notes.clone(),
            if t.created_at > 0 { timeutil::fmt_datetime(t.created_at) } else { String::new() },
            t.completed_at.map(timeutil::fmt_datetime).unwrap_or_default(),
        ];
        out.push_str(&row.iter().map(|f| field(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

// ── Reading ─────────────────────────────────────────────────────

/// "YYYY-MM-DD HH:MM" or "YYYY-MM-DD" in local time.
fn parse_datetime(s: &str) -> Option<i64> {
    let s = s.trim();
    let (date, time) = s.split_once(' ').unwrap_or((s, "00:00"));
    let (y, m, d) = timeutil::parse_date(date)?;
    let (h, min) = timeutil::parse_time(time)?;
    Some(timeutil::from_local(y, m, d, h, min))
}

/// RFC 4180-style parser: quoted fields, doubled quotes, newlines inside quotes.
fn parse_rows(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    cur.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => cur.push(c),
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ',' => row.push(std::mem::take(&mut cur)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cur));
                rows.push(std::mem::take(&mut row));
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() || !row.is_empty() {
        row.push(cur);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    for row in &mut rows {
        for f in row.iter_mut() {
            *f = unguard(std::mem::take(f));
        }
    }
    rows
}

pub struct Imported {
    pub tasks: Vec<Task>,
    pub warnings: Vec<String>,
}

/// Parse CSV text into tasks (ids are assigned by the caller).
pub fn import(text: &str, now: i64) -> Imported {
    let mut rows = parse_rows(text);
    let mut warnings = Vec::new();
    if rows.is_empty() {
        return Imported { tasks: Vec::new(), warnings: vec!["The file has no rows.".into()] };
    }

    let header: Vec<String> = rows[0].iter().map(|h| h.trim().to_lowercase().replace(' ', "_")).collect();
    let col = |name: &str| header.iter().position(|h| h == name);
    let has_header = col("title").is_some();
    if has_header {
        rows.remove(0);
    }
    let title_col = col("title").unwrap_or(0);
    let get = |r: &Vec<String>, name: &str| -> String {
        if !has_header {
            return String::new();
        }
        col(name).and_then(|i| r.get(i)).map(|s| s.trim().to_string()).unwrap_or_default()
    };

    let mut tasks = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let line = i + if has_header { 2 } else { 1 };
        let raw_title = r.get(title_col).map(|s| s.trim()).unwrap_or("");
        if raw_title.is_empty() {
            warnings.push(format!("Row {line}: no title, skipped."));
            continue;
        }
        let (title, mut tags) = parse_quick_add(raw_title);
        let mut t = Task { title, created_at: now, ..Default::default() };

        let q = get(r, "quadrant");
        t.section = parse_quadrant(&q).unwrap_or_else(|| {
            warnings.push(format!("Row {line}: unknown quadrant \"{q}\", put in the Inbox."));
            Section::Unsorted
        });
        t.completed = parse_bool(&get(r, "completed"));

        let date = get(r, "due_date");
        if !date.is_empty() {
            match timeutil::parse_date(&date) {
                Some((y, m, d)) => {
                    let time = get(r, "due_time");
                    let all_day_col = get(r, "all_day");
                    t.all_day = if all_day_col.is_empty() { time.is_empty() } else { parse_bool(&all_day_col) };
                    let (h, min) = if t.all_day {
                        (0, 0)
                    } else {
                        timeutil::parse_time(&time).unwrap_or_else(|| {
                            if !time.is_empty() {
                                warnings.push(format!("Row {line}: time \"{time}\" isn't HH:MM, used 09:00."));
                            }
                            (9, 0)
                        })
                    };
                    t.due = Some(timeutil::from_local(y, m, d, h, min));
                }
                None => warnings.push(format!("Row {line}: date \"{date}\" isn't YYYY-MM-DD, imported without a due date.")),
            }
        }

        let rep = get(r, "repeat");
        match parse_repeat(&rep) {
            Some(u) => t.repeat = if t.due.is_some() { u } else { RepeatUnit::None },
            None => warnings.push(format!("Row {line}: repeat \"{rep}\" not understood, ignored.")),
        }
        if let Ok(n) = get(r, "repeat_every").parse::<u32>() {
            t.repeat_every = n.clamp(1, 365);
        }

        tags.extend(
            get(r, "tags")
                .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .map(|s| s.trim().trim_start_matches('#').to_lowercase())
                .filter(|s| !s.is_empty()),
        );
        t.tags = tags.into_iter().collect::<BTreeSet<_>>();
        t.notes = get(r, "notes");

        // Keep original dates so a CSV works as a faithful backup.
        if let Some(ts) = parse_datetime(&get(r, "created")) {
            t.created_at = ts;
        }
        if t.completed {
            t.completed_at = Some(parse_datetime(&get(r, "completed_at")).unwrap_or(now));
        }
        tasks.push(t);
    }
    Imported { tasks, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeutil::from_local;

    fn sample() -> Vec<Task> {
        let mut a = Task { id: 1, title: "Pay rent, \"urgent\"".into(), section: Section::Do, ..Default::default() };
        a.due = Some(from_local(2026, 11, 1, 18, 30));
        a.tags = ["home".to_string(), "money".to_string()].into();
        a.notes = "Line one\nLine two".into();
        let mut b = Task { id: 2, title: "Weekly review".into(), section: Section::Schedule, ..Default::default() };
        b.due = Some(from_local(2026, 11, 6, 0, 0));
        b.all_day = true;
        b.repeat = RepeatUnit::Week;
        b.repeat_every = 2;
        let mut c = Task { id: 3, title: "Old idea".into(), section: Section::Eliminate, completed: true, ..Default::default() };
        c.created_at = from_local(2026, 1, 2, 8, 15);
        c.completed_at = Some(from_local(2026, 3, 4, 17, 45));
        let d = Task { id: 4, title: "Unsorted thought".into(), ..Default::default() };
        vec![a, b, c, d]
    }

    #[test]
    fn hostile_csv_never_crashes_the_import() {
        let nasty = [
            "", "\"", "\"\"\"", ",,,,", "\n\n\n", "\r\r\n\r", "title\n\"unterminated",
            "title,due_date,due_time,repeat_every,quadrant,completed\nx,9999-99-99,99:99,-1,nowhere,maybe",
            "title,due_date\nx,2026-02-30", "title,repeat,repeat_every\nx,weekly,99999999999999999999",
            "title\n\u{0}\u{feff}é\u{1F600}\u{202E}", "\u{feff}title\nbom", "title,title,title\na,b,c",
            "title,tags\nx,\"#a,#b,,#,##\"", "title,due_time\nx,2400", "title,due_date\nx,0000-01-01",
            "title,due_date\nx,-99999-01-01", "title,created,completed_at\nx,abc,-",
        ];
        for text in nasty {
            let _ = import(text, 1_700_000_000);
        }
        // Pseudo-random bytes made of CSV-significant characters (deterministic).
        let alphabet: Vec<char> = "a,\"\n\r#=+-@:; 0129é\u{0}\t".chars().collect();
        let mut seed: u64 = 42;
        for _ in 0..2_000 {
            let len = (seed % 200) as usize;
            let text: String = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    alphabet[(seed >> 33) as usize % alphabet.len()]
                })
                .collect();
            let _ = import(&format!("title,due_date,due_time,tags\n{text}"), 1_700_000_000);
            let _ = import(&text, 1_700_000_000);
        }
    }

    #[test]
    fn export_then_import_round_trips() {
        let original = sample();
        let back = import(&export(&original), 0);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(back.tasks.len(), original.len());
        for (o, b) in original.iter().zip(&back.tasks) {
            assert_eq!(o.title, b.title);
            assert_eq!(o.section, b.section);
            assert_eq!(o.completed, b.completed);
            assert_eq!(o.due, b.due, "{}", o.title);
            assert_eq!(o.all_day, b.all_day);
            assert_eq!(o.repeat, b.repeat);
            assert_eq!(o.tags, b.tags);
            assert_eq!(o.notes, b.notes);
            if o.created_at > 0 {
                assert_eq!(o.created_at, b.created_at, "created date kept");
            }
            assert_eq!(o.completed_at, b.completed_at, "completed date kept");
        }
        assert_eq!(back.tasks[1].repeat_every, 2);
    }

    #[test]
    fn formula_like_cells_are_exported_as_text_and_restored() {
        let titles = ["=HYPERLINK(\"http://evil\",\"x\")", "+1 call", "-2 days left", "@home", "normal"];
        let tasks: Vec<Task> = titles
            .iter()
            .map(|t| Task { title: t.to_string(), notes: "=1+1".into(), ..Default::default() })
            .collect();
        let csv = export(&tasks);
        for line in csv.lines().skip(1) {
            let first = line.trim_start_matches('"');
            assert!(!first.starts_with(FORMULA_STARTS), "unsafe cell in export: {line}");
            assert!(!line.contains(",=1+1"), "notes guarded too: {line}");
        }
        let back = import(&csv, 0);
        let got: Vec<_> = back.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(got, titles, "titles round-trip unchanged");
        assert!(back.tasks.iter().all(|t| t.notes == "=1+1"));
    }

    #[test]
    fn simple_one_column_list_works() {
        let r = import("Buy milk\nCall mum #family\n\n", 0);
        assert_eq!(r.tasks.len(), 2);
        assert_eq!(r.tasks[1].title, "Call mum");
        assert!(r.tasks[1].tags.contains("family"));
        assert!(r.tasks.iter().all(|t| t.section == Section::Unsorted));
    }

    #[test]
    fn spreadsheet_style_headers_and_quadrant_names() {
        let csv = "\u{feff}Title,Quadrant,Due Date,Tags\r\nA,Urgent & Important,2026-12-01,\"work, q4\"\r\nB,schedule,,\r\nC,delegate,,\r\nD,unsorted,,\r\n";
        let r = import(csv, 0);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        let secs: Vec<_> = r.tasks.iter().map(|t| t.section).collect();
        assert_eq!(secs, [Section::Do, Section::Schedule, Section::Delegate, Section::Unsorted]);
        assert!(r.tasks[0].all_day, "date without time is all-day");
        assert_eq!(r.tasks[0].tags.len(), 2);
    }

    #[test]
    fn bad_values_are_reported_not_fatal() {
        let csv = "title,quadrant,due_date,repeat\nX,Someday,31/12/2026,fortnightly\n,Do,,\nY,Do,2026-02-30,\n";
        let r = import(csv, 0);
        assert_eq!(r.tasks.len(), 2);
        assert_eq!(r.tasks[0].section, Section::Unsorted);
        assert_eq!(r.tasks[0].due, None);
        assert_eq!(r.tasks[1].due, None, "Feb 30 rejected");
        assert_eq!(r.warnings.len(), 5, "{:?}", r.warnings);
    }
}
