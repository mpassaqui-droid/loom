//! PARTITION panel: the current bar read as sheet music — a grand staff
//! (clé de sol + clé de fa) with one note-head per event, coloured like the
//! roll. Click a note to jump to its voice and arm it; the ◀ ▶ buttons then
//! transpose that voice by one semitone at a time (undoable, the `| semi`
//! verb is rewritten in the editor).
//!
//! The music theory lives in [`loom_app::studio::notation`]; this file only
//! draws it and turns clicks into [`StudioAction`]s.

use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke};
use loom_app::studio::notation::{midi_name, StaffNote};
use loom_app::Theme;
use crate::scene::logic::hsl_to_rgb;
use crate::style::{color, mix};
use super::StudioAction;

/// One ledger space, in points (line-to-line is twice this).
const STEP: f32 = 5.5;
/// Whole panel height, in points.
const HEIGHT: f32 = 210.0;

/// Draw the grand staff for `bar`. `armed` is the voice the ◀ ▶ buttons will
/// transpose; `semi` its current transposition (so the label can show it).
pub fn show(ui: &mut egui::Ui, notes: &[StaffNote], bar: i64, armed: Option<&str>, semi: i32, theme: &Theme, actions: &mut Vec<StudioAction>) {
    egui::Panel::bottom("notation")
        .resizable(false)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
        .show(ui, |ui| {
            let width = ui.available_width();
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Partition").strong());
                ui.label(RichText::new(format!("mesure {} · clic une note = aller à sa voix · ◀ ▶ transpose", bar + 1)).color(color(theme.dim)));
                if let Some(voice) = armed {
                    ui.separator();
                    ui.label(RichText::new(format!("voix {voice} · {semi:+} demi-tons")).color(color(theme.accent)));
                    let up = ui.button("▶").on_hover_text(format!("Transposer « {voice} » d'un demi-ton plus haut"));
                    let down = ui.button("◀").on_hover_text(format!("Transposer « {voice} » d'un demi-ton plus bas"));
                    let reset = ui.button("réinit").on_hover_text(format!("Remettre « {voice} » à sa hauteur d'origine"));
                    if up.clicked() {
                        actions.push(StudioAction::NotationTranspose(1));
                    }
                    if down.clicked() {
                        actions.push(StudioAction::NotationTranspose(-1));
                    }
                    if reset.clicked() && semi != 0 {
                        actions.push(StudioAction::NotationTranspose(-semi));
                    }
                }
            });
            let height = (ui.ctx().content_rect().height() * 0.22).clamp(150.0, HEIGHT);
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, color(theme.background));
            if notes.is_empty() {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "partition — écris des notes (noms, degrés ou accords) pour voir une portée", FontId::monospace(12.0), color(theme.dim));
                return;
            }

            // two staves: treble on top, bass below; both put degree 0 (bottom line)
            // near the bottom of their band and degree 8 (top line) near the top.
            let band_h = (height - 16.0) / 2.0;
            let treble = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.min.y + band_h));
            let bass = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.min.y + band_h), rect.max);
            // the notes as pixels, for the click test afterwards
            let mut placed: Vec<(f32, f32, &StaffNote)> = Vec::new();

            for (band, top_ref, clef) in [(treble, 14.0, "clé de sol"), (bass, 14.0, "clé de fa")] {
                painter.text(egui::pos2(band.left() + 30.0, band.top() + 1.0), egui::Align2::LEFT_TOP, clef, FontId::monospace(11.0), color(theme.dim));
                // the five lines sit on even degrees 0..=8
                for d in [0i32, 2, 4, 6, 8] {
                    let y = band.top() + top_ref + (8.0 - d as f32) * STEP;
                    painter.hline(band.x_range(), y, Stroke::new(1.0, mix(theme.background, theme.foreground, 0.30)));
                }
                for n in notes.iter().filter(|n| n.treble == (clef == "clé de sol")) {
                    let x = band.left() + n.start * band.width();
                    let y = band.top() + top_ref + (8.0 - n.degree as f32) * STEP;
                    // ledger lines, exactly where a real ledger line sits (even degrees)
                    let (r, g, b) = hsl_to_rgb(n.hue, 0.75, 0.55);
                    let ink = Color32::from_rgb(r, g, b);
                    if n.ledger > 0 {
                        let mut ld = if n.degree > 8 { 10 } else { -2 };
                        for _ in 0..n.ledger {
                            let ly = band.top() + top_ref + (8.0 - ld as f32) * STEP;
                            painter.hline(egui::Rangef::new(x - 14.0, x + 14.0), ly, Stroke::new(1.5, ink));
                            ld += if n.degree > 8 { 2 } else { -2 };
                        }
                    }
                    // the note-head: a small rounded block, wide like the note lasts
                    let w = (n.len * band.width()).clamp(6.0, 96.0);
                    let head = egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(w, 8.0));
                    painter.rect_filled(head, 3.0, ink);
                    placed.push((x, y, n));
                }
            }

            if let Some(phase) = response.hover_pos() {
                let hovered = placed
                    .iter()
                    .filter(|(x, y, _)| (phase.x - x).abs() < 10.0 && (phase.y - y).abs() < 9.0)
                    .min_by(|a, b| {
                        let da = (phase.x - a.0).powi(2) + (phase.y - a.1).powi(2);
                        let db = (phase.x - b.0).powi(2) + (phase.y - b.1).powi(2);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                if let Some((_, _, n)) = hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    let name = midi_name(n.midi).replace("♯", "#").replace("♭", "b");
                    let voice = n.voice.clone();
                    if response.on_hover_text(format!("{voice} · {name} (MIDI {}) — clic pour aller à sa ligne", n.midi)).clicked() {
                        actions.push(StudioAction::GoToVoice(voice.clone()));
                        actions.push(StudioAction::ArmNotation(voice));
                    }
                }
            }
        });
}
