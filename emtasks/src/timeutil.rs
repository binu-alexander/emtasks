//! Local-time helpers on top of libc, so we don't need a date crate.

use crate::model::RepeatUnit;

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Copy)]
pub struct Local {
    pub year: i32,
    pub month: i32, // 1-12
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
    pub weekday: i32, // 0 = Sunday
}

pub fn to_local(ts: i64) -> Local {
    // SAFETY: localtime_r writes into the tm we own.
    let tm = unsafe {
        let t: libc::time_t = ts as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    };
    Local {
        year: tm.tm_year + 1900,
        month: tm.tm_mon + 1,
        day: tm.tm_mday,
        hour: tm.tm_hour,
        minute: tm.tm_min,
        weekday: tm.tm_wday,
    }
}

/// Out-of-range fields are normalized (e.g. day 32 rolls into the next month).
pub fn from_local(year: i32, month: i32, day: i32, hour: i32, minute: i32) -> i64 {
    // SAFETY: mktime only reads/normalizes the tm we own.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = year - 1900;
        tm.tm_mon = month - 1;
        tm.tm_mday = day;
        tm.tm_hour = hour;
        tm.tm_min = minute;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as i64
    }
}

pub fn start_of_day(ts: i64) -> i64 {
    let l = to_local(ts);
    from_local(l.year, l.month, l.day, 0, 0)
}

pub fn days_in_month(year: i32, month: i32) -> i32 {
    // Day 0 of next month = last day of this month.
    to_local(from_local(year, month + 1, 0, 12, 0)).day
}

pub fn parse_date(s: &str) -> Option<(i32, i32, i32)> {
    let mut it = s.trim().split('-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: i32 = it.next()?.parse().ok()?;
    let d: i32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1970..=9999).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    (1..=days_in_month(y, m)).contains(&d).then_some((y, m, d))
}

pub fn parse_time(s: &str) -> Option<(i32, i32)> {
    let (h, m) = s.trim().split_once(':')?;
    let h: i32 = h.parse().ok()?;
    let m: i32 = m.parse().ok()?;
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some((h, m))
}

pub fn fmt_date(ts: i64) -> String {
    let l = to_local(ts);
    format!("{:04}-{:02}-{:02}", l.year, l.month, l.day)
}

pub fn fmt_time(ts: i64) -> String {
    let l = to_local(ts);
    format!("{:02}:{:02}", l.hour, l.minute)
}

pub fn fmt_datetime(ts: i64) -> String {
    format!("{} {}", fmt_date(ts), fmt_time(ts))
}

/// "20261012-221530", for file names.
pub fn fmt_stamp(ts: i64) -> String {
    let l = to_local(ts);
    let sec = ts.rem_euclid(60);
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", l.year, l.month, l.day, l.hour, l.minute, sec)
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
pub const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Today 14:00", "Tomorrow", "Fri", "3 Nov", "3 Nov 2027".
pub fn fmt_due(due: i64, all_day: bool, now: i64) -> String {
    let days = ((start_of_day(due) - start_of_day(now)) as f64 / 86_400.0).round() as i64;
    let l = to_local(due);
    let day = match days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => WEEKDAYS[l.weekday as usize].to_string(),
        _ if l.year == to_local(now).year => format!("{} {}", l.day, MONTHS[(l.month - 1) as usize]),
        _ => format!("{} {} {}", l.day, MONTHS[(l.month - 1) as usize], l.year),
    };
    if all_day { day } else { format!("{day} {}", fmt_time(due)) }
}

/// Advance a due date by one repeat step, keeping the local wall-clock time.
pub fn advance(ts: i64, unit: RepeatUnit, every: u32) -> i64 {
    let n = every.max(1) as i32;
    let l = to_local(ts);
    match unit {
        RepeatUnit::None => ts,
        RepeatUnit::Day => from_local(l.year, l.month, l.day + n, l.hour, l.minute),
        RepeatUnit::Week => from_local(l.year, l.month, l.day + 7 * n, l.hour, l.minute),
        RepeatUnit::Month | RepeatUnit::Year => {
            let months = if unit == RepeatUnit::Month { n } else { 12 * n };
            let total = l.year * 12 + (l.month - 1) + months;
            let (y, m) = (total / 12, total % 12 + 1);
            let d = l.day.min(days_in_month(y, m));
            from_local(y, m, d, l.hour, l.minute)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_roundtrip() {
        let ts = from_local(2026, 3, 15, 14, 30);
        let l = to_local(ts);
        assert_eq!((l.year, l.month, l.day, l.hour, l.minute), (2026, 3, 15, 14, 30));
        assert_eq!(fmt_datetime(ts), "2026-03-15 14:30");
    }

    #[test]
    fn parses_dates_and_times() {
        assert_eq!(parse_date("2026-02-28"), Some((2026, 2, 28)));
        assert_eq!(parse_date("2026-02-29"), None);
        assert_eq!(parse_date("2028-02-29"), Some((2028, 2, 29)));
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date("tomorrow"), None);
        assert_eq!(parse_time("09:05"), Some((9, 5)));
        assert_eq!(parse_time("24:00"), None);
    }

    #[test]
    fn repeat_steps() {
        let d = |y, m, day| from_local(y, m, day, 10, 0);
        assert_eq!(advance(d(2026, 1, 30), RepeatUnit::Day, 3), d(2026, 2, 2));
        assert_eq!(advance(d(2026, 1, 1), RepeatUnit::Week, 2), d(2026, 1, 15));
        // Month end is clamped, not rolled over.
        assert_eq!(advance(d(2026, 1, 31), RepeatUnit::Month, 1), d(2026, 2, 28));
        assert_eq!(advance(d(2026, 11, 15), RepeatUnit::Month, 3), d(2027, 2, 15));
        assert_eq!(advance(d(2028, 2, 29), RepeatUnit::Year, 1), d(2029, 2, 28));
    }
}
