//! What LE PROF & COMPOSITION do inside the window: carrying out
//! [`TeacherAction`]s, the work done every frame (the teacher's answer, the
//! example that plays, the HUM take) and drawing the teacher's panels.
//!
//! This file is a child module of `app.rs`, so it can read and change the
//! app's state directly; `app.rs` only calls the few functions below.

use std::time::{Duration, Instant};

use eframe::egui;
use loom_app::ear::{Demo, Level};
use loom_app::{develop, generate, hum, prof};
use loom_audio::input::MicInput;
use loom_core::Program;

use super::{Action, LoomApp};
use loom_app::teacher::browse::Tab;
use loom_app::teacher::demo::DemoPlay;
use loom_app::teacher::ear::EarSession;
use loom_app::teacher::fresh_seed;
use loom_app::teacher::hum::HumTake;
use loom_app::teacher::route::{self, Answer};
use loom_app::teacher::transform;
use crate::teacher_ui::{self, LessonsView, ProfAnswer, TeacherAction};

impl LoomApp {
    /// Carry out a teacher action.
    pub(super) fn teacher_action(&mut self, action: TeacherAction) {
        match action {
            TeacherAction::Generate => {
                let song = generate::compose(fresh_seed());
                let when = self.replace_song("GÉNÉRER", song.code);
                self.say(format!("{} — genre : {}{when}", generate::DONE, song.genre));
            }
            TeacherAction::Develop => match develop::develop(&self.text, fresh_seed() % 99 + 1) {
                Ok(developed) => {
                    let when = self.replace_song("DÉVELOPPE", developed.code.clone());
                    self.say(format!("{} (F3){when}", developed.message()));
                }
                Err(why) => self.say(why),
            },
            TeacherAction::Undo => match self.teacher.history.undo(&self.text) {
                Some(back) => {
                    self.set_editor_code(back.code);
                    self.say(format!("[ ↩ ] annulé : {} — le code d'avant est revenu (Maj+F3 pour rétablir).", back.label));
                }
                None => self.say("Rien à annuler."),
            },
            TeacherAction::Redo => match self.teacher.history.redo(&self.text) {
                Some(again) => {
                    self.set_editor_code(again.code);
                    self.say(format!("Rétabli : {} (F3 pour l'annuler).", again.label));
                }
                None => self.say("Rien à rétablir."),
            },
            TeacherAction::Explain => self.explain(),
            TeacherAction::Transform(kind) => match transform::transform(&self.text, kind, fresh_seed() % 99 + 1) {
                Ok(t) => {
                    let when = self.replace_song("TRANSFORME", t.code);
                    let mut lines = vec![kind.explanation(), String::new(), "Ce que j'ai changé :".into()];
                    lines.extend(t.touches.iter().cloned());
                    self.teacher.prof = Some(ProfAnswer { title: format!("TRANSFORME — {}", kind.label()), lines, is_error: false, demo: None });
                    self.say(format!("[ prof ] « {} » : {} ligne(s) modifiée(s) — écoute la différence (F3 pour annuler){when}", kind.label(), t.touches.len()));
                }
                Err(why) => self.say(why),
            },
            TeacherAction::CancelProf => {
                if self.teacher.prof_job.take().is_some() {
                    if let Some(answer) = &mut self.teacher.prof {
                        answer.lines = vec!["Question annulée.".into()];
                        answer.is_error = false;
                    }
                    self.say("Le prof : annulé.");
                }
            }
            TeacherAction::CloseProf => {
                self.teacher.prof = None;
                self.teacher.prof_job = None;
            }
            TeacherAction::PlayDemo(demo) => self.play_demo(&demo),
            TeacherAction::ToggleEar => {
                self.teacher.ear = match self.teacher.ear {
                    Some(_) => None,
                    None => Some(EarSession::new(Level::Intervalles, fresh_seed())),
                };
            }
            TeacherAction::EarLevel(level) => {
                if let Some(ear) = &mut self.teacher.ear {
                    ear.set_level(level, fresh_seed());
                }
            }
            TeacherAction::EarListen => self.ear_listen(),
            TeacherAction::EarCheck => {
                let replay = self.teacher.ear.as_mut().is_some_and(|ear| ear.check().replay);
                if replay {
                    self.ear_listen();
                }
            }
            TeacherAction::EarReveal => {
                if let Some(ear) = &mut self.teacher.ear {
                    ear.reveal();
                }
            }
            TeacherAction::EarNext => {
                if let Some(ear) = &mut self.teacher.ear {
                    ear.next(fresh_seed());
                }
                self.ear_listen();
            }
            TeacherAction::OpenHum | TeacherAction::HumRetry => {
                self.teacher.hum = Some(HumTake::new(MicInput::start(), &self.text, Instant::now()));
            }
            TeacherAction::HumAccept => {
                let heard = self.teacher.hum.take().and_then(|take| take.result().filter(|h| !h.is_silent()).cloned());
                if let Some(melody) = heard {
                    let when = self.replace_song("HUM", hum::append(&self.text, &melody.line));
                    self.say(format!("{} (F3 pour l'enlever){when}", melody.message()));
                }
            }
            TeacherAction::CloseHum => self.teacher.hum = None,
            TeacherAction::ToggleLessons => {
                self.teacher.lessons = match self.teacher.lessons {
                    Some(_) => None,
                    None => Some(LessonsView { tab: Tab::Lessons, query: String::new() }),
                };
            }
            TeacherAction::TryLesson(lesson) => {
                self.record_and_set("LEÇON", lesson.code.to_string());
                if self.engine.is_some() {
                    self.run();
                    self.say(format!("Leçon « {} » : elle joue. ↩ Annuler (F3) rend ton morceau.", lesson.title));
                } else {
                    self.set_program(Program::parse(&self.text));
                    self.warn(format!("Leçon « {} » dans l'éditeur, mais pas de sortie audio. ↩ Annuler (F3) rend ton morceau.", lesson.title));
                }
            }
            TeacherAction::ToggleHelp => self.teacher.show_help = !self.teacher.show_help,
        }
    }

    /// Replace the whole editor text with `code` from `label` (VIBE, GÉNÉRER…),
    /// undoably. While playing it is heard at the next bar. Returns how to
    /// hear it, to end the status message with.
    pub(super) fn replace_song(&mut self, label: &str, code: String) -> &'static str {
        self.record_and_set(label, code);
        if self.is_playing() && self.teacher.demo.is_none() {
            self.run();
            " (à la prochaine mesure)"
        } else {
            self.set_program(Program::parse(&self.text));
            " — Ctrl+Entrée pour l'entendre"
        }
    }

    /// Remember the current text for undo, then put `code` in the editor.
    fn record_and_set(&mut self, label: &str, code: String) {
        if code != self.text {
            self.teacher.history.record(label, &self.text);
        }
        self.vibe_summary = None;
        self.text = code;
    }

    /// Put an undone / redone text in the editor and let the grid (and the engine, if playing) follow.
    fn set_editor_code(&mut self, code: String) {
        self.text = code;
        self.vibe_summary = None;
        if self.is_playing() && self.teacher.demo.is_none() {
            self.run();
        } else {
            self.set_program(Program::parse(&self.text));
        }
    }

    /// The VIBE box's text: `?question` goes to the teacher, the rest to VIBE.
    /// Returns the words for VIBE, if it's a VIBE request.
    pub(super) fn route_vibe_input(&mut self) -> Option<String> {
        match route::route(&self.vibe_input) {
            route::Route::Empty => None,
            route::Route::Vibe(words) => Some(words),
            route::Route::Question(question) => {
                self.vibe_input.clear();
                self.ask_teacher(&question);
                None
            }
        }
    }

    /// `?question`: the offline teacher first, Claude only if it doesn't know.
    fn ask_teacher(&mut self, question: &str) {
        let title = if question.is_empty() { "?".to_string() } else { format!("? {question}") };
        match route::answer(question) {
            Answer::Known(found) => {
                self.teacher.prof_job = None;
                self.teacher.prof = Some(ProfAnswer { title, lines: found.lines, is_error: false, demo: found.demo.clone() });
                if let Some(demo) = found.demo {
                    self.play_demo(&demo);
                }
            }
            Answer::AskClaude(question) => {
                self.teacher.prof_explain = false;
                self.teacher.prof_job = Some(prof::ask(&question));
                self.teacher.prof = Some(ProfAnswer { title, lines: Vec::new(), is_error: false, demo: None });
                self.say("Le prof réfléchit à ta question…");
            }
        }
    }

    /// EXPLIQUE: ask the teacher about the song in the editor.
    fn explain(&mut self) {
        if self.teacher.prof_job.is_some() {
            return;
        }
        self.teacher.prof_explain = true;
        self.teacher.prof_job = Some(prof::explain(&self.text));
        // The web shows the offline analysis first, then the AI's answer.
        self.teacher.prof = Some(ProfAnswer { title: "EXPLIQUE — ton morceau".into(), lines: prof::analyze(&self.text), is_error: false, demo: None });
        self.say(prof::THINKING);
    }

    /// Play the current ear-training example.
    fn ear_listen(&mut self) {
        if let Some(demo) = self.teacher.ear.as_ref().map(|ear| ear.exercise().demo()) {
            self.play_demo(&demo);
        }
    }

    /// Play a one-bar example instead of the song (see `teacher::demo`).
    fn play_demo(&mut self, demo: &Demo) {
        let Some(engine) = &self.engine else {
            self.warn("Pas de sortie audio : impossible de jouer l'exemple.");
            return;
        };
        let play = DemoPlay::start(engine, demo, Instant::now(), self.teacher.demo.as_ref());
        self.teacher.demo = Some(play);
    }

    /// The example is over (or was interrupted): give the engine the song back, stopped.
    fn finish_demo(&mut self) {
        let Some(play) = self.teacher.demo.take() else { return };
        if let Some(engine) = &self.engine {
            DemoPlay::finish(engine, &self.engine_code());
        }
        if play.song_was_playing {
            self.say("Exemple joué : ton morceau est arrêté, Ctrl+Entrée le relance.");
        }
    }

    /// Work done every frame: the teacher's answer, the end of an example, the HUM take.
    pub(super) fn teacher_frame(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.teacher.prof_job.as_mut().and_then(prof::Job::poll) {
            self.teacher.prof_job = None;
            let was_explain = std::mem::take(&mut self.teacher.prof_explain);
            let (lines, is_error) = match result {
                Ok(text) => {
                    self.say("Le prof a répondu.");
                    // EXPLIQUE: the offline lines are already shown, append the AI text.
                    if was_explain {
                        if let Some(answer) = &mut self.teacher.prof {
                            answer.lines.push(text);
                        }
                        self.teacher.prof_explain = false;
                        return;
                    }
                    (vec![text], false)
                }
                Err(e) => {
                    self.warn(format!("Le prof : {e}"));
                    (vec![e], true)
                }
            };
            if let Some(answer) = &mut self.teacher.prof {
                answer.lines = lines;
                answer.is_error = is_error;
            }
        }
        if self.teacher.demo.as_ref().is_some_and(|d| d.is_over(Instant::now())) {
            self.finish_demo();
        }
        if let Some(take) = &mut self.teacher.hum {
            take.tick(Instant::now());
        }
        if self.teacher.hum.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30)); // countdown and level meter
        } else if self.teacher.demo.is_some() || self.teacher.prof_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30)); // stop the example on time, spinner
        }
    }

    /// Something else took over the engine (RUN, STOP, a new song): the example just ends.
    pub(super) fn forget_demo(&mut self) {
        self.teacher.demo = None;
    }

    /// The PROF bar.
    pub(super) fn teacher_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let mut teacher_actions = Vec::new();
        let theme = self.theme.clone();
        teacher_ui::teacher_bar(ui, &self.teacher, &theme, &mut teacher_actions);
        actions.extend(teacher_actions.into_iter().map(Action::Teacher));
    }

    /// The side panels (teacher's answer, lessons), drawn before the editor.
    pub(super) fn teacher_panels(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let mut teacher_actions = Vec::new();
        let theme = self.theme.clone();
        if let Some(view) = &mut self.teacher.lessons {
            teacher_ui::lessons::show(ui, view, &theme, &mut teacher_actions);
        }
        teacher_ui::prof::show(ui, &self.teacher, &theme, &mut teacher_actions);
        actions.extend(teacher_actions.into_iter().map(Action::Teacher));
    }

    /// The windows: OREILLE, HUM, the keyboard help.
    pub(super) fn teacher_windows(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let mut teacher_actions = Vec::new();
        let theme = self.theme.clone();
        let playing_demo = self.teacher.demo.is_some();
        if let Some(ear) = &mut self.teacher.ear {
            teacher_ui::ear::show(ctx, ear, playing_demo, &theme, &mut teacher_actions);
        }
        if let Some(take) = &self.teacher.hum {
            teacher_ui::hum::show(ctx, take, &theme, &mut teacher_actions);
        }
        if self.teacher.show_help {
            teacher_ui::help_window(ctx, &theme, &mut teacher_actions);
        }
        actions.extend(teacher_actions.into_iter().map(Action::Teacher));
    }
}
