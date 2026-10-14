use eframe::egui::{
    self, Color32, CursorIcon, Frame, Label, Margin, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder,
};

use crate::app::{apply_due, Action, App, Recovery};
use crate::theme::{self, ThemeName};
use crate::{datepicker, platform};
use crate::model::*;
use crate::timeutil::{self, now};

const GAP: f32 = 8.0;

impl App {
    // ── Top menu ────────────────────────────────────────────────

    pub fn ui_menu(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").frame(self.bar_frame(ctx)).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New task").shortcut_text("⌘N")).clicked() {
                        self.ui.focus_quick_add = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Back up now").clicked() {
                        self.backup_now();
                        ui.close_menu();
                    }
                    if ui.button("Export tasks to CSV").on_hover_text("Saves all tasks to your Downloads folder").clicked() {
                        self.export_csv();
                        ui.close_menu();
                    }
                    if ui.button("Import tasks from CSV…").on_hover_text("Adds tasks; never removes any. You can also drop a .csv on the window.").clicked() {
                        ui.close_menu();
                        if let Some(p) = platform::choose_csv_file() {
                            self.import_csv_file(&p);
                        }
                    }
                    if ui.button("Show data in Finder").clicked() {
                        if let Some(s) = &self.storage {
                            platform::reveal_in_finder(&s.data_path());
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Settings…").shortcut_text("⌘,")).clicked() {
                        self.ui.show_settings = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Quit").shortcut_text("⌘Q")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(self.history.can_undo(), egui::Button::new("Undo").shortcut_text("⌘Z"))
                        .clicked()
                    {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.history.can_redo(), egui::Button::new("Redo").shortcut_text("Shift⌘Z"))
                        .clicked()
                    {
                        self.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    let sel = self.ui.selected;
                    if ui
                        .add_enabled(sel.is_some(), egui::Button::new("Complete / reopen task").shortcut_text("⌘↩"))
                        .clicked()
                    {
                        if let Some(id) = sel {
                            let done = self.task(id).is_some_and(|t| t.completed);
                            self.set_completed(id, !done);
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(sel.is_some(), egui::Button::new("Delete task…").shortcut_text("Delete")).clicked() {
                        self.ui.confirm_delete = sel;
                        ui.close_menu();
                    }
                    ui.add_enabled_ui(sel.is_some(), |ui| {
                        ui.menu_button("Move to", |ui| {
                            for (i, s) in [Section::Unsorted, Section::Do, Section::Schedule, Section::Delegate, Section::Eliminate]
                                .into_iter()
                                .enumerate()
                            {
                                if ui.add(egui::Button::new(s.title()).shortcut_text(format!("⌘{i}"))).clicked() {
                                    if let Some(id) = sel {
                                        self.move_task(id, s, None);
                                    }
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                });
                ui.menu_button("View", |ui| {
                    ui.radio_value(&mut self.view, View::Matrix, "Matrix");
                    ui.radio_value(&mut self.view, View::List, "List of sections");
                    ui.weak("Toggle with ⌘L");
                    let mut show = self.settings.show_inbox;
                    if ui.checkbox(&mut show, "Show Inbox  (⌘I)").changed() {
                        self.set_inbox_visible(show);
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Theme", |ui| {
                        for t in ThemeName::ALL {
                            if ui.radio(self.theme == t, format!("{}  —  {}", t.label(), t.description())).clicked() {
                                self.set_theme(t);
                                ui.close_menu();
                            }
                        }
                    });
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Keyboard shortcuts").clicked() {
                        self.ui.show_shortcuts = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    // ── Toolbar: view switch, quick add, sort / filter / presets ─

    pub fn ui_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").frame(self.bar_frame(ctx)).show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Says exactly what it will do, so it doesn't read like a third view tab.
                let n = self.inbox_count();
                let inbox_label = match (self.settings.show_inbox, n) {
                    (true, _) => "📥 Hide Inbox".to_string(),
                    (false, 0) => "📥 Show Inbox".to_string(),
                    (false, n) => format!("📥 Show Inbox ({n})"),
                };
                let toggle = ui.button(inbox_label).on_hover_text("Show / hide the Inbox (⌘I)");
                self.ui.inbox_toggle = Some(toggle.rect);
                if toggle.clicked() {
                    self.set_inbox_visible(!self.settings.show_inbox);
                }
                ui.separator();
                ui.selectable_value(&mut self.view, View::Matrix, "⊞ Matrix");
                ui.selectable_value(&mut self.view, View::List, "📋 List");
                ui.separator();
                self.ui_quick_add(ui);
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Sort");
                egui::ComboBox::from_id_salt("sort")
                    .selected_text(self.filter.sort.label())
                    .show_ui(ui, |ui| {
                        for s in SortMode::ALL {
                            ui.selectable_value(&mut self.filter.sort, s, s.label());
                        }
                    });

                ui.label("Show");
                egui::ComboBox::from_id_salt("status")
                    .selected_text(self.filter.status.label())
                    .show_ui(ui, |ui| {
                        for s in StatusFilter::ALL {
                            ui.selectable_value(&mut self.filter.status, s, s.label());
                        }
                    });

                let tags_label = if self.filter.tags.is_empty() {
                    "🏷 All tags".to_string()
                } else {
                    format!("🏷 {}", self.filter.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" "))
                };
                ui.menu_button(tags_label, |ui| {
                    let all = self.all_tags();
                    if all.is_empty() {
                        ui.weak("No tags yet. Add #tags to tasks.");
                    }
                    for tag in all {
                        let mut on = self.filter.tags.contains(&tag);
                        if ui.checkbox(&mut on, format!("#{tag}")).changed() {
                            if on {
                                self.filter.tags.insert(tag);
                            } else {
                                self.filter.tags.remove(&tag);
                            }
                        }
                    }
                    if !self.filter.tags.is_empty() {
                        ui.separator();
                        if ui.button("Clear tag filter").clicked() {
                            self.filter.tags.clear();
                            ui.close_menu();
                        }
                    }
                });

                ui.menu_button("⭐ Presets", |ui| {
                    let mut apply = None;
                    let mut delete = None;
                    for (i, p) in self.presets.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let active = self.filter == p.filter;
                            if ui.selectable_label(active, &p.name).clicked() {
                                apply = Some(i);
                            }
                            if ui.small_button("🗑").on_hover_text("Delete preset").clicked() {
                                delete = Some(i);
                            }
                        });
                    }
                    if self.presets.is_empty() {
                        ui.weak("No presets saved yet.");
                    }
                    ui.separator();
                    if ui.button("💾 Save current filter…").clicked() {
                        self.ui.show_save_preset = true;
                        self.ui.preset_name.clear();
                        ui.close_menu();
                    }
                    if let Some(i) = apply {
                        self.filter = self.presets[i].filter.clone();
                        self.ui.status = format!("Applied preset \"{}\"", self.presets[i].name);
                        ui.close_menu();
                    }
                    if let Some(i) = delete {
                        self.presets.remove(i);
                    }
                });

                if self.filter != Filter::default() && ui.button("✖ Reset").on_hover_text("Reset sort and filters, showing all tasks").clicked() {
                    self.filter = Filter::default();
                }

                self.ui.toolbar_left_end = ui.cursor().left();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let gear = ui.button("⚙").on_hover_text("Settings");
                    self.ui.settings_button = Some(gear.rect);
                    if gear.clicked() {
                        self.ui.show_settings = true;
                    }
                    self.ui_quadrant_total(ui);
                });
            });
            ui.add_space(4.0);
        });
    }

    fn ui_quick_add(&mut self, ui: &mut Ui) {
        let id = egui::Id::new("quick_add");

        // Pasting multi-line text creates one task per line.
        for text in take_multiline_paste(ui, id) {
            let n = self.add_lines(&text, Section::Unsorted);
            self.ui.status = format!("Added {n} tasks to the Inbox");
        }

        // Place the button first (from the right edge), then let the text box take
        // whatever width is left, so nothing is pushed off-screen on small windows.
        let (resp, add_clicked) = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add = ui.button("➕ Add");
                self.ui.quick_add_button = Some(add.rect);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.ui.quick_add)
                        .id(id)
                        .desired_width(ui.available_width())
                        .hint_text("Add a task to the Inbox…  (⌘N · #tags · paste many lines)"),
                );
                (resp, add.clicked())
            })
            .inner;
        if std::mem::take(&mut self.ui.focus_quick_add) {
            resp.request_focus();
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (add_clicked || enter) && !self.ui.quick_add.trim().is_empty() {
            let text = std::mem::take(&mut self.ui.quick_add);
            self.add_task(text.trim(), Section::Unsorted);
            self.ui.status = if self.settings.show_inbox {
                "Task added to the Inbox".into()
            } else {
                "Task added to the Inbox (hidden — ⌘I to show it)".into()
            };
            if enter {
                resp.request_focus();
            }
        }
    }

    // ── Status bar ──────────────────────────────────────────────

    /// A hard-to-miss bar whenever a tag/status filter is hiding tasks,
    /// so a forgotten filter never looks like lost data.
    pub fn ui_filter_banner(&mut self, ui: &mut Ui) {
        let hidden = self.hidden_by_filter();
        if hidden == 0 {
            return;
        }
        let mut parts = Vec::new();
        if !self.filter.tags.is_empty() {
            parts.push(self.filter.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" "));
        }
        if self.filter.status != StatusFilter::default() {
            parts.push(self.filter.status.label().to_lowercase());
        }
        let warn = ui.visuals().warn_fg_color;
        Frame::none()
            .fill(warn.gamma_multiply(0.12))
            .stroke(Stroke::new(1.0_f32, warn))
            .rounding(6.0)
            .inner_margin(Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let what = if hidden == 1 { "1 task is hidden".to_string() } else { format!("{hidden} tasks are hidden") };
                    ui.label(
                        RichText::new(format!("🔍 Filter on: showing only {}. {what}.", parts.join(", ")))
                            .color(warn)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let b = ui.button("Show all tasks");
                        self.ui.show_all_button = Some(b.rect);
                        if b.clicked() {
                            self.show_all_tasks();
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }

    /// "All time: X of Y done" next to ⚙, for tasks in the four quadrants (Inbox not included).
    fn ui_quadrant_total(&mut self, ui: &mut Ui) {
        let (done, all) = self.quadrant_progress();
        let r = ui
            .label(RichText::new(format!("All time: {done} of {all} done")).strong())
            .on_hover_text(format!(
                "Every task in the four boxes, including completed ones that are hidden. \
                 {} still to do. Inbox tasks count once you sort them.",
                all - done
            ));
        self.ui.quadrant_total = Some(r.rect);
        ui.add_space(4.0);
    }

    /// Frame for the menu bar, toolbar, status bar and editor panel.
    fn bar_frame(&self, ctx: &egui::Context) -> Frame {
        let frame = Frame::side_top_panel(&ctx.style());
        if self.theme.is_classic() {
            frame
        } else {
            let p = theme::palette(self.theme);
            frame.fill(p.panel).stroke(Stroke::new(1.0_f32, p.border))
        }
    }

    pub fn ui_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").frame(self.bar_frame(ctx)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.ui.save_error {
                    ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {err}"));
                    ui.separator();
                }
                ui.label(if self.ui.status.is_empty() { "Ready" } else { &self.ui.status });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ts = now();
                    // The toolbar shows all-time progress; this counts only what's on screen now.
                    let (open, done) = self.on_screen_counts();
                    let overdue = self.tasks.iter().filter(|t| t.is_overdue(ts)).count();
                    // Right-to-left: the first item sits in the bottom-right corner.
                    ui.weak(format!("emTasks {} by B. Alexander", env!("CARGO_PKG_VERSION")));
                    ui.separator();
                    if overdue > 0 {
                        ui.colored_label(ui.visuals().error_fg_color, format!("{overdue} overdue"));
                    }
                    if overdue > 0 {
                        ui.separator();
                    }
                    ui.label(format!("Showing {open} open · {done} done"))
                        .on_hover_text("Tasks in the sections on screen now, after filters. Completed tasks show here only while the Show setting keeps them visible.");
                });
            });
        });
    }

    // ── Views ───────────────────────────────────────────────────

    pub fn ui_matrix(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let r = ui.available_rect_before_wrap();
        ui.allocate_rect(r, Sense::hover());

        if let Some(s) = self.ui.maximized {
            self.section_in_rect(ui, s, r, actions);
            return;
        }

        let show_inbox = self.settings.show_inbox;
        let left_w = (r.width() * 0.24).clamp(210.0, 320.0);
        let left = Rect::from_min_max(r.min, egui::pos2(r.min.x + left_w, r.max.y));
        // With the Inbox hidden, the quadrants take the full width.
        let gx = if show_inbox { left.max.x + GAP } else { r.min.x };
        let gw = (r.max.x - gx - GAP) / 2.0;
        let gh = (r.height() - GAP) / 2.0;
        let cell = |col: f32, row: f32| {
            Rect::from_min_size(
                egui::pos2(gx + col * (gw + GAP), r.min.y + row * (gh + GAP)),
                egui::vec2(gw, gh),
            )
        };

        if show_inbox {
            self.section_in_rect(ui, Section::Unsorted, left, actions);
        }
        self.section_in_rect(ui, Section::Do, cell(0.0, 0.0), actions);
        self.section_in_rect(ui, Section::Schedule, cell(1.0, 0.0), actions);
        self.section_in_rect(ui, Section::Delegate, cell(0.0, 1.0), actions);
        self.section_in_rect(ui, Section::Eliminate, cell(1.0, 1.0), actions);
    }

    pub fn ui_list(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if let Some(s) = self.ui.maximized {
            let r = ui.available_rect_before_wrap();
            ui.allocate_rect(r, Sense::hover());
            self.section_in_rect(ui, s, r, actions);
            return;
        }
        ScrollArea::vertical().auto_shrink(false).drag_to_scroll(false).show(ui, |ui| {
            for s in self.shown_sections() {
                self.section_panel(ui, s, actions, false);
                ui.add_space(GAP);
            }
        });
    }

    fn section_in_rect(&mut self, ui: &mut Ui, s: Section, rect: Rect, actions: &mut Vec<Action>) {
        ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
            self.section_panel(ui, s, actions, true);
        });
    }

    /// One section box: header + tasks. `fill` = take all space and scroll inside.
    fn section_panel(&mut self, ui: &mut Ui, s: Section, actions: &mut Vec<Action>, fill: bool) {
        let tasks = self.visible_tasks(s);
        let dragging = egui::DragAndDrop::has_payload_of_type::<TaskId>(ui.ctx());
        let color = theme::section_color(self.theme, s);

        let classic = self.theme.is_classic();
        let pal = theme::palette(self.theme);
        let card = if classic {
            Frame::group(ui.style())
                .inner_margin(Margin::same(8.0))
                .rounding(6.0)
                .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.5)))
        } else {
            Frame::none()
                .fill(pal.card)
                .rounding(10.0)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .inner_margin(egui::Margin { left: 10.0, right: 10.0, top: 12.0, bottom: 8.0 })
                .shadow(egui::Shadow {
                    offset: egui::vec2(0.0, 2.0),
                    blur: 10.0,
                    spread: 0.0,
                    color: Color32::from_black_alpha(if pal.dark { 60 } else { 14 }),
                })
        };
        let resp = card
            .show(ui, |ui| {
                if fill {
                    ui.set_min_size(ui.available_size());
                } else {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(70.0);
                }

                // Header: controls are laid out first from the right edge, then the
                // name takes what's left (truncated if needed) and the subtitle only
                // appears when there's room, so nothing ever overlaps.
                let mut parts = Vec::new();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let maxed = self.ui.maximized == Some(s);
                        let (icon, tip) = if maxed { ("🗕", "Restore (Esc)") } else { ("⛶", "Maximize") };
                        let max = ui.small_button(icon).on_hover_text(tip);
                        parts.push(max.rect);
                        if max.clicked() {
                            actions.push(Action::ToggleMaximize(s));
                        }
                        let add = ui.small_button("➕").on_hover_text(format!("Add a task to {}", s.title()));
                        self.ui.add_buttons.insert(s, add.rect);
                        parts.push(add.rect);
                        if add.clicked() {
                            if self.ui.adding_to == Some(s) {
                                self.close_add();
                            } else {
                                self.open_add(s);
                            }
                        }
                        parts.push(ui.label(RichText::new(tasks.len().to_string()).weak()).rect);
                        let overdue = tasks.iter().filter(|t| t.is_overdue(now())).count();
                        if overdue > 0 {
                            let o = ui.label(
                                RichText::new(format!("{overdue} overdue"))
                                    .small()
                                    .color(ui.visuals().error_fg_color.gamma_multiply(0.8)),
                            );
                            parts.push(o.rect);
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            parts.push(ui.label(RichText::new("⏺").color(color)).rect);
                            let title = ui.add(Label::new(RichText::new(s.title()).strong().color(color)).truncate());
                            parts.push(title.rect);
                            let sub = RichText::new(s.subtitle()).weak().small();
                            let sub_w = ui
                                .fonts(|f| {
                                    f.layout_no_wrap(s.subtitle().to_string(), egui::TextStyle::Small.resolve(ui.style()), Color32::WHITE)
                                })
                                .size()
                                .x;
                            if ui.available_width() > sub_w + 6.0 {
                                parts.push(ui.label(sub).rect);
                            }
                        });
                    });
                });
                self.ui.header_parts.insert(s, parts);
                ui.add(egui::Separator::default().spacing(4.0));
                if self.ui.adding_to == Some(s) {
                    self.ui_section_add(ui, s);
                }

                if fill {
                    ScrollArea::vertical()
                        .id_salt(("section", s))
                        .auto_shrink(false)
                        .drag_to_scroll(false)
                        .show(ui, |ui| self.section_rows(ui, s, &tasks, actions));
                } else {
                    self.section_rows(ui, s, &tasks, actions);
                }
            })
            .response;

        if !classic {
            // Coloured strip along the top edge of the card.
            let r = resp.rect;
            let strip = Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + 4.0));
            let rounding = egui::Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 };
            ui.painter().rect_filled(strip, rounding, color);
        }
        self.ui.section_rects.insert(s, resp.rect);
        if dragging && resp.contains_pointer() {
            if self.ui.drop_target.is_none() {
                self.ui.drop_target = Some((s, None));
            }
            ui.painter().rect_stroke(resp.rect, 6.0, Stroke::new(2.0_f32, color));
        }
    }

    /// Inline "add a task" box inside a section.
    fn ui_section_add(&mut self, ui: &mut Ui, s: Section) {
        let id = egui::Id::new(("section_add", s));
        for text in take_multiline_paste(ui, id) {
            let n = self.add_lines(&text, s);
            self.ui.status = format!("Added {n} tasks to {}", s.title());
        }
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.ui.section_input)
                .id(id)
                .desired_width(f32::INFINITY)
                .hint_text(format!("New task in {}…  (Enter to add · Esc to close)", s.subtitle())),
        );
        if std::mem::take(&mut self.ui.focus_section_input) {
            resp.request_focus();
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter {
            let text = std::mem::take(&mut self.ui.section_input);
            if text.trim().is_empty() {
                self.close_add();
            } else {
                self.add_task(text.trim(), s);
                self.ui.status = format!("Added to {}", s.title());
                // Stay open for the next one.
                resp.request_focus();
            }
        } else if resp.lost_focus() && self.ui.section_input.trim().is_empty() {
            // Clicked away from an empty box.
            self.close_add();
        }
        ui.add_space(2.0);
    }

    fn section_rows(&mut self, ui: &mut Ui, s: Section, tasks: &[Task], actions: &mut Vec<Action>) {
        if tasks.is_empty() {
            ui.add_space(6.0);
            ui.weak("Drop tasks here");
        }
        for t in tasks {
            self.task_row(ui, s, t, actions);
        }
    }

    fn task_row(&mut self, ui: &mut Ui, s: Section, t: &Task, actions: &mut Vec<Action>) {
        let ts = now();
        let dragging = egui::DragAndDrop::payload::<TaskId>(ui.ctx()).map(|p| *p);
        let selected = self.ui.selected == Some(t.id);
        let overdue = t.is_overdue(ts);
        let overdue_color = ui.visuals().error_fg_color;
        let fill = if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.35)
        } else if dragging == Some(t.id) {
            ui.visuals().faint_bg_color
        } else if overdue {
            overdue_color.gamma_multiply(0.07)
        } else if !self.theme.is_classic()
            && self.ui.prev_row_rects.get(&t.id).is_some_and(|r| ui.rect_contains_pointer(*r))
        {
            theme::palette(self.theme).hover
        } else {
            Color32::TRANSPARENT
        };

        // The whole row can be clicked (opens the task) or dragged (moves it). Its
        // area is registered *before* the row's contents (using last frame's
        // position) so it sits underneath: the checkbox / title / 🗑 on top keep
        // their own clicks. A drag only begins once the mouse has really moved,
        // so a plain click never picks the task up.
        if let Some(r) = self.ui.prev_row_rects.get(&t.id).copied() {
            let row = ui.interact(r, egui::Id::new(("task_row", t.id)), Sense::click_and_drag());
            if row.clicked() {
                actions.push(Action::Select(t.id));
            }
            if is_real_drag(ui, &row) {
                actions.push(Action::StartDrag(t.id));
            }
        }

        let resp = Frame::none()
            .fill(fill)
            .rounding(if self.theme.is_classic() { 4.0 } else { 6.0 })
            .inner_margin(Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let handle = ui
                        .add(Label::new(RichText::new("☰").weak()).sense(Sense::drag()))
                        .on_hover_cursor(CursorIcon::Grab)
                        .on_hover_text("Drag to move");
                    if is_real_drag(ui, &handle) {
                        actions.push(Action::StartDrag(t.id));
                    }

                    let mut done = t.completed;
                    if ui.checkbox(&mut done, "").changed() {
                        actions.push(Action::SetDone(t.id, done));
                    }

                    let mut title = RichText::new(&t.title);
                    if t.completed {
                        title = title.strikethrough().weak();
                    }
                    // Unsorted rows get an instant-delete button for quick triage.
                    let quick_delete = s == Section::Unsorted;
                    let title_w = (ui.available_width() - if quick_delete { 26.0 } else { 0.0 }).max(40.0);
                    ui.scope(|ui| {
                        ui.set_max_width(title_w);
                        if ui.add(Label::new(title).sense(Sense::click()).selectable(false).truncate()).clicked() {
                            actions.push(Action::Select(t.id));
                        }
                    });
                    if quick_delete {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let del = ui
                                .add(egui::Button::new(RichText::new("🗑").small().weak()).frame(false))
                                .on_hover_text("Delete now (⌘Z to undo)");
                            if del.clicked() {
                                actions.push(Action::Delete(t.id));
                            }
                        });
                    }
                });

                let has_meta = t.due.is_some() || !t.tags.is_empty() || t.wait_for.is_some();
                if has_meta {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(34.0);
                        if let Some(due) = t.due {
                            let when = timeutil::fmt_due(due, t.all_day, ts);
                            if overdue {
                                ui.label(RichText::new(format!("📅 {when} · overdue")).small().color(overdue_color));
                            } else {
                                ui.label(RichText::new(format!("📅 {when}")).small().weak());
                            }
                        }
                        if t.repeat != RepeatUnit::None {
                            ui.label(RichText::new("🔁").small()).on_hover_text(format!(
                                "Repeats every {} {}",
                                t.repeat_every,
                                t.repeat.unit(t.repeat_every)
                            ));
                        }
                        if let Some(w) = t.wait_for.and_then(|w| self.task(w)) {
                            ui.label(RichText::new("🔗").small())
                                .on_hover_text(format!("Becomes urgent when \"{}\" is done", w.title));
                        }
                        if t.auto_from.is_some() {
                            ui.label(RichText::new("⚡").small()).on_hover_text("Moved here automatically");
                        }
                        for tag in &t.tags {
                            ui.label(RichText::new(format!("#{tag}")).small().color(Color32::from_rgb(130, 160, 220)));
                        }
                    });
                }
            })
            .response;

        self.ui.row_rects.insert(t.id, resp.rect);

        if overdue {
            let r = resp.rect;
            ui.painter().vline(r.left() + 1.0, r.y_range().shrink(3.0), Stroke::new(2.0_f32, overdue_color.gamma_multiply(0.7)));
        }

        if let Some(d) = dragging {
            if d != t.id && resp.contains_pointer() {
                self.ui.drop_target = Some((s, Some(t.id)));
                if self.filter.sort == SortMode::Manual {
                    let y = resp.rect.top();
                    ui.painter().hline(resp.rect.x_range(), y, Stroke::new(2.0_f32, theme::section_color(self.theme, s)));
                }
            }
        }
    }

    // ── Task editor (right panel) ───────────────────────────────

    pub fn ui_editor(&mut self, ctx: &egui::Context) {
        let Some(id) = self.ui.selected else { return };
        let Some(idx) = self.tasks.iter().position(|t| t.id == id) else {
            self.ui.selected = None;
            return;
        };

        if self.ui.edit.id != Some(id) {
            let t = &self.tasks[idx];
            self.ui.edit.id = Some(id);
            self.ui.edit.date = t.due.map(timeutil::fmt_date).unwrap_or_default();
            self.ui.edit.time = t.due.filter(|_| !t.all_day).map(timeutil::fmt_time).unwrap_or_default();
            self.ui.edit.tags = t.tags.iter().cloned().collect::<Vec<_>>().join(", ");
            self.ui.edit.error = None;
        }

        // Other open tasks this one can wait on.
        let candidates: Vec<(TaskId, String)> = self
            .tasks
            .iter()
            .filter(|o| o.id != id && !o.completed)
            .map(|o| (o.id, o.title.clone()))
            .collect();
        // Every tag in use, offered as one-click chips under the Tags field.
        let known_tags = self.all_tags();

        let mut close = false;
        let mut delete = false;
        let mut set_done = None;

        let editor_frame = self.bar_frame(ctx).inner_margin(egui::Margin::same(12.0));
        egui::SidePanel::right("editor").frame(editor_frame).default_width(330.0).min_width(280.0).show(ctx, |ui| {
            let ts = now();
            let t = &mut self.tasks[idx];
            let edit = &mut self.ui.edit;

            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Task");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🗙").on_hover_text("Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add(
                    egui::TextEdit::singleline(&mut t.title)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Heading),
                );

                ui.horizontal(|ui| {
                    let mut done = t.completed;
                    if ui.checkbox(&mut done, "Completed").changed() {
                        set_done = Some(done);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Section");
                    egui::ComboBox::from_id_salt("edit_section")
                        .selected_text(t.section.title())
                        .show_ui(ui, |ui| {
                            for s in Section::ALL {
                                if ui.selectable_value(&mut t.section, s, s.title()).changed() {
                                    t.auto_from = None;
                                }
                            }
                        });
                });

                ui.label("Notes");
                ui.add(egui::TextEdit::multiline(&mut t.notes).desired_rows(3).desired_width(f32::INFINITY));

                ui.separator();
                ui.strong("Due");
                let mut due_changed = false;
                ui.horizontal(|ui| {
                    due_changed |= ui
                        .add(egui::TextEdit::singleline(&mut edit.date).desired_width(90.0).hint_text("YYYY-MM-DD"))
                        .changed();
                    let picker_id = ui.make_persistent_id(("due_picker", t.id));
                    let btn = ui.button("📅").on_hover_text("Pick a date");
                    if btn.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(picker_id));
                    }
                    egui::popup_below_widget(ui, picker_id, &btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                        ui.set_min_width(200.0);
                        let current = timeutil::parse_date(&edit.date);
                        if let Some((y, m, d)) = datepicker::calendar(ui, picker_id.with("cal"), current) {
                            edit.date = format!("{y:04}-{m:02}-{d:02}");
                            if edit.time.trim().is_empty() {
                                t.all_day = true;
                            }
                            due_changed = true;
                            ui.memory_mut(|m| m.close_popup());
                        }
                    });
                    ui.add_enabled_ui(!t.all_day, |ui| {
                        due_changed |= ui
                            .add(egui::TextEdit::singleline(&mut edit.time).desired_width(55.0).hint_text("HH:MM"))
                            .changed();
                    });
                    if ui.checkbox(&mut t.all_day, "All day").changed() {
                        due_changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    let today = timeutil::start_of_day(ts);
                    for (label, days) in [("Today", 0), ("Tomorrow", 1), ("Next week", 7)] {
                        if ui.small_button(label).clicked() {
                            edit.date = timeutil::fmt_date(today + days * 86_400 + 3600);
                            if edit.time.trim().is_empty() {
                                t.all_day = true;
                            }
                            due_changed = true;
                        }
                    }
                    if ui.small_button("Clear").clicked() {
                        edit.date.clear();
                        edit.time.clear();
                        due_changed = true;
                    }
                });
                if due_changed {
                    edit.error = apply_due(t, &edit.date, &edit.time).err();
                }
                if let Some(err) = &edit.error {
                    ui.colored_label(ui.visuals().error_fg_color, err);
                } else if let Some(due) = t.due {
                    ui.weak(format!("Due {}", timeutil::fmt_due(due, t.all_day, ts)));
                }

                ui.add_space(4.0);
                ui.add_enabled_ui(t.due.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Repeat");
                        egui::ComboBox::from_id_salt("edit_repeat")
                            .selected_text(t.repeat.label())
                            .show_ui(ui, |ui| {
                                for r in RepeatUnit::ALL {
                                    ui.selectable_value(&mut t.repeat, r, r.label());
                                }
                            });
                        if t.repeat != RepeatUnit::None {
                            ui.label("every");
                            ui.add(egui::DragValue::new(&mut t.repeat_every).range(1..=365));
                            ui.label(t.repeat.unit(t.repeat_every));
                        }
                    });
                    if t.due.is_none() {
                        ui.weak("Set a due date to enable repeat.");
                    }
                });

                ui.separator();
                ui.strong("Tags");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut edit.tags)
                            .desired_width(f32::INFINITY)
                            .hint_text("work, home, errands"),
                    )
                    .changed()
                {
                    t.tags = parse_tags(&edit.tags);
                }
                if !known_tags.is_empty() {
                    ui.weak("Click a tag to add or remove it:");
                    ui.horizontal_wrapped(|ui| {
                        for tag in &known_tags {
                            let on = t.tags.contains(tag);
                            let chip = ui.selectable_label(on, format!("#{tag}"));
                            self.ui.tag_chips.push((tag.clone(), chip.rect));
                            if chip.clicked() {
                                if on {
                                    t.tags.remove(tag);
                                } else {
                                    t.tags.insert(tag.clone());
                                }
                                edit.tags = t.tags.iter().cloned().collect::<Vec<_>>().join(", ");
                            }
                        }
                    });
                }

                ui.separator();
                ui.strong("Linked task");
                ui.weak("Move this task to Urgent when another task is completed:");
                let current = t
                    .wait_for
                    .and_then(|w| candidates.iter().find(|(cid, _)| *cid == w))
                    .map_or("None".to_string(), |(_, title)| title.clone());
                egui::ComboBox::from_id_salt("edit_link")
                    .width(ui.available_width() - 8.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut t.wait_for, None, "None");
                        for (cid, title) in &candidates {
                            ui.selectable_value(&mut t.wait_for, Some(*cid), title);
                        }
                    });

                ui.separator();
                ui.weak(format!("Created {}", timeutil::fmt_datetime(t.created_at)));
                if let Some(c) = t.completed_at {
                    ui.weak(format!("Completed {}", timeutil::fmt_datetime(c)));
                }
                if let Some(from) = t.auto_from {
                    ui.weak(format!("⚡ Moved to Urgent automatically (was: {})", from.title()));
                }

                ui.add_space(12.0);
                if ui
                    .button(RichText::new("🗑 Delete task").color(ui.visuals().error_fg_color))
                    .clicked()
                {
                    delete = true;
                }
            });
        });

        if let Some(done) = set_done {
            self.set_completed(id, done);
        }
        if delete {
            self.ui.confirm_delete = Some(id);
        }
        if close {
            self.ui.selected = None;
        }
    }

    // ── Windows ─────────────────────────────────────────────────

    pub fn ui_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.ui.show_settings;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.strong("Deadlines");
                ui.checkbox(&mut self.settings.auto_urgent, "Move tasks to Urgent as their deadline approaches");
                ui.add_enabled_ui(self.settings.auto_urgent, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("When due within");
                        ui.add(egui::DragValue::new(&mut self.settings.urgent_hours).range(1..=720));
                        ui.label("hours");
                    });
                });

                ui.separator();
                ui.strong("Completed tasks");
                ui.weak("How long a ticked-off task stays in the Uncompleted view.");
                ui.horizontal(|ui| {
                    for k in KeepCompleted::ALL {
                        ui.radio_value(&mut self.settings.keep_completed, k, k.label());
                    }
                });

                ui.separator();
                ui.strong("Theme");
                ui.weak("Your choice is saved and used next time emTasks opens.");
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for t in ThemeName::ALL {
                        if theme_card(ui, t, self.theme == t).clicked() {
                            self.set_theme(t);
                        }
                    }
                });

                ui.separator();
                ui.strong("Your data");
                if let Some(storage) = &self.storage {
                    ui.weak(storage.data_path().display().to_string());
                    ui.weak(format!(
                        "Saved automatically · a backup is made each launch ({} kept)",
                        storage.list_backups().len()
                    ));
                }
                ui.horizontal(|ui| {
                    if ui.button("Show in Finder").clicked() {
                        if let Some(s) = &self.storage {
                            platform::reveal_in_finder(&s.data_path());
                        }
                    }
                    if ui.button("Back up now").clicked() {
                        self.backup_now();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Export tasks to CSV").on_hover_text("Saves all tasks to your Downloads folder").clicked() {
                        self.export_csv();
                    }
                    if ui.button("Import CSV…").on_hover_text("Adds tasks; never removes any").clicked() {
                        if let Some(p) = platform::choose_csv_file() {
                            self.import_csv_file(&p);
                        }
                    }
                });
                ui.weak("CSV export includes every task, whatever filter is on. It opens in Numbers or Excel.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.settings.documents_backup, "Daily CSV backup to Documents/emTasks");
                    if ui.button("Show").clicked() {
                        if let Some(dir) = &self.documents_backup_dir {
                            platform::reveal_in_finder(dir);
                        }
                    }
                });
                ui.weak("One file per day, newest 30 kept. If your Documents folder syncs with iCloud, so do these.");

            });
        self.ui.show_settings = open;
    }

    pub fn ui_save_preset_window(&mut self, ctx: &egui::Context) {
        if !self.ui.show_save_preset {
            return;
        }
        let mut open = true;
        let mut save = false;
        egui::Window::new("Save filter preset")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                let resp = ui.add(egui::TextEdit::singleline(&mut self.ui.preset_name).hint_text("Preset name"));
                resp.request_focus();
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                ui.weak(format!(
                    "Saves: sort by {}, {}, {}",
                    self.filter.sort.label(),
                    self.filter.status.label().to_lowercase(),
                    if self.filter.tags.is_empty() { "all tags".to_string() } else {
                        self.filter.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
                    }
                ));
                if ui.button("Save").clicked() {
                    save = true;
                }
            });
        let name = self.ui.preset_name.trim().to_string();
        if save && !name.is_empty() {
            self.presets.retain(|p| p.name != name);
            self.presets.push(Preset { name: name.clone(), filter: self.filter.clone() });
            self.ui.status = format!("Saved preset \"{name}\"");
            open = false;
        }
        self.ui.show_save_preset = open;
    }

    pub fn ui_shortcuts_window(&mut self, ctx: &egui::Context) {
        let mut open = self.ui.show_shortcuts;
        egui::Window::new("Keyboard shortcuts")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                    for (keys, what) in [
                        ("⌘N", "New task (into the maximized quadrant, if any)"),
                        ("⬆ / ⬇", "Select previous / next task"),
                        ("⌘↩", "Complete / reopen selected task"),
                        ("Delete or ⌘Delete", "Delete selected task (instant in the Inbox)"),
                        ("⌘1 … ⌘4", "Move to Do / Schedule / Delegate / Eliminate"),
                        ("⌘0", "Move to the Inbox"),
                        ("⌘Z / Shift⌘Z", "Undo / redo"),
                        ("⌘L", "Switch Matrix / List view"),
                        ("⌘I", "Show / hide the Inbox"),
                        ("⌘,", "Settings"),
                        ("Esc", "Close ➕ add box / restore maximized section / close editor"),
                    ] {
                        ui.label(RichText::new(keys).monospace().strong());
                        ui.label(what);
                        ui.end_row();
                    }
                });
            });
        self.ui.show_shortcuts = open;
    }

    pub fn ui_import_report(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.ui.import_report else { return };
        let mut close = false;
        egui::Window::new("Import finished")
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.strong(&r.file);
                ui.label(format!("Added {} task{}.", r.added, if r.added == 1 { "" } else { "s" }));
                if r.skipped > 0 {
                    ui.label(format!("Skipped {} already in emTasks.", r.skipped));
                }
                if !r.warnings.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(ui.visuals().warn_fg_color, "Some rows needed attention:");
                    ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for w in &r.warnings {
                            ui.weak(w);
                        }
                    });
                }
                ui.add_space(4.0);
                ui.weak("Nothing was removed. ⌘Z undoes the import.");
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        if close {
            self.ui.import_report = None;
        }
    }

    pub fn ui_confirm_delete(&mut self, ctx: &egui::Context) {
        let Some(id) = self.ui.confirm_delete else { return };
        let Some(title) = self.task(id).map(|t| t.title.clone()) else {
            self.ui.confirm_delete = None;
            return;
        };
        let mut decision = None;
        egui::Window::new("Delete task?")
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!("\"{title}\" will be deleted."));
                ui.weak("You can undo this with ⌘Z.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let del = ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color));
                    del.request_focus();
                    if del.clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            decision = Some(true);
        }
        match decision {
            Some(true) => {
                self.delete_task(id);
                self.ui.confirm_delete = None;
            }
            Some(false) => self.ui.confirm_delete = None,
            None => {}
        }
    }

    /// Shown instead of the app when the data file can't be used.
    pub fn ui_recovery(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.set_max_width(560.0);
                match &self.ui.recovery {
                    Some(Recovery::TooNew(v)) => {
                        ui.heading("This data is from a newer emTasks");
                        ui.add_space(8.0);
                        ui.label(format!(
                            "Your tasks were saved by a newer version of emTasks (data format {v}). \
                             Please install the latest version. Nothing has been changed."
                        ));
                        ui.add_space(12.0);
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Some(Recovery::AlreadyRunning) => {
                        ui.heading("emTasks is already open");
                        ui.add_space(8.0);
                        ui.label(
                            "Another emTasks window is using your tasks. Switch to it with ⌘Tab. \
                             Two copies saving at once could lose changes, so this one won't open them.",
                        );
                        ui.add_space(12.0);
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Some(Recovery::Corrupt(err)) => {
                        let err = err.clone();
                        ui.heading("emTasks couldn't open your tasks");
                        ui.add_space(8.0);
                        ui.label(err);
                        ui.label("Your data file has not been changed or deleted.");
                        ui.add_space(12.0);
                        let backups = self.storage.as_ref().map(|s| s.list_backups()).unwrap_or_default();
                        if backups.is_empty() {
                            ui.weak("No backups found.");
                        } else {
                            ui.strong("Restore from a backup:");
                            for b in backups.iter().take(8) {
                                let name = b.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.monospace(&name);
                                    if ui.button("Restore").clicked() {
                                        self.restore_backup(b);
                                    }
                                });
                            }
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button("Start with an empty list").on_hover_text("The damaged file is kept next to your data").clicked() {
                                if let Some(s) = &self.storage {
                                    match s.set_aside_damaged() {
                                        Ok(_) => {
                                            self.ui.recovery = None;
                                            self.ui.status = "Started fresh — the damaged file was kept".into();
                                        }
                                        Err(e) => self.ui.status = format!("Couldn't move the damaged file: {e}"),
                                    }
                                }
                            }
                            if ui.button("Show in Finder").clicked() {
                                if let Some(s) = &self.storage {
                                    platform::reveal_in_finder(&s.data_path());
                                }
                            }
                            if ui.button("Quit").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                        if !self.ui.status.is_empty() {
                            ui.colored_label(ui.visuals().warn_fg_color, &self.ui.status);
                        }
                    }
                    None => {}
                }
            });
        });
    }

    fn restore_backup(&mut self, backup: &std::path::Path) {
        let Some(storage) = &self.storage else { return };
        let result = storage.restore(backup).map(|_| storage.load());
        match result {
            Ok(crate::storage::LoadResult::Loaded(d)) => {
                self.reload_from(d);
                self.ui.status = "Restored from backup".into();
            }
            Ok(_) => self.ui.status = "That backup couldn't be loaded".into(),
            Err(e) => self.ui.status = format!("Restore failed: {e}"),
        }
    }
}

/// A clickable preview of a theme for the Settings window.
fn theme_card(ui: &mut Ui, t: ThemeName, selected: bool) -> egui::Response {
    let p = theme::palette(t);
    let size = egui::vec2(112.0, 92.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect.expand(2.0));
    let preview = Rect::from_min_size(rect.min, egui::vec2(size.x, 62.0));

    painter.rect_filled(preview, 8.0, p.bg);
    // A mini matrix: four quadrant cards with their colour strips.
    let inner = preview.shrink(8.0);
    let (cw, ch) = ((inner.width() - 4.0) / 2.0, (inner.height() - 4.0) / 2.0);
    for (i, s) in [Section::Do, Section::Schedule, Section::Delegate, Section::Eliminate].into_iter().enumerate() {
        let (col, row) = ((i % 2) as f32, (i / 2) as f32);
        let r = Rect::from_min_size(
            egui::pos2(inner.min.x + col * (cw + 4.0), inner.min.y + row * (ch + 4.0)),
            egui::vec2(cw, ch),
        );
        painter.rect_filled(r, 3.0, p.card);
        painter.rect_filled(Rect::from_min_size(r.min, egui::vec2(cw, 3.0)), 1.0, theme::section_color(t, s));
    }
    let stroke = if selected {
        Stroke::new(2.0_f32, ui.visuals().selection.stroke.color)
    } else if resp.hovered() {
        Stroke::new(1.0_f32, ui.visuals().widgets.hovered.bg_stroke.color)
    } else {
        Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color)
    };
    painter.rect_stroke(preview, 8.0, stroke);

    let label = if selected { format!("✔ {}", t.label()) } else { t.label().to_string() };
    painter.text(
        egui::pos2(rect.min.x + 2.0, preview.max.y + 6.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
    painter.text(
        egui::pos2(rect.min.x + 2.0, preview.max.y + 22.0),
        egui::Align2::LEFT_TOP,
        if p.dark { "Dark" } else { "Light" },
        egui::FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
    resp.on_hover_text(t.description())
}

/// True once a press on `resp` has turned into an actual drag (the mouse moved
/// past egui's click threshold) and no task is being dragged yet.
fn is_real_drag(ui: &Ui, resp: &egui::Response) -> bool {
    resp.dragged()
        && ui.input(|i| i.pointer.is_decidedly_dragging())
        && !egui::DragAndDrop::has_payload_of_type::<TaskId>(ui.ctx())
}

/// If the text field `id` has focus, pull out any multi-line paste events
/// (so the single-line field doesn't flatten them) and return their text.
fn take_multiline_paste(ui: &Ui, id: egui::Id) -> Vec<String> {
    if !ui.ctx().memory(|m| m.has_focus(id)) {
        return Vec::new();
    }
    let mut pasted = Vec::new();
    ui.ctx().input_mut(|i| {
        i.events.retain(|e| match e {
            egui::Event::Paste(s) if s.contains('\n') => {
                pasted.push(s.clone());
                false
            }
            _ => true,
        })
    });
    pasted
}
