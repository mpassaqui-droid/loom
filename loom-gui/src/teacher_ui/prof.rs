//! The teacher's answer panel (right side): EXPLIQUE and `?question`.

use eframe::egui::{self, Align, Layout, RichText};
use loom_app::Theme;

use super::{Teacher, TeacherAction};
use crate::style::color;

/// Draw the panel when it's open: title, spinner while thinking, the answer, replay.
pub fn show(ui: &mut egui::Ui, teacher: &Teacher, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    let Some(answer) = &teacher.prof else { return };
    egui::Panel::right("prof")
        .resizable(true)
        .default_size(360.0)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Le prof").size(18.0).strong().color(color(theme.accent)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Fermer le panneau").clicked() {
                        actions.push(TeacherAction::CloseProf);
                    }
                });
            });
            ui.label(RichText::new(&answer.title).strong());
            ui.separator();
            if let Some(job) = &teacher.prof_job {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().color(color(theme.accent)));
                    ui.label(format!("le prof réfléchit… {} s", job.elapsed().as_secs()));
                    if ui.button("Annuler").on_hover_text("Arrêter d'attendre le prof (Échap)").clicked() {
                        actions.push(TeacherAction::CancelProf);
                    }
                });
            }
            if let Some(demo) = &answer.demo {
                if ui.button("♪ Réécouter l'exemple").on_hover_text("Le moteur joue l'exemple une mesure, puis ton morceau revient (arrêté)").clicked() {
                    actions.push(TeacherAction::PlayDemo(demo.clone()));
                }
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let tint = if answer.is_error { color(theme.error) } else { color(theme.foreground) };
                for line in &answer.lines {
                    ui.label(RichText::new(line).size(15.0).color(tint));
                    ui.add_space(6.0);
                }
                if answer.lines.is_empty() && teacher.prof_job.is_none() {
                    ui.label(RichText::new("(pas de réponse)").color(color(theme.dim)));
                }
            });
        });
}
