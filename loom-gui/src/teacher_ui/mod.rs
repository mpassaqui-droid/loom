//! LE PROF & COMPOSITION in the window: the PROF bar (GÉNÉRER, DÉVELOPPE,
//! ↩ undo, EXPLIQUE, OREILLE, HUM, LEÇONS, Aide), the teacher's answer
//! panel, the ear-training window, the HUM dialog, the lessons panel and
//! the keyboard help.
//!
//! The state lives in [`Teacher`] and the logic in [`loom_app::teacher`] and
//! `loom_app`; these functions only draw and turn clicks into
//! [`TeacherAction`]s, which the app carries out (see `app_teacher.rs`).

pub mod ear;
pub mod hum;
pub mod lessons;
pub mod prof;

use eframe::egui::{self, Button, RichText};
use loom_app::ear::{Demo, Level};
use loom_app::lessons::Lesson;
use loom_app::Theme;

use crate::style::color;
use loom_app::teacher::browse::Tab;
use loom_app::teacher::demo::DemoPlay;
use loom_app::teacher::ear::EarSession;
use loom_app::teacher::history::History;
use loom_app::teacher::hum::HumTake;

/// Something asked for with a PROF button or key.
#[derive(Debug, Clone, PartialEq)]
pub enum TeacherAction {
    /// GÉNÉRER: a new song (Ctrl+Maj+G).
    Generate,
    /// DÉVELOPPE: make every voice evolve (Ctrl+Maj+B).
    Develop,
    /// ↩ the last VIBE / GÉNÉRER / DÉVELOPPE / HUM / leçon (F3).
    Undo,
    /// Bring it back (Maj+F3).
    Redo,
    /// EXPLIQUE: the teacher explains the song (Ctrl+Maj+P).
    Explain,
    /// TRANSFORME: the teacher changes the song to make an idea audible (chip).
    Transform(loom_app::teacher::transform::Kind),
    /// Stop waiting for the teacher.
    CancelProf,
    CloseProf,
    /// Play a teacher's example.
    PlayDemo(Demo),
    /// Open / close OREILLE (Ctrl+Maj+O).
    ToggleEar,
    EarLevel(Level),
    EarListen,
    EarCheck,
    EarReveal,
    EarNext,
    /// Open the HUM dialog (Ctrl+Maj+H).
    OpenHum,
    HumAccept,
    HumRetry,
    CloseHum,
    /// Open / close LEÇONS & RÉFÉRENCE (F12).
    ToggleLessons,
    /// « Essayer »: put the lesson's code in the editor and play it.
    TryLesson(&'static Lesson),
    /// The keyboard help (F1).
    ToggleHelp,
}

/// The teacher's answer shown in the side panel.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfAnswer {
    /// What was asked, e.g. `EXPLIQUE — ton morceau` or `? quinte`.
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
    /// An example to (re)play.
    pub demo: Option<Demo>,
}

/// The lessons panel: which tab, and the search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonsView {
    pub tab: Tab,
    pub query: String,
}

/// Everything the teacher features remember.
#[derive(Default)]
pub struct Teacher {
    /// Undo / redo for VIBE, GÉNÉRER, DÉVELOPPE, HUM and lessons.
    pub history: History,
    /// The answer panel, when open.
    pub prof: Option<ProfAnswer>,
    /// The teacher (Claude) thinking.
    pub prof_job: Option<loom_app::prof::Job>,
    /// The pending job is an EXPLIQUE: keep the offline analysis, append the AI text.
    pub prof_explain: bool,
    /// An example playing instead of the song.
    pub demo: Option<DemoPlay>,
    /// OREILLE, when open.
    pub ear: Option<EarSession>,
    /// HUM, when open.
    pub hum: Option<HumTake>,
    /// LEÇONS, when open.
    pub lessons: Option<LessonsView>,
    pub show_help: bool,
}

/// The PROF bar under the VIBE bar.
pub fn teacher_bar(ui: &mut egui::Ui, teacher: &Teacher, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    egui::Panel::top("teacher")
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(RichText::new("PROF").size(16.0).strong().color(color(theme.accent)))
                    .on_hover_text("Composer et apprendre : générer, développer, expliquer, entraîner l'oreille, chanter, leçons");
                button(ui, actions, "✨ GÉNÉRER", "Un morceau neuf au hasard, dans un genre au hasard (Ctrl+Maj+G). ↩ Annuler rend ton code.", TeacherAction::Generate);
                button(ui, actions, "DÉVELOPPE", "Fait évoluer chaque voix qui ne varie pas encore et change le seed (Ctrl+Maj+B)", TeacherAction::Develop);
                let undo = teacher.history.undo_label().map_or("↩ Annuler".to_string(), |l| format!("↩ Annuler {l}"));
                if ui.add_enabled(teacher.history.undo_label().is_some(), Button::new(undo))
                    .on_hover_text("Revenir au code d'avant le dernier VIBE, GÉNÉRER, DÉVELOPPE, HUM ou leçon (F3)")
                    .clicked()
                {
                    actions.push(TeacherAction::Undo);
                }
                if let Some(label) = teacher.history.redo_label() {
                    if ui.button(format!("Rétablir {label}")).on_hover_text("Refaire ce qui vient d'être annulé (Maj+F3)").clicked() {
                        actions.push(TeacherAction::Redo);
                    }
                }
                ui.separator();
                let thinking = teacher.prof_job.is_some();
                if ui.add_enabled(!thinking, Button::new("EXPLIQUE"))
                    .on_hover_text("Le prof explique ton morceau : ambiance, gamme, pourquoi il sonne ainsi, un conseil (Ctrl+Maj+P). Pour une question : ?quinte dans VIBE.")
                    .clicked()
                {
                    actions.push(TeacherAction::Explain);
                }
                if ui.selectable_label(teacher.ear.is_some(), "👂 OREILLE").on_hover_text("Entraîner l'oreille : intervalles, accords, rythmes (Ctrl+Maj+O)").clicked() {
                    actions.push(TeacherAction::ToggleEar);
                }
                button(ui, actions, "🎤 HUM", "Chante ou siffle 16 temps : LOOM écrit la mélodie (Ctrl+Maj+H)", TeacherAction::OpenHum);
                if ui.selectable_label(teacher.lessons.is_some(), "LEÇONS").on_hover_text("Les 57 leçons et la référence du langage, avec recherche (F12)").clicked() {
                    actions.push(TeacherAction::ToggleLessons);
                }
                if ui.selectable_label(teacher.show_help, "Aide").on_hover_text("Tous les raccourcis clavier (F1)").clicked() {
                    actions.push(TeacherAction::ToggleHelp);
                }
            });
            // TRANSFORME : the prof changes a few lines to make one idea audible.
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(RichText::new("TRANSFORME").size(13.0).strong().color(color(theme.note)))
                    .on_hover_text("Le prof modifie ton morceau (hors-ligne) pour te faire ENTENDRE une idée — ↩ Annuler (F3) rend ton code.");
                use loom_app::teacher::transform::Kind;
                let kinds = [Kind::Transpose(1), Kind::Transpose(-1), Kind::Dark, Kind::Bright, Kind::Dense, Kind::Open, Kind::Groove];
                for kind in kinds {
                    let chip = ui.selectable_label(false, kind.label());
                    if chip.on_hover_text(kind.explanation()).clicked() {
                        actions.push(TeacherAction::Transform(kind));
                    }
                }
            });
        });
}

/// A window that first opens in the middle of the app (then stays where the user drags it).
pub fn centered<'a>(window: egui::Window<'a>, ctx: &egui::Context) -> egui::Window<'a> {
    window.pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center())
}

/// A button that pushes `action` when clicked.
fn button(ui: &mut egui::Ui, actions: &mut Vec<TeacherAction>, text: &str, hover: &str, action: TeacherAction) {
    if ui.button(text).on_hover_text(hover).clicked() {
        actions.push(action);
    }
}

/// Every key of the window, for the help (F1).
pub const KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Jouer",
        &[
            ("Ctrl+Entrée", "jouer / appliquer le code (à la prochaine mesure)"),
            ("Échap · Ctrl+.", "arrêter (Échap annule d'abord VIBE ou le prof s'ils réfléchissent)"),
            ("F4", "REC : enregistrer tout ce qui joue en WAV"),
            ("Ctrl+T", "métronome"),
        ],
    ),
    (
        "Fichiers",
        &[
            ("Ctrl+S · Ctrl+N", "enregistrer · nouveau morceau"),
            ("Ctrl+Maj+E", "exporter en WAV"),
        ],
    ),
    (
        "IA — VIBE",
        &[
            ("F2 · Ctrl+G", "écrire à VIBE : décris la musique, Claude écrit le code"),
            ("?question", "dans VIBE : demander au prof (?quinte, ?raga, ?comment rendre ça plus triste)"),
        ],
    ),
    (
        "Prof & composition",
        &[
            ("Ctrl+Maj+G", "GÉNÉRER un morceau neuf"),
            ("Ctrl+Maj+B", "DÉVELOPPE : faire évoluer les voix"),
            ("F3 · Maj+F3", "annuler · rétablir (VIBE, GÉNÉRER, DÉVELOPPE, HUM, leçon)"),
            ("TRANSFORME", "le prof modifie le morceau pour te faire entendre une idée (chips sous la barre)"),
            ("Ctrl+Maj+P", "EXPLIQUE : le prof explique ton morceau"),
            ("Ctrl+Maj+O", "OREILLE : entraînement de l'oreille"),
            ("Ctrl+Maj+H", "HUM : chanter une mélodie (Échap ferme)"),
            ("F12", "LEÇONS & RÉFÉRENCE"),
            ("F1", "cette aide"),
        ],
    ),
    (
        "Scène",
        &[
            ("F11 · Ctrl+F", "scène plein écran (N scène suivante · V mode VJ · Échap retour)"),
        ],
    ),
    (
        "Studio",
        &[
            ("F6 · F7", "potards · TAP tempo"),
            ("F8 · F9 · F10", "mixeur · piano-roll · clips"),
            ("Ctrl+Maj+L · Ctrl+Maj+I", "partager · importer"),
            ("Ctrl+Maj+D", "démo (une touche l'arrête)"),
        ],
    ),
];

/// The keyboard help window (F1).
pub fn help_window(ctx: &egui::Context, theme: &Theme, actions: &mut Vec<TeacherAction>) {
    let mut open = true;
    let window = egui::Window::new("Aide — raccourcis").open(&mut open).collapsible(false).default_width(620.0);
    centered(window, ctx).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 140.0).max(200.0)).show(ui, |ui| {
            // one grid for every section, so the key column lines up
            egui::Grid::new("loom-help").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                for (section, keys) in KEYS {
                    ui.label(RichText::new(*section).strong().color(color(theme.accent)));
                    ui.end_row();
                    for (key, what) in *keys {
                        ui.label(RichText::new(*key).monospace().color(color(theme.note)));
                        ui.label(*what);
                        ui.end_row();
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Tout le langage : TUTORIEL.md, ou LEÇONS (F12).").color(color(theme.dim)));
        });
    });
    if !open {
        actions.push(TeacherAction::ToggleHelp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The teacher's symbols (and those in loom-app's messages) exist in egui's bundled fonts.
    #[test]
    fn teacher_symbols_exist_in_the_bundled_fonts() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        let font = egui::FontId::proportional(14.0);
        let missing = |c: char| {
            let uv = |ch: char| ctx.fonts_mut(|f| f.layout_no_wrap(ch.to_string(), font.clone(), egui::Color32::WHITE).rows[0].row.glyphs[0].uv_rect);
            let (glyph, boxed) = (uv(c), uv('◻'));
            (glyph.min, glyph.max) == (boxed.min, boxed.max)
        };
        let absent: String = "✨↩♪👂🎤✅❌⏺×▶".chars().filter(|c| missing(*c)).collect();
        output.textures_delta.clear();
        assert!(absent.is_empty(), "no glyph for {absent}");
    }

    #[test]
    fn help_lists_every_teacher_key() {
        let all: Vec<&str> = KEYS.iter().flat_map(|(_, keys)| keys.iter().map(|(k, _)| *k)).collect();
        for key in ["Ctrl+Maj+G", "Ctrl+Maj+B", "F3 · Maj+F3", "Ctrl+Maj+P", "Ctrl+Maj+O", "Ctrl+Maj+H", "F12", "F1", "?question"] {
            assert!(all.contains(&key), "{key} missing from the help");
        }
    }
}
