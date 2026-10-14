//! Themes: Classic (egui's stock dark look) plus two light and two dark palettes.
//!
//! The chosen theme is saved with the user's data and re-applied on launch.

use std::sync::OnceLock;

use eframe::egui::{
    self, vec2, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Shadow, Stroke,
    TextStyle, Theme, Visuals,
};
use serde::{Deserialize, Serialize};

use crate::model::Section;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ThemeName {
    /// The original look, kept unchanged.
    #[default]
    Classic,
    Daylight,
    Paper,
    Midnight,
    Graphite,
}

impl ThemeName {
    pub const ALL: [ThemeName; 5] =
        [ThemeName::Classic, ThemeName::Daylight, ThemeName::Paper, ThemeName::Midnight, ThemeName::Graphite];

    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Classic => "Classic",
            ThemeName::Daylight => "Daylight",
            ThemeName::Paper => "Paper",
            ThemeName::Midnight => "Midnight",
            ThemeName::Graphite => "Graphite",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ThemeName::Classic => "The original look",
            ThemeName::Daylight => "Light · crisp white, blue",
            ThemeName::Paper => "Light · warm cream, orange",
            ThemeName::Midnight => "Dark · deep navy, indigo",
            ThemeName::Graphite => "Dark · charcoal, teal",
        }
    }

    pub fn is_classic(self) -> bool {
        self == ThemeName::Classic
    }
}

/// Colors for one theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub border: Color32,
    pub text: Color32,
    pub accent: Color32,
    pub hover: Color32,
    pub input: Color32,
    pub button: Color32,
    pub button_hover: Color32,
    pub danger: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn palette(t: ThemeName) -> Palette {
    match t {
        // Approximates egui's stock dark visuals; Classic uses those directly.
        ThemeName::Classic => Palette {
            dark: true,
            bg: rgb(0x1B1B1B),
            panel: rgb(0x1B1B1B),
            card: rgb(0x1B1B1B),
            border: rgb(0x3C3C3C),
            text: rgb(0xB4B4B4),
            accent: rgb(0x5A9BE6),
            hover: rgb(0x2A2A2A),
            input: rgb(0x0A0A0A),
            button: rgb(0x3C3C3C),
            button_hover: rgb(0x464646),
            danger: rgb(0xFF0000),
        },
        ThemeName::Daylight => Palette {
            dark: false,
            bg: rgb(0xF3F5F8),
            panel: rgb(0xFFFFFF),
            card: rgb(0xFFFFFF),
            border: rgb(0xE2E6EC),
            text: rgb(0x1E2329),
            accent: rgb(0x3B82F6),
            hover: rgb(0xF1F5FB),
            input: rgb(0xF7F9FC),
            button: rgb(0xEEF1F5),
            button_hover: rgb(0xE3E8EF),
            danger: rgb(0xD14343),
        },
        ThemeName::Paper => Palette {
            dark: false,
            bg: rgb(0xF6F1E8),
            panel: rgb(0xFBF7F0),
            card: rgb(0xFFFDF8),
            border: rgb(0xE6DCCB),
            text: rgb(0x2A241C),
            accent: rgb(0xC2410C),
            hover: rgb(0xF7F0E4),
            input: rgb(0xFBF7EF),
            button: rgb(0xF0E8DA),
            button_hover: rgb(0xE8DDC9),
            danger: rgb(0xC23B3B),
        },
        ThemeName::Midnight => Palette {
            dark: true,
            bg: rgb(0x0F1220),
            panel: rgb(0x151A2B),
            card: rgb(0x1B2136),
            border: rgb(0x2A3150),
            text: rgb(0xE6E9F5),
            accent: rgb(0x7C8CFF),
            hover: rgb(0x232A44),
            input: rgb(0x12162A),
            button: rgb(0x242B45),
            button_hover: rgb(0x2E3657),
            danger: rgb(0xFF7A7A),
        },
        ThemeName::Graphite => Palette {
            dark: true,
            bg: rgb(0x17181A),
            panel: rgb(0x1D1F22),
            card: rgb(0x232528),
            border: rgb(0x33363A),
            text: rgb(0xE8E9EB),
            accent: rgb(0x2DD4BF),
            hover: rgb(0x2A2D31),
            input: rgb(0x1A1C1F),
            button: rgb(0x2C2F33),
            button_hover: rgb(0x363A3F),
            danger: rgb(0xFF7B72),
        },
    }
}

/// Quadrant colors, tuned for contrast on light or dark backgrounds.
pub fn section_color(t: ThemeName, s: Section) -> Color32 {
    if t.is_classic() {
        return s.color();
    }
    let dark = palette(t).dark;
    match (s, dark) {
        (Section::Do, false) => rgb(0xD64545),
        (Section::Schedule, false) => rgb(0x2F6FDB),
        (Section::Delegate, false) => rgb(0xD97A1E),
        (Section::Eliminate, false) => rgb(0x3F9564),
        (Section::Unsorted, false) => rgb(0x7C828C),
        (Section::Do, true) => rgb(0xF27474),
        (Section::Schedule, true) => rgb(0x6E9EF7),
        (Section::Delegate, true) => rgb(0xF0A35A),
        (Section::Eliminate, true) => rgb(0x6FC293),
        (Section::Unsorted, true) => rgb(0x9AA1AB),
    }
}

/// Install the theme's colors, spacing and fonts.
pub fn apply(ctx: &egui::Context, t: ThemeName) {
    let p = palette(t);
    let kind = if p.dark { Theme::Dark } else { Theme::Light };

    let mut style = if t.is_classic() {
        egui::Style { visuals: Visuals::dark(), ..Default::default() }
    } else {
        styled(p)
    };
    style.text_styles = text_styles();
    // Labels aren't selectable text: pressing on a task's title must start a drag.
    style.interaction.selectable_labels = false;
    ctx.set_style_of(kind, style);
    // Pin the mode so macOS light/dark switching doesn't override the user's choice.
    ctx.set_theme(kind);
    install_fonts(ctx, !t.is_classic());
}

fn styled(p: Palette) -> egui::Style {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.faint_bg_color = p.hover;
    v.extreme_bg_color = p.input;
    v.code_bg_color = p.button;
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_rounding = Rounding::same(10.0);
    v.menu_rounding = Rounding::same(8.0);
    let shadow_alpha = if p.dark { 110 } else { 30 };
    v.window_shadow = Shadow { offset: vec2(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(shadow_alpha) };
    v.popup_shadow = Shadow { offset: vec2(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(shadow_alpha) };
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.danger;
    v.warn_fg_color = if p.dark { rgb(0xFBBF24) } else { rgb(0xB45309) };
    v.selection.bg_fill = p.accent.gamma_multiply(if p.dark { 0.45 } else { 0.22 });
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);

    let round = Rounding::same(6.0);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.weak_bg_fill = p.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.noninteractive.rounding = round;

    w.inactive.bg_fill = p.button;
    w.inactive.weak_bg_fill = p.button;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.inactive.rounding = round;

    w.hovered.bg_fill = p.button_hover;
    w.hovered.weak_bg_fill = p.button_hover;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent.gamma_multiply(0.7));
    w.hovered.fg_stroke = Stroke::new(1.5_f32, p.text);
    w.hovered.rounding = round;

    w.active.bg_fill = p.accent.gamma_multiply(0.35);
    w.active.weak_bg_fill = p.accent.gamma_multiply(0.35);
    w.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    w.active.fg_stroke = Stroke::new(2.0_f32, p.text);
    w.active.rounding = round;

    w.open = w.hovered;

    let mut style = egui::Style { visuals: v, ..Default::default() };
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.button_padding = vec2(10.0, 4.0);
    style.spacing.interact_size.y = 22.0;
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style
}

/// Text sizes for every theme (egui's default body text is 12.5, which reads small).
pub const BODY_SIZE: f32 = 14.0;

fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    [
        (TextStyle::Heading, FontId::proportional(20.0)),
        (TextStyle::Body, FontId::proportional(BODY_SIZE)),
        (TextStyle::Button, FontId::proportional(BODY_SIZE)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into()
}

/// The Mac's own UI font, if it can be read and parsed. Never bundled.
fn system_font() -> Option<&'static [u8]> {
    static FONT: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    FONT.get_or_init(|| {
        let bytes = std::fs::read("/System/Library/Fonts/SFNS.ttf").ok()?;
        // egui panics on fonts it can't parse, so check first.
        ab_glyph::FontRef::try_from_slice(&bytes).ok()?;
        Some(bytes)
    })
    .as_deref()
}

/// Classic keeps egui's bundled font; the new themes use the system font.
/// egui's fonts stay as fallbacks so every icon still renders.
fn install_fonts(ctx: &egui::Context, use_system: bool) {
    let key = egui::Id::new("emtasks_system_font");
    if ctx.data(|d| d.get_temp::<bool>(key)) == Some(use_system) {
        return;
    }
    let mut defs = FontDefinitions::default();
    if use_system {
        if let Some(bytes) = system_font() {
            defs.font_data.insert("system".into(), FontData::from_owned(bytes.to_vec()));
            if let Some(family) = defs.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "system".into());
            }
        }
    }
    ctx.set_fonts(defs);
    ctx.data_mut(|d| d.insert_temp(key, use_system));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance / contrast ratio.
    fn luminance(c: Color32) -> f32 {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn two_light_and_two_dark_themes_plus_classic() {
        let new: Vec<_> = ThemeName::ALL.iter().filter(|t| !t.is_classic()).collect();
        assert_eq!(new.iter().filter(|t| palette(***t).dark).count(), 2);
        assert_eq!(new.iter().filter(|t| !palette(***t).dark).count(), 2);
        assert_eq!(ThemeName::default(), ThemeName::Classic, "existing users keep the current look");
    }

    #[test]
    fn text_is_readable_in_every_new_theme() {
        for t in ThemeName::ALL.into_iter().filter(|t| !t.is_classic()) {
            let p = palette(t);
            for (what, bg) in [("card", p.card), ("background", p.bg), ("panel", p.panel)] {
                let c = contrast(p.text, bg);
                assert!(c >= 7.0, "{t:?}: body text on {what} only {c:.1}:1");
            }
            let danger = contrast(p.danger, p.card);
            assert!(danger >= 3.0, "{t:?}: overdue red on cards only {danger:.1}:1");
            for s in Section::ALL {
                let c = contrast(section_color(t, s), p.card);
                assert!(c >= 3.0, "{t:?}: {s:?} heading on card only {c:.1}:1");
            }
        }
    }

    #[test]
    fn every_theme_renders() {
        for t in ThemeName::ALL {
            let ctx = egui::Context::default();
            apply(&ctx, t);
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| ui.label("Render check ⏺ ➕ 🗑"));
            });
            assert_eq!(ctx.theme() == Theme::Dark, palette(t).dark, "{t:?} light/dark mode");
            let body = ctx.style().text_styles[&TextStyle::Body].size;
            assert_eq!(body, 14.0, "{t:?} body text size");
            if !t.is_classic() {
                assert_eq!(ctx.style().visuals.panel_fill, palette(t).bg, "{t:?} colours installed");
            }
        }
    }
}

