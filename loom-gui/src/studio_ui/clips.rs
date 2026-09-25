//! CLIPS panel: launch an example song at the next bar, or vary one voice live.

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};
use loom_app::Theme;

use super::{Studio, StudioAction};
use crate::scene::logic::{hsl_to_rgb, voice_hue};
use loom_app::studio::clips::SLOTS;
use crate::style::color;

/// Draw the clips. `songs` are the songs offered for launching.
pub fn show(ui: &mut egui::Ui, studio: &Studio, songs: &[PathBuf], theme: &Theme, actions: &mut Vec<StudioAction>) {
    egui::Panel::bottom("clips")
        .resizable(true)
        .default_size(150.0)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(RichText::new("Clips — lancer un morceau (il démarre à la prochaine mesure)").strong());
                ui.horizontal_wrapped(|ui| {
                    for song in songs {
                        let name = song.file_stem().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default();
                        if ui.small_button(name).on_hover_text(song.display().to_string()).clicked() {
                            actions.push(StudioAction::Launch(song.clone()));
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("— clips par voix (variation live, quantisée, le code ne change pas) —").color(color(theme.dim)));
                for strip in studio.mixer.visible() {
                    ui.horizontal(|ui| {
                        let (r, g, b) = hsl_to_rgb(voice_hue(&strip.name, None), 0.6, 0.72);
                        let name: String = strip.name.chars().take(10).collect();
                        ui.add_sized(egui::vec2(80.0, 16.0), egui::Label::new(RichText::new(name).monospace().color(Color32::from_rgb(r, g, b))));
                        let playing = studio.clips.verb_for(&strip.name);
                        for (label, verb) in SLOTS {
                            let hover = if verb.is_empty() { "comme écrit".to_string() } else { format!("| {verb}") };
                            if ui.selectable_label(playing == verb, label).on_hover_text(hover).clicked() && playing != verb {
                                actions.push(StudioAction::Clip(strip.name.clone(), verb));
                            }
                        }
                    });
                }
            });
        });
}
