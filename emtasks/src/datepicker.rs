//! A small month-grid calendar for picking a due date.

use eframe::egui::{self, Id, RichText, Ui};

use crate::timeutil::{days_in_month, from_local, now, to_local, MONTHS};

/// Returns the picked (year, month, day), if a day was clicked.
pub fn calendar(ui: &mut Ui, id: Id, selected: Option<(i32, i32, i32)>) -> Option<(i32, i32, i32)> {
    let today = to_local(now());
    let today_ymd = (today.year, today.month, today.day);
    let (mut y, mut m): (i32, i32) = ui
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| selected.map_or((today.year, today.month), |(y, m, _)| (y, m)));
    let mut picked = None;

    ui.horizontal(|ui| {
        if ui.small_button("◀").clicked() {
            (y, m) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
        }
        ui.add_sized([120.0, 18.0], egui::Label::new(RichText::new(format!("{} {y}", MONTHS[(m - 1) as usize])).strong()));
        if ui.small_button("▶").clicked() {
            (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        }
    });

    egui::Grid::new(id.with("grid")).spacing([2.0, 2.0]).show(ui, |ui| {
        for h in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
            ui.label(RichText::new(h).small().weak());
        }
        ui.end_row();

        // Weekday of the 1st, Monday-based.
        let offset = (to_local(from_local(y, m, 1, 12, 0)).weekday + 6) % 7;
        for _ in 0..offset {
            ui.label("");
        }
        let mut col = offset;
        for d in 1..=days_in_month(y, m) {
            let mut text = RichText::new(format!("{d:>2}"));
            if (y, m, d) == today_ymd {
                text = text.strong().underline();
            }
            let resp = ui.add_sized([26.0, 20.0], egui::SelectableLabel::new(selected == Some((y, m, d)), text));
            if resp.clicked() {
                picked = Some((y, m, d));
            }
            col += 1;
            if col == 7 {
                ui.end_row();
                col = 0;
            }
        }
    });

    if ui.small_button("Today").clicked() {
        picked = Some(today_ymd);
    }

    ui.data_mut(|d| d.insert_temp(id, (y, m)));
    picked
}
