//! The live step grid: one row per voice, one column per step, and a playhead.
//!
//! The rows come from `loom_app::grid` (what each voice plays this bar); this
//! module only draws them. Clicking a voice name asks to mute/unmute it.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind};
use loom_app::{Cell, Row, Theme};

use crate::style::{color, mix};

/// Steps drawn per bar.
pub const COLUMNS: usize = 16;
/// Width of the voice-name column, in points.
const NAME_WIDTH: f32 = 96.0;
/// Height of one voice row, in points.
const ROW_HEIGHT: f32 = 22.0;

/// Which column the playhead is in, for a position `phase` (0..1) in the bar.
pub fn playhead_column(phase: f32, columns: usize) -> usize {
    let columns = columns.max(1);
    ((phase.clamp(0.0, 1.0) * columns as f32) as usize).min(columns - 1)
}

/// Height the grid needs for `rows` voices.
pub fn height_for(rows: usize) -> f32 {
    rows.max(1) as f32 * ROW_HEIGHT
}

/// Draw the grid. `playhead` is the bar phase while playing (None when stopped).
/// Returns the name of the voice whose label was clicked, if any.
pub fn show(ui: &mut egui::Ui, rows: &[Row], playhead: Option<f32>, muted: &[String], theme: &Theme) -> Option<String> {
    if rows.is_empty() {
        ui.label(egui::RichText::new("Aucune voix pour l'instant — écris par exemple  kick: x . x .  puis Ctrl+Entrée.").color(color(theme.dim)));
        return None;
    }

    let width = ui.available_width().max(NAME_WIDTH + 16.0 * 8.0);
    let (area, _) = ui.allocate_exact_size(egui::vec2(width, height_for(rows.len())), Sense::hover());
    let painter = ui.painter_at(area);
    let columns = rows[0].cells.len().max(1);
    let cell_width = (width - NAME_WIDTH) / columns as f32;
    let mut clicked = None;

    // soft background every 4 steps, so beats are easy to count
    for col in (0..columns).step_by(4) {
        let x = area.left() + NAME_WIDTH + col as f32 * cell_width;
        let beat = Rect::from_min_size(egui::pos2(x, area.top()), egui::vec2(cell_width * 4.0, area.height()));
        let fill = if col % 8 == 0 { mix(theme.panel, theme.foreground, 0.05) } else { color(theme.panel) };
        painter.rect_filled(beat, CornerRadius::ZERO, fill);
    }

    for (i, row) in rows.iter().enumerate() {
        let top = area.top() + i as f32 * ROW_HEIGHT;
        let is_muted = muted.contains(&row.name);

        // the voice name: a click target for mute
        let name_rect = Rect::from_min_size(egui::pos2(area.left(), top), egui::vec2(NAME_WIDTH, ROW_HEIGHT));
        let response = ui
            .interact(name_rect, ui.id().with(("mute", &row.name)), Sense::click())
            .on_hover_text(if is_muted { "Clic : rétablir cette voix" } else { "Clic : couper cette voix (mute)" });
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            painter.rect_filled(name_rect.shrink(1.0), CornerRadius::same(3), color(theme.selection));
        }
        if response.clicked() {
            clicked = Some(row.name.clone());
        }
        let name_color = if is_muted { color(theme.dim) } else if row.is_drum { color(theme.drum) } else { color(theme.note) };
        let text_pos = egui::pos2(name_rect.left() + 6.0, name_rect.center().y);
        let galley_rect = painter.text(text_pos, Align2::LEFT_CENTER, &row.name, FontId::monospace(13.0), name_color);
        if is_muted {
            painter.hline(galley_rect.x_range(), galley_rect.center().y, Stroke::new(1.0, name_color));
        }

        // the steps
        let hit_color = if is_muted { color(theme.dim) } else if row.is_drum { color(theme.drum) } else { color(theme.note) };
        for (col, cell) in row.cells.iter().enumerate() {
            let x = area.left() + NAME_WIDTH + col as f32 * cell_width;
            let rect = Rect::from_min_size(egui::pos2(x, top), egui::vec2(cell_width, ROW_HEIGHT)).shrink2(egui::vec2(1.5, 3.0));
            match cell {
                Cell::Rest => {
                    painter.rect_filled(rect, CornerRadius::same(2), mix(theme.panel, theme.foreground, 0.12));
                }
                Cell::Hit(velocity) => {
                    let strength = velocity.clamp(0.45, 1.0);
                    painter.rect_filled(rect, CornerRadius::same(2), hit_color.gamma_multiply(strength));
                    if *velocity > 1.0 {
                        // accent: a bright outline
                        painter.rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.5, color(theme.foreground)), StrokeKind::Inside);
                    }
                    if let Some(note) = row.notes.get(col).and_then(|n| n.as_deref()) {
                        if cell_width >= 30.0 {
                            painter.text(rect.center(), Align2::CENTER_CENTER, note, FontId::monospace(10.0), color(theme.background));
                        }
                    }
                }
            }
        }
    }

    // the playhead: the current column is outlined, a thin line shows the exact position
    if let Some(phase) = playhead {
        let col = playhead_column(phase, columns);
        let x = area.left() + NAME_WIDTH + col as f32 * cell_width;
        let column = Rect::from_min_size(egui::pos2(x, area.top()), egui::vec2(cell_width, area.height()));
        painter.rect_filled(column, CornerRadius::same(2), with_alpha(color(theme.accent), 40));
        painter.rect_stroke(column, CornerRadius::same(2), Stroke::new(1.0, color(theme.accent)), StrokeKind::Inside);
        let exact = area.left() + NAME_WIDTH + phase.clamp(0.0, 1.0) * (width - NAME_WIDTH);
        painter.vline(exact, area.y_range(), Stroke::new(1.0, color(theme.foreground)));
    }

    clicked
}

/// The same colour, see-through (`alpha` 0..255).
fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playhead_stays_inside_the_grid() {
        assert_eq!(playhead_column(0.0, 16), 0);
        assert_eq!(playhead_column(0.49, 16), 7);
        assert_eq!(playhead_column(0.5, 16), 8);
        assert_eq!(playhead_column(1.0, 16), 15);
        assert_eq!(playhead_column(-3.0, 16), 0);
        assert_eq!(playhead_column(0.7, 0), 0);
    }
}
