//! HUM: the capture dialog (countdown, recording with a level meter, result).

use std::time::Instant;

use eframe::egui::{self, Id, RichText};
use loom_app::Theme;

use super::TeacherAction;
use crate::style::color;
use crate::teacher::hum::{HumTake, Step};

/// Draw the dialog for the take in progress.
pub fn show(ctx: &egui::Context, take: &HumTake, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    let response = egui::Modal::new(Id::new("loom-hum")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.heading("🎤 HUM — chante, LOOM écrit la mélodie");
        ui.label(RichText::new(take.prompt()).color(color(theme.dim)));
        ui.add_space(8.0);
        let meter = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.label("micro");
                ui.add(egui::ProgressBar::new(take.level()).desired_width(260.0).fill(color(theme.note)));
            });
        };
        match take.step(Instant::now()) {
            Step::NoMic(text) => {
                ui.label(RichText::new(text).color(color(theme.error)));
                buttons(ui, actions, &[("Recommencer", TeacherAction::HumRetry), ("Fermer", TeacherAction::CloseHum)]);
            }
            Step::Countdown(n) => {
                ui.label(RichText::new(format!("{n}")).size(48.0).strong().color(color(theme.accent)));
                ui.label("Prépare-toi… l'enregistrement commence à zéro.");
                meter(ui);
                buttons(ui, actions, &[("Annuler", TeacherAction::CloseHum)]);
            }
            Step::Recording(progress) => {
                ui.label(RichText::new("⏺ enregistrement — chante !").size(20.0).strong().color(color(theme.error)));
                ui.add(egui::ProgressBar::new(progress).desired_width(320.0).show_percentage());
                meter(ui);
                buttons(ui, actions, &[("Annuler", TeacherAction::CloseHum)]);
            }
            Step::Heard(hum) => {
                ui.label("Mélodie entendue :");
                ui.label(RichText::new(hum.tokens.join(" ")).monospace().size(18.0).color(color(theme.accent)));
                ui.label(RichText::new(&hum.line).monospace().color(color(theme.dim)));
                buttons(ui, actions, &[
                    ("Ajouter au morceau", TeacherAction::HumAccept),
                    ("Recommencer", TeacherAction::HumRetry),
                    ("Annuler", TeacherAction::CloseHum),
                ]);
            }
            Step::Silent(hum) => {
                ui.label(RichText::new(hum.message()).color(color(theme.error)));
                buttons(ui, actions, &[("Recommencer", TeacherAction::HumRetry), ("Fermer", TeacherAction::CloseHum)]);
            }
        }
    });
    if response.should_close() && actions.is_empty() {
        actions.push(TeacherAction::CloseHum);
    }
}

fn buttons(ui: &mut egui::Ui, actions: &mut Vec<TeacherAction>, list: &[(&str, TeacherAction)]) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (label, action) in list {
            if ui.button(*label).clicked() {
                actions.push(action.clone());
            }
        }
    });
}
