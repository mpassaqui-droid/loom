//! Theme → egui look.
//!
//! `loom_app::Theme` holds the Omarchy colours (or LOOM's green default); this
//! module paints egui's widgets with them, and picks the text sizes.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use loom_app::{Rgb, Theme};

use crate::editor::Palette;

/// Size of the code in the editor, in points.
pub const CODE_SIZE: f32 = 15.0;

/// When the large-text accessibility mode is on, text grows by this factor
/// (everything is scaled from the normal sizes, so the layout stays coherent).
const LARGE_SCALE: f32 = 1.25;

/// An Omarchy colour as an egui colour.
pub fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A blend of two theme colours (0 = `a`, 1 = `b`).
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Color32 {
    color(a.mix(b, t))
}

/// egui `Visuals` built from a LOOM theme.
pub fn visuals(theme: &Theme) -> Visuals {
    let mut v = if theme.dark { Visuals::dark() } else { Visuals::light() };
    let bg = theme.background;
    let fg = color(theme.foreground);
    let accent = color(theme.accent);

    v.dark_mode = theme.dark;
    v.panel_fill = color(theme.panel);
    v.window_fill = color(theme.panel);
    v.window_stroke = Stroke::new(1.0, mix(theme.panel, theme.foreground, 0.2));
    v.extreme_bg_color = color(bg); // text edit and slider rail backgrounds
    v.text_edit_bg_color = Some(color(bg));
    v.faint_bg_color = mix(theme.panel, theme.foreground, 0.04);
    v.code_bg_color = color(bg);
    v.hyperlink_color = accent;
    v.error_fg_color = color(theme.error);
    v.warn_fg_color = color(theme.error);
    v.weak_text_color = Some(color(theme.dim));
    v.override_text_color = None;

    v.selection.bg_fill = color(theme.selection);
    v.selection.stroke = Stroke::new(1.0, fg);
    v.text_cursor.stroke = Stroke::new(2.0, accent);
    v.slider_trailing_fill = true;

    // widgets: calm at rest, the accent shows up when you touch them
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = color(theme.panel);
    w.noninteractive.weak_bg_fill = color(theme.panel);
    w.noninteractive.bg_stroke = Stroke::new(1.0, mix(theme.panel, theme.foreground, 0.15));
    w.noninteractive.fg_stroke = Stroke::new(1.0, fg);

    w.inactive.bg_fill = mix(theme.panel, theme.foreground, 0.12);
    w.inactive.weak_bg_fill = mix(theme.panel, theme.foreground, 0.08);
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, fg);

    w.hovered.bg_fill = mix(theme.panel, theme.accent, 0.35);
    w.hovered.weak_bg_fill = mix(theme.panel, theme.accent, 0.2);
    w.hovered.bg_stroke = Stroke::new(1.0, accent);
    w.hovered.fg_stroke = Stroke::new(1.5, fg);

    w.active.bg_fill = accent;
    w.active.weak_bg_fill = mix(theme.panel, theme.accent, 0.35);
    w.active.bg_stroke = Stroke::new(1.0, accent);
    w.active.fg_stroke = Stroke::new(2.0, fg);

    w.open = w.hovered;
    v
}

/// Apply the theme to the whole app (both egui's dark and light slots, so the
/// OS preference can't override Omarchy's choice).
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let egui_theme = if theme.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(egui_theme);
    ctx.set_visuals_of(egui_theme, visuals(theme));
}

/// Slightly larger text than egui's defaults: easier to read while playing.
/// `large` turns on the accessibility mode (bigger fonts everywhere).
pub fn apply_text_sizes(ctx: &egui::Context, large: bool) {
    let s = if large { LARGE_SCALE } else { 1.0 };
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0 * s, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0 * s, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0 * s, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0 * s, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(CODE_SIZE * s, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0 * s, 5.0 * s);
        style.spacing.button_padding = egui::vec2(8.0 * s, 4.0 * s);
    });
}

/// Editor colours for a theme. `large` also grows the code font.
pub fn palette(theme: &Theme, large: bool) -> Palette {
    let font_size = if large { CODE_SIZE * LARGE_SCALE } else { CODE_SIZE };
    Palette {
        plain: color(theme.foreground),
        comment: color(theme.dim),
        name: color(theme.accent),
        verb: color(theme.note),
        hit: color(theme.drum),
        font_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Does `font` really draw `c`, or the "missing glyph" box instead?
    /// (egui's own `has_glyph` can't tell when the box lives in the same font.)
    fn draws(ctx: &egui::Context, font: &FontId, c: char) -> bool {
        const MISSING: char = '◻'; // what egui draws for an unknown character
        let uv = |ch: char| {
            ctx.fonts_mut(|f| {
                let galley = f.layout_no_wrap(ch.to_string(), font.clone(), Color32::WHITE);
                galley.rows[0].row.glyphs[0].uv_rect
            })
        };
        let (glyph, missing) = (uv(c), uv(MISSING));
        (glyph.min, glyph.max) != (missing.min, missing.max)
    }

    /// egui only ships a few fonts: a symbol missing from them shows as a box.
    #[test]
    fn ui_symbols_exist_in_the_bundled_fonts() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        // `●` isn't in them, so the "modified" mark is `⏺`
        for c in "▶■⏺⟳·«»éÉè…—".chars() {
            assert!(draws(&ctx, &FontId::proportional(14.0), c), "no glyph for {c}");
        }
        // the code font must handle French comments too
        for c in "#éèàçÉ—«»".chars() {
            assert!(draws(&ctx, &FontId::monospace(CODE_SIZE), c), "no code glyph for {c}");
        }
        output.textures_delta.clear(); // no GPU here: nothing to upload
    }
}
