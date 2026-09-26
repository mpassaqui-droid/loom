//! MIX panel: a channel strip per voice (meter, fader, pan, M, S) and a MASTER strip.

use eframe::egui::{self, Color32, CornerRadius, RichText, Sense};
use loom_app::Theme;

use super::{Studio, StudioAction};
use crate::app::Action;
use crate::scene::logic::{hsl_to_rgb, voice_hue};
use crate::style::{color, mix};

const STRIP_WIDTH: f32 = 70.0;
const METER_HEIGHT: f32 = 64.0;

/// Draw the mixer. `muted` is the app's mute list; `master_peak` the output level.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio, muted: &[String], master_peak: f32, theme: &Theme, actions: &mut Vec<Action>) {
    egui::Panel::bottom("mixer")
        .resizable(false)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Mixeur").strong());
                let hint = if studio.mixer.any_solo() { "· solo actif : seules les voix S jouent" } else { "· M = couper, S = solo, les réglages restent tant que le morceau est ouvert" };
                ui.label(RichText::new(hint).color(color(theme.dim)));
            });
            // scrolls sideways with many voices, and vertically in a short window
            let max_height = (ui.ctx().content_rect().height() * 0.3).max(120.0);
            egui::ScrollArea::both().max_height(max_height).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let count = studio.mixer.visible().len();
                    if count == 0 {
                        ui.label(RichText::new("Aucune voix : écris une voix puis Ctrl+Entrée.").color(color(theme.dim)));
                    }
                    for i in 0..count {
                        strip(ui, studio, i, muted, theme, actions);
                    }
                    master_strip(ui, studio, master_peak, theme, actions);
                });
            });
        });
}

fn strip(ui: &mut egui::Ui, studio: &mut Studio, i: usize, muted: &[String], theme: &Theme, actions: &mut Vec<Action>) {
    let s = &mut studio.mixer.strips[i];
    let is_muted = muted.contains(&s.name);
    let (r, g, b) = hsl_to_rgb(voice_hue(&s.name, None), 0.6, 0.72);
    let voice_color = Color32::from_rgb(r, g, b);
    ui.allocate_ui(egui::vec2(STRIP_WIDTH, 0.0), |ui| {
        ui.vertical_centered(|ui| {
            let name: String = s.name.chars().take(9).collect();
            let name_label = ui.add(egui::Label::new(RichText::new(name).monospace().color(voice_color)).sense(Sense::click()));
            if name_label.on_hover_text(format!("{} · clic : aller à sa ligne", s.name)).clicked() {
                actions.push(Action::Studio(StudioAction::GoToVoice(s.name.clone())));
            }
            ui.horizontal(|ui| {
                meter(ui, s.meter_fraction(), Some(s.hold_fraction()), s.meter, theme);
                let fader = egui::Slider::new(&mut s.gain, 0.0..=2.0).vertical().show_value(false);
                let fader = colored_grip(ui, voice_color, |ui| ui.add_sized(egui::vec2(18.0, METER_HEIGHT), fader));
                if fader.on_hover_text("Volume de la voix (100 % = comme écrit)").changed() {
                    actions.push(Action::Studio(StudioAction::StripChanged(i)));
                }
            });
            ui.label(RichText::new(s.gain_text()).small().monospace());
            let pan = egui::Slider::new(&mut s.pan, -1.0..=1.0).show_value(false);
            let pan = colored_grip(ui, voice_color, |ui| ui.add_sized(egui::vec2(STRIP_WIDTH - 8.0, 16.0), pan));
            if pan.on_hover_text("Pan : gauche ↔ droite (double-clic : centre)").changed() {
                actions.push(Action::Studio(StudioAction::StripChanged(i)));
            }
            ui.label(RichText::new(s.pan_text()).small().monospace().color(color(theme.dim)));
            ui.horizontal(|ui| {
                let m = egui::Button::new(RichText::new("M").strong().color(if is_muted { color(theme.background) } else { color(theme.dim) }))
                    .fill(if is_muted { color(theme.error) } else { color(theme.panel) });
                if ui.add(m).on_hover_text("Mute : couper cette voix").clicked() {
                    actions.push(Action::ToggleMute(s.name.clone()));
                }
                let solo = s.solo;
                let sb = egui::Button::new(RichText::new("S").strong().color(if solo { color(theme.background) } else { color(theme.dim) }))
                    .fill(if solo { color(theme.drum) } else { color(theme.panel) });
                if ui.add(sb).on_hover_text("Solo : n'entendre que les voix en solo").clicked() {
                    actions.push(Action::Studio(StudioAction::ToggleSolo(i)));
                }
            });
        });
    });
    ui.separator();
}

fn master_strip(ui: &mut egui::Ui, studio: &mut Studio, peak: f32, theme: &Theme, actions: &mut Vec<Action>) {
    use loom_app::studio::knobs::Knob;
    ui.allocate_ui(egui::vec2(STRIP_WIDTH, 0.0), |ui| {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("MASTER").monospace().color(color(theme.accent)));
            ui.horizontal(|ui| {
                meter(ui, loom_app::studio::mixer::meter_fraction(peak), None, peak, theme);
                let (min, max, step) = Knob::Volume.range();
                let fader = egui::Slider::new(&mut studio.knobs.volume, min..=max).step_by(step as f64).vertical().show_value(false);
                let fader = colored_grip(ui, color(theme.accent), |ui| ui.add_sized(egui::vec2(18.0, METER_HEIGHT), fader));
                if fader.on_hover_text("Volume général (VOL)").changed() {
                    actions.push(Action::Studio(StudioAction::KnobChanged(Knob::Volume)));
                }
            });
            ui.label(RichText::new(studio.knobs.text(Knob::Volume)).small().monospace());
            let sends = studio.mixer.sends.clone();
            if !sends.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("RETOURS").small().monospace().color(color(theme.dim)));
                ui.horizontal(|ui| {
                    for (i, snd) in sends.iter().enumerate() {
                        ui.vertical_centered(|ui| {
                            let mut level = snd.level;
                            let grip = color(theme.note);
                            let slider = egui::Slider::new(&mut level, 0.0..=1.0).vertical().show_value(false);
                            let slider = colored_grip(ui, grip, |ui| ui.add_sized(egui::vec2(12.0, METER_HEIGHT * 0.6), slider));
                            if slider.changed() {
                                studio.mixer.sends[i].level = level;
                                actions.push(Action::Studio(StudioAction::SendLevel(i)));
                            }
                            slider.on_hover_text(format!("Retour {} (wet)", snd.name));
                            ui.label(RichText::new(format!("{:.0}%", level * 100.0)).small().monospace().color(color(theme.dim)));
                            ui.label(RichText::new(&snd.name).small().monospace().color(color(theme.foreground)));
                        });
                    }
                });
            }
        });
    });
}

/// Run `add` with the slider grips painted in `grip` (fader / pan handles take
/// their colour from [`egui::Visuals`], so we tint them around just these widgets
/// and restore the previous colours afterwards).
fn colored_grip(ui: &mut egui::Ui, grip: Color32, add: impl FnOnce(&mut egui::Ui) -> egui::Response) -> egui::Response {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        let keep = [
            visuals.widgets.inactive.bg_fill,
            visuals.widgets.hovered.bg_fill,
            visuals.widgets.active.bg_fill,
        ];
        visuals.widgets.inactive.bg_fill = grip;
        visuals.widgets.hovered.bg_fill = grip;
        visuals.widgets.active.bg_fill = grip;
        let response = add(ui);
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.bg_fill = keep[0];
        visuals.widgets.hovered.bg_fill = keep[1];
        visuals.widgets.active.bg_fill = keep[2];
        response
    })
    .inner
}

/// A vertical level meter filled to `fraction`; `hold` (if any) is the peak-hold
/// marker — a thin bright line — amber above a level of 0.6, red above 1 (like
/// the browser).
fn meter(ui: &mut egui::Ui, fraction: f32, hold: Option<f32>, level: f32, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, METER_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(2), mix(theme.background, theme.panel, 0.3));
    let height = rect.height() * fraction.clamp(0.0, 1.0);
    let mut bar = rect;
    bar.set_top(rect.bottom() - height);
    let fill = if level > 1.0 { color(theme.error) } else if level > 0.6 { color(theme.drum) } else { color(theme.note) };
    painter.rect_filled(bar, CornerRadius::same(2), fill);
    // the peak-hold: a 1px hint of where the level peaked a few frames ago
    if let Some(h) = hold {
        let h = h.clamp(0.0, 1.0);
        let marker = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - rect.height() * h),
            egui::pos2(rect.right(), rect.bottom() - rect.height() * h + 2.0),
        );
        painter.rect_filled(marker, 0.0, color(theme.foreground));
    }
}
