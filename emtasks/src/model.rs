use std::collections::BTreeSet;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

pub type TaskId = u64;

/// The four Eisenhower quadrants plus an inbox for incoming tasks.
/// `Unsorted` is shown to the user as "Inbox"; the variant name stays as-is
/// because it's what's written in saved data files.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Section {
    #[default]
    Unsorted,
    Do,
    Schedule,
    Delegate,
    Eliminate,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Unsorted,
        Section::Do,
        Section::Schedule,
        Section::Delegate,
        Section::Eliminate,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Unsorted => "Inbox",
            Section::Do => "Urgent & Important",
            Section::Schedule => "Important, Not Urgent",
            Section::Delegate => "Urgent, Not Important",
            Section::Eliminate => "Not Urgent, Not Important",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Section::Unsorted => "Sort into a quadrant",
            Section::Do => "Do first",
            Section::Schedule => "Schedule",
            Section::Delegate => "Delegate",
            Section::Eliminate => "Eliminate / later",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Section::Unsorted => Color32::from_rgb(150, 150, 160),
            Section::Do => Color32::from_rgb(225, 85, 85),
            Section::Schedule => Color32::from_rgb(80, 140, 230),
            Section::Delegate => Color32::from_rgb(235, 155, 50),
            Section::Eliminate => Color32::from_rgb(120, 170, 120),
        }
    }

    /// Where a task goes when it becomes urgent (importance is kept).
    pub fn urgent_version(self) -> Section {
        match self {
            Section::Unsorted | Section::Schedule => Section::Do,
            Section::Eliminate => Section::Delegate,
            s => s,
        }
    }

    pub fn is_urgent(self) -> bool {
        matches!(self, Section::Do | Section::Delegate)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum RepeatUnit {
    #[default]
    None,
    Day,
    Week,
    Month,
    Year,
}

impl RepeatUnit {
    pub const ALL: [RepeatUnit; 5] =
        [RepeatUnit::None, RepeatUnit::Day, RepeatUnit::Week, RepeatUnit::Month, RepeatUnit::Year];

    pub fn label(self) -> &'static str {
        match self {
            RepeatUnit::None => "Never",
            RepeatUnit::Day => "Daily",
            RepeatUnit::Week => "Weekly",
            RepeatUnit::Month => "Monthly",
            RepeatUnit::Year => "Yearly",
        }
    }

    pub fn unit(self, n: u32) -> &'static str {
        let one = n == 1;
        match self {
            RepeatUnit::None => "",
            RepeatUnit::Day => if one { "day" } else { "days" },
            RepeatUnit::Week => if one { "week" } else { "weeks" },
            RepeatUnit::Month => if one { "month" } else { "months" },
            RepeatUnit::Year => if one { "year" } else { "years" },
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub notes: String,
    pub section: Section,
    pub completed: bool,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    /// Unix seconds. For all-day tasks this is local midnight of the day.
    pub due: Option<i64>,
    pub all_day: bool,
    pub repeat: RepeatUnit,
    pub repeat_every: u32,
    pub tags: BTreeSet<String>,
    /// Move this task to Urgent when the linked task is completed.
    pub wait_for: Option<TaskId>,
    /// Set when the app moved this task to Urgent automatically.
    pub auto_from: Option<Section>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            notes: String::new(),
            section: Section::Unsorted,
            completed: false,
            created_at: 0,
            completed_at: None,
            due: None,
            all_day: false,
            repeat: RepeatUnit::None,
            repeat_every: 1,
            tags: BTreeSet::new(),
            wait_for: None,
            auto_from: None,
        }
    }
}

impl Task {
    /// The moment the task is actually late.
    pub fn deadline(&self) -> Option<i64> {
        self.due.map(|d| if self.all_day { d + 86_399 } else { d })
    }

    pub fn is_overdue(&self, now: i64) -> bool {
        !self.completed && self.deadline().is_some_and(|d| d < now)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum StatusFilter {
    All,
    #[default]
    Active,
    Completed,
}

impl StatusFilter {
    pub const ALL: [StatusFilter; 3] = [StatusFilter::All, StatusFilter::Active, StatusFilter::Completed];

    pub fn label(self) -> &'static str {
        match self {
            StatusFilter::All => "All tasks",
            StatusFilter::Active => "Uncompleted",
            StatusFilter::Completed => "Completed",
        }
    }
}

/// How long a ticked-off task stays in the Uncompleted view before dropping away.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum KeepCompleted {
    #[default]
    Hide,
    ThreeDays,
    Week,
}

impl KeepCompleted {
    pub const ALL: [KeepCompleted; 3] = [KeepCompleted::Hide, KeepCompleted::ThreeDays, KeepCompleted::Week];

    pub fn label(self) -> &'static str {
        match self {
            KeepCompleted::Hide => "Hide right away",
            KeepCompleted::ThreeDays => "Show for 3 days",
            KeepCompleted::Week => "Show for 1 week",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            KeepCompleted::Hide => 0,
            KeepCompleted::ThreeDays => 3 * 86_400,
            KeepCompleted::Week => 7 * 86_400,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortMode {
    #[default]
    Manual,
    Title,
    Due,
}

impl SortMode {
    pub const ALL: [SortMode; 3] = [SortMode::Manual, SortMode::Title, SortMode::Due];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Manual => "Manual",
            SortMode::Title => "Title",
            SortMode::Due => "Due date",
        }
    }
}

#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub tags: BTreeSet<String>,
    pub status: StatusFilter,
    pub sort: SortMode,
}

impl Filter {
    /// `completed_since`: tasks completed at or after this time still count as Uncompleted
    /// (the "keep completed tasks visible" setting); None hides them right away.
    pub fn matches(&self, t: &Task, completed_since: Option<i64>) -> bool {
        let status_ok = match self.status {
            StatusFilter::All => true,
            StatusFilter::Active => {
                !t.completed || completed_since.is_some_and(|since| t.completed_at.is_some_and(|c| c >= since))
            }
            StatusFilter::Completed => t.completed,
        };
        status_ok && (self.tags.is_empty() || self.tags.iter().any(|g| t.tags.contains(g)))
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub filter: Filter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum View {
    #[default]
    Matrix,
    List,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_urgent: bool,
    pub urgent_hours: u32,
    pub show_inbox: bool,
    /// Daily CSV copy in ~/Documents/emTasks.
    pub documents_backup: bool,
    pub keep_completed: KeepCompleted,
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_urgent: true, urgent_hours: 24, show_inbox: true, documents_backup: true, keep_completed: KeepCompleted::Hide }
    }
}

/// Split "Buy milk #home #errands" into a title and tags.
pub fn parse_quick_add(line: &str) -> (String, BTreeSet<String>) {
    let mut tags = BTreeSet::new();
    let mut words = Vec::new();
    for w in line.split_whitespace() {
        match w.strip_prefix('#') {
            Some(tag) if !tag.is_empty() => {
                tags.insert(tag.to_lowercase());
            }
            _ => words.push(w),
        }
    }
    (words.join(" "), tags)
}

pub fn parse_tags(s: &str) -> BTreeSet<String> {
    s.split(',')
        .map(|t| t.trim().trim_start_matches('#').to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_add_extracts_tags() {
        let (title, tags) = parse_quick_add("Buy milk #Home #errands");
        assert_eq!(title, "Buy milk");
        assert_eq!(tags.into_iter().collect::<Vec<_>>(), ["errands", "home"]);
    }

    #[test]
    fn urgency_keeps_importance() {
        assert_eq!(Section::Schedule.urgent_version(), Section::Do);
        assert_eq!(Section::Eliminate.urgent_version(), Section::Delegate);
        assert_eq!(Section::Unsorted.urgent_version(), Section::Do);
    }
}
