//! OREILLE: the ear-training window.

use eframe::egui::{self, Button, Id, Key, RichText};
use loom_app::Theme;

use super::TeacherAction;
use crate::style::color;
use loom_app::teacher::ear::{level_name, EarSession, Tone, LEVELS};

/// Widget id of the answer field (focused after each check).
const ANSWER_FIELD: &str = "loom-ear-answer";

/// Draw the window. The answer field is edited in place; buttons become actions.
pub fn show(ctx: &egui::Context, session: &mut EarSession, playing: bool, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    let mut open = true;
    let window = egui::Window::new("👂 Oreille — entraînement").open(&mut open).collapsible(false).resizable(false).default_width(440.0);
    super::centered(window, ctx)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Niveau :");
                for (level, _) in LEVELS {
                    if ui.selectable_label(session.level == level, level_name(level)).clicked() && session.level != level {
                        actions.push(TeacherAction::EarLevel(level));
                    }
                }
            });
            ui.separator();
            ui.label(RichText::new(session.prompt()).size(15.0));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let listen = Button::new(RichText::new("♪ Écouter").strong());
                if ui.add_enabled(!playing, listen)
                    .on_hover_text("Le moteur joue l'exemple une mesure (ton morceau s'arrête, Ctrl+Entrée le relance)")
                    .clicked()
                {
                    actions.push(TeacherAction::EarListen);
                }
                if playing {
                    ui.label(RichText::new("écoute…").color(color(theme.accent)));
                }
            });
            ui.horizontal(|ui| {
                let field = egui::TextEdit::singleline(&mut session.input)
                    .id(Id::new(ANSWER_FIELD))
                    .hint_text("ta réponse, ex. quinte")
                    .desired_width(220.0);
                let response = ui.add(field);
                let entered = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let clicked = ui.add_enabled(!session.input.trim().is_empty(), Button::new("Vérifier")).clicked();
                if entered || clicked {
                    if !session.input.trim().is_empty() {
                        actions.push(TeacherAction::EarCheck);
                    }
                    response.request_focus();
                }
            });
            if let Some((text, tone)) = session.verdict() {
                let tint = match tone {
                    Tone::Good => color(theme.note),
                    Tone::Bad => color(theme.error),
                    Tone::Neutral => color(theme.dim),
                };
                ui.label(RichText::new(text).size(15.0).color(tint));
            }
            if let Some(answer) = session.revealed() {
                ui.label(RichText::new(format!("Réponse : {answer}")).strong().color(color(theme.accent)));
            }
            ui.label(RichText::new(session.progress()).color(color(theme.dim)));
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Réponse").on_hover_text("Voir la réponse (elle ne compte plus dans le score)").clicked() {
                    actions.push(TeacherAction::EarReveal);
                }
                if ui.button("Suivant").on_hover_text("Un nouvel exercice, joué tout de suite").clicked() {
                    actions.push(TeacherAction::EarNext);
                }
            });
        });
    if !open {
        actions.push(TeacherAction::ToggleEar);
    }
}
