//! ROLL panel: the current bar's melodies as coloured blocks, with the playhead.

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke};
use loom_app::Theme;

use super::StudioAction;
use crate::scene::logic::hsl_to_rgb;
use crate::studio::roll::Roll;
use crate::style::{color, mix};

/// Height of the roll in a tall window, in points (the browser's 140 px).
const HEIGHT: f32 = 140.0;

/// Draw the roll for `bar`. `phase` is the playhead while playing.
pub fn show(ui: &mut egui::Ui, roll: &Roll, bar: i64, phase: Option<f32>, theme: &Theme, actions: &mut Vec<StudioAction>) {
    egui::Panel::bottom("roll")
        .resizable(false)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
        .show(ui, |ui| {
            let width = ui.available_width(); // before the header, which may be wider than the window
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Piano-roll").strong());
                ui.label(RichText::new(format!("mesure {} · clic sur un bloc = aller à la ligne de sa voix", bar + 1)).color(color(theme.dim)));
            });
            // a smaller roll in a short window, so the editor keeps some room
            let height = (ui.ctx().content_rect().height() * 0.18).clamp(60.0, HEIGHT);
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, CornerRadius::same(3), color(theme.background));
            if roll.notes.is_empty() {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "piano-roll — écris une mélodie (notes, degrés ou accords)", FontId::monospace(12.0), color(theme.dim));
                return;
            }
            let rows = roll.rows() as f32;
            let row_h = rect.height() / rows;
            // a faint line on every C, so octaves are easy to see
            for midi in roll.low..=roll.high {
                if midi.rem_euclid(12) == 0 {
                    let y = rect.bottom() - (midi - roll.low + 1) as f32 * row_h;
                    painter.hline(rect.x_range(), y + row_h, Stroke::new(1.0, mix(theme.background, theme.dim, 0.35)));
                }
            }
            for note in &roll.notes {
                let x = rect.left() + note.start * rect.width();
                let w = (note.len * rect.width() - 1.0).max(2.0);
                let y = rect.bottom() - (note.midi - roll.low + 1) as f32 * row_h;
                let block = egui::Rect::from_min_size(egui::pos2(x + 0.5, y + 1.0), egui::vec2(w, (row_h - 2.0).max(2.0)));
                let (r, g, b) = hsl_to_rgb(note.hue, 0.8, 0.6);
                painter.rect_filled(block, CornerRadius::same(2), Color32::from_rgb(r, g, b));
            }
            if let Some(phase) = phase {
                let x = rect.left() + phase.clamp(0.0, 1.0) * rect.width();
                painter.vline(x, rect.y_range(), Stroke::new(2.0, Color32::from_white_alpha(115)));
            }
            let hovered = response.hover_pos().and_then(|p| {
                roll.note_at((p.x - rect.left()) / rect.width(), (p.y - rect.top()) / rect.height())
            });
            if let Some(note) = hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                let voice = note.voice.clone();
                if response.on_hover_text(format!("{voice} · note MIDI {}", note.midi)).clicked() {
                    actions.push(StudioAction::GoToVoice(voice));
                }
            }
        });
}
