//! TIMELINE panel: the `song:` arrangement as blocks as wide as their bars,
//! with the playhead, and buttons to edit the selected section.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind};
use loom_app::Theme;

use super::StudioAction;
use crate::scene::logic::{hsl_to_rgb, voice_hue};
use crate::studio::songline::{Section, SongEdit};
use crate::style::color;

const HEIGHT: f32 = 30.0;

/// Draw the arrangement. `position` is the playhead in bars (while playing).
pub fn show(ui: &mut egui::Ui, song: &[Section], selected: Option<usize>, position: Option<f32>, theme: &Theme, actions: &mut Vec<StudioAction>) {
    egui::Panel::bottom("timeline")
        .resizable(false)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            let total: u32 = song.iter().map(|(_, bars)| bars).sum();
            let width = ui.available_width(); // before the header, which may be wider than the window
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Arrangement").strong());
                ui.label(RichText::new(format!("{} sections · {total} mesures", song.len())).color(color(theme.dim)));
                match selected.and_then(|i| song.get(i)) {
                    Some((name, bars)) => {
                        ui.separator();
                        ui.label(RichText::new(format!("« {name} » ({bars} mes.)")).color(color(theme.accent)));
                        let buttons: [(&str, &str, SongEdit); 6] = [
                            ("−", "−1 mesure", SongEdit::Shorter),
                            ("+", "+1 mesure", SongEdit::Longer),
                            ("dup", "dupliquer la section", SongEdit::Duplicate),
                            ("◀", "déplacer à gauche", SongEdit::MoveLeft),
                            ("▶", "déplacer à droite", SongEdit::MoveRight),
                            ("×", "supprimer la section", SongEdit::Delete),
                        ];
                        for (label, hover, edit) in buttons {
                            if ui.small_button(label).on_hover_text(hover).clicked() {
                                actions.push(StudioAction::EditSong(edit));
                            }
                        }
                    }
                    None => {
                        ui.label(RichText::new("· clic sur une section pour la modifier").color(color(theme.dim)));
                    }
                }
            });
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, HEIGHT), Sense::hover());
            if total == 0 {
                return;
            }
            let gap = 2.0;
            let usable = rect.width() - gap * song.len().saturating_sub(1) as f32;
            let mut x = rect.left();
            for (i, (name, bars)) in song.iter().enumerate() {
                let w = usable * *bars as f32 / total as f32;
                let block = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height()));
                let response = ui.interact(block, ui.id().with(("section", i)), Sense::click())
                    .on_hover_text(format!("{name} · {bars} mes. — clic pour la sélectionner"));
                let is_selected = selected == Some(i);
                let (r, g, b) = hsl_to_rgb(voice_hue(name, None), 0.6, 0.45);
                let painter = ui.painter();
                painter.rect_filled(block, CornerRadius::same(2), Color32::from_rgba_unmultiplied(r, g, b, if is_selected { 140 } else { 60 }));
                let stroke = if is_selected { Stroke::new(2.0, color(theme.accent)) } else { Stroke::new(1.0, color(theme.dim)) };
                painter.rect_stroke(block, CornerRadius::same(2), stroke, StrokeKind::Inside);
                if w > 24.0 {
                    let text = painter.layout_no_wrap(name.clone(), FontId::monospace(12.0), color(theme.foreground));
                    if text.size().x < w - 4.0 {
                        painter.galley(egui::pos2(block.center().x - text.size().x / 2.0, block.center().y - text.size().y / 2.0), text, color(theme.foreground));
                    } else {
                        painter.text(block.left_center() + egui::vec2(3.0, 0.0), Align2::LEFT_CENTER, name.chars().next().unwrap_or(' '), FontId::monospace(12.0), color(theme.foreground));
                    }
                }
                if response.clicked() {
                    actions.push(StudioAction::SelectSection(if is_selected { None } else { Some(i) }));
                }
                x += w + gap;
            }
            if let Some(bar) = position {
                let px = rect.left() + (bar / total as f32).fract() * rect.width();
                ui.painter().vline(px, (rect.top() - 2.0)..=(rect.bottom() + 2.0), Stroke::new(2.0, color(theme.accent)));
            }
        });
}
