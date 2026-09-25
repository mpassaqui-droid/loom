//! LEÇONS & RÉFÉRENCE: the right-side panel with two tabs, a search and the list.

use eframe::egui::{self, Align, Layout, RichText};
use loom_app::lessons::{LESSONS, REFERENCE};
use loom_app::Theme;

use super::{LessonsView, TeacherAction};
use crate::style::color;
use loom_app::teacher::browse::{self, Row, Tab};

/// Draw the panel. The search box and the tab are edited in place.
pub fn show(ui: &mut egui::Ui, view: &mut LessonsView, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    egui::Panel::right("lessons")
        .resizable(true)
        .default_size(400.0)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Leçons & référence").size(18.0).strong().color(color(theme.accent)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Fermer (F12)").clicked() {
                        actions.push(TeacherAction::ToggleLessons);
                    }
                });
            });
            ui.horizontal(|ui| {
                for (tab, count) in [(Tab::Lessons, LESSONS.len()), (Tab::Reference, REFERENCE.len())] {
                    if ui.selectable_label(view.tab == tab, format!("{} ({count})", tab.name())).clicked() {
                        view.tab = tab;
                    }
                }
            });
            ui.add(egui::TextEdit::singleline(&mut view.query).hint_text("chercher : rythme, développe, raga…").desired_width(f32::INFINITY));
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match view.tab {
                Tab::Lessons => lesson_list(ui, &view.query, theme, actions),
                Tab::Reference => reference_list(ui, &view.query, theme),
            });
        });
}

fn lesson_list(ui: &mut egui::Ui, query: &str, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    let found = browse::rows(Tab::Lessons, query);
    if found.is_empty() {
        ui.label(RichText::new("Aucune leçon ne correspond.").color(color(theme.dim)));
    }
    for row in found {
        let Row::Lesson(lesson) = row else { continue };
        egui::CollapsingHeader::new(RichText::new(lesson.title).strong()).id_salt(lesson.title).show(ui, |ui| {
            ui.label(lesson.description);
            egui::Frame::new().fill(color(theme.background)).inner_margin(6).corner_radius(4).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(lesson.code.trim_end()).monospace().color(color(theme.foreground))).wrap_mode(egui::TextWrapMode::Wrap));
            });
            if ui.button("▶ Essayer").on_hover_text("Le code va dans l'éditeur et joue (↩ Annuler / F3 rend ton morceau)").clicked() {
                actions.push(TeacherAction::TryLesson(lesson));
            }
        });
    }
}

fn reference_list(ui: &mut egui::Ui, query: &str, theme: &Theme) {
    let found = browse::rows(Tab::Reference, query);
    if found.is_empty() {
        ui.label(RichText::new("Rien dans la référence.").color(color(theme.dim)));
    }
    for row in found {
        match row {
            Row::Section(section) => {
                ui.add_space(6.0);
                ui.label(RichText::new(section).strong().color(color(theme.accent)));
            }
            Row::Reference(reference) => {
                ui.label(RichText::new(reference.syntax).monospace().color(color(theme.note)));
                ui.indent(("ref", reference.syntax, reference.section), |ui| {
                    ui.label(reference.explanation);
                });
            }
            Row::Lesson(_) => {}
        }
    }
}
