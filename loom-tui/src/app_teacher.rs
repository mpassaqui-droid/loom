//! LE PROF & COMPOSITION in the terminal: what their keys do, and the work
//! done every frame.
//!
//! | key            | what                                                         |
//! |----------------|--------------------------------------------------------------|
//! | Alt+G          | GÉNÉRER: a new song (undoable)                                |
//! | Alt+B          | DÉVELOPPE: make every voice evolve (undoable)                 |
//! | F3 · Maj+F3/^Y | undo · redo (VIBE, GÉNÉRER, DÉVELOPPE, HUM, lesson)           |
//! | Alt+E          | EXPLIQUE: the teacher explains the song (popup)               |
//! | F2 `?question` | the teacher answers (offline first, then Claude)              |
//! | Alt+O          | OREILLE: ear training (popup)                                 |
//! | Alt+H          | HUM: sing a melody (popup)                                    |
//! | Alt+L          | LEÇONS & RÉFÉRENCE (popup with search)                        |
//!
//! This file is a child module of `app.rs`, so it can use the app's state directly.

use std::time::Instant;

use loom_app::ear::{Demo, Level};
use loom_app::{develop, generate, hum, prof};
use loom_audio::input::MicInput;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Mode};
use loom_app::teacher::browse::{self, Row, Tab};
use loom_app::teacher::demo::DemoPlay;
use loom_app::teacher::ear::{EarSession, LEVELS};
use loom_app::teacher::history::History;
use loom_app::teacher::hum::{HumTake, Step};
use loom_app::teacher::route::{self, Answer, Route};
use loom_app::teacher::fresh_seed;

/// The teacher's answer shown in the popup.
pub struct ProfAnswer {
    /// `EXPLIQUE — ton morceau`, `? quinte`…
    pub title: String,
    pub lines: Vec<String>,
    pub is_error: bool,
    pub demo: Option<Demo>,
}

/// Everything the teacher features remember.
#[derive(Default)]
pub struct Teacher {
    pub history: History,
    pub prof: Option<ProfAnswer>,
    pub prof_job: Option<prof::Job>,
    /// The pending job is an EXPLIQUE: keep the offline analysis, append the AI text.
    pub prof_explain: bool,
    pub demo: Option<DemoPlay>,
    /// Kept when the popup closes, so the score stays.
    pub ear: Option<EarSession>,
    pub hum: Option<HumTake>,
}

/// What a key does in the editor for the teacher (pure, so it is tested).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    Generate,
    Develop,
    Undo,
    Redo,
    Explain,
    Ear,
    Hum,
    Lessons,
}

/// The teacher's keys in the editor: Alt+letters, F3, and Shift+F3 or Ctrl+Y (some
/// terminals send Shift+F3 as F15, others as a code crossterm can't read).
pub fn edit_key(key: &KeyEvent) -> Option<EditKey> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    Some(match key.code {
        KeyCode::F(3) if shift => EditKey::Redo,
        KeyCode::F(15) => EditKey::Redo,
        KeyCode::Char('y' | 'Y') if ctrl && !alt => EditKey::Redo,
        KeyCode::F(3) => EditKey::Undo,
        KeyCode::Char(c) if alt && !ctrl => match c.to_ascii_lowercase() {
            'g' => EditKey::Generate,
            'b' => EditKey::Develop,
            'e' => EditKey::Explain,
            'o' => EditKey::Ear,
            'h' => EditKey::Hum,
            'l' => EditKey::Lessons,
            _ => return None,
        },
        _ => return None,
    })
}

impl App {
    /// Keys for the teacher. Returns true when the key was used here.
    pub(super) fn teacher_key(&mut self, key: KeyEvent) -> bool {
        match &self.mode {
            Mode::Prof { .. } => self.prof_key(key),
            Mode::Ear => self.ear_key(key),
            Mode::Hum => self.hum_key(key),
            Mode::Lessons { .. } => self.lessons_key(key),
            Mode::Edit => {
                if key.code == KeyCode::Esc && self.teacher.prof_job.is_some() {
                    self.cancel_prof();
                    return true;
                }
                let Some(what) = edit_key(&key) else { return false };
                match what {
                    EditKey::Generate => self.generate(),
                    EditKey::Develop => self.develop(),
                    EditKey::Undo => self.undo(),
                    EditKey::Redo => self.redo(),
                    EditKey::Explain => self.explain(),
                    EditKey::Ear => {
                        if self.teacher.ear.is_none() {
                            self.teacher.ear = Some(EarSession::new(Level::Intervalles, fresh_seed()));
                        }
                        self.mode = Mode::Ear;
                    }
                    EditKey::Hum => self.open_hum(),
                    EditKey::Lessons => {
                        let selected = browse::move_selection(&browse::rows(Tab::Lessons, ""), 0, 0);
                        self.mode = Mode::Lessons { tab: Tab::Lessons, query: String::new(), selected };
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Work done every frame: the teacher's answer, the end of an example, the HUM take.
    pub(super) fn teacher_tick(&mut self) {
        if let Some(result) = self.teacher.prof_job.as_mut().and_then(prof::Job::poll) {
            self.teacher.prof_job = None;
            let was_explain = std::mem::take(&mut self.teacher.prof_explain);
            match result {
                Ok(text) => {
                    // EXPLIQUE: the offline analysis is already shown, append the AI's reply.
                    if was_explain {
                        if let Some(answer) = &mut self.teacher.prof {
                            answer.lines.push(text);
                            answer.is_error = false;
                        }
                    } else if let Some(answer) = &mut self.teacher.prof {
                        answer.lines = vec![text];
                        answer.is_error = false;
                    }
                }
                Err(e) => {
                    if let Some(answer) = &mut self.teacher.prof {
                        if !was_explain {
                            answer.lines = vec![e.clone()];
                        } else {
                            answer.lines.push(format!("[ prof ] {e}"));
                        }
                        answer.is_error = true;
                    }
                }
            }
            // the popup was closed while waiting: show the answer, unless another popup is in use
            if matches!(self.mode, Mode::Edit) {
                self.mode = Mode::Prof { scroll: 0 };
            }
        }
        if self.teacher.demo.as_ref().is_some_and(|d| d.is_over(Instant::now())) {
            self.finish_demo();
        }
        if let Some(take) = &mut self.teacher.hum {
            take.tick(Instant::now());
        }
    }

    // ---- GÉNÉRER, DÉVELOPPE, undo ----------------------------------------

    /// Replace the whole editor text with `code` from `label`, undoably.
    /// While playing it is heard at the next bar. Returns how to hear it.
    pub(super) fn replace_song(&mut self, label: &str, code: &str) -> &'static str {
        let before = self.editor.text();
        if before != code {
            self.teacher.history.record(label, &before);
        }
        self.editor.replace_text(code);
        self.follow_text()
    }

    /// After a whole-text change: the engine follows while playing (not during an example).
    fn follow_text(&mut self) -> &'static str {
        if self.is_playing() && self.teacher.demo.is_none() {
            self.run();
            " (à la prochaine mesure)"
        } else {
            " — Ctrl+R pour l'entendre"
        }
    }

    fn generate(&mut self) {
        let song = generate::compose(fresh_seed());
        let when = self.replace_song("GÉNÉRER", &song.code);
        self.info(format!("{} — genre : {}{when} · F3 annule", generate::DONE, song.genre));
    }

    fn develop(&mut self) {
        match develop::develop(&self.editor.text(), fresh_seed() % 99 + 1) {
            Ok(developed) => {
                let when = self.replace_song("DÉVELOPPE", &developed.code);
                self.info(format!("{} (F3){when}", developed.message()));
            }
            Err(why) => self.info(why),
        }
    }

    pub(super) fn undo(&mut self) {
        match self.teacher.history.undo(&self.editor.text()) {
            Some(back) => {
                self.editor.replace_text(&back.code);
                self.follow_text();
                self.info(format!("[ ↩ ] annulé : {} — le code d'avant est revenu (Ctrl+Y ou Maj+F3 rétablit)", back.label));
            }
            None => self.error("rien à annuler — F3 annule le dernier VIBE, GÉNÉRER, DÉVELOPPE, HUM ou leçon"),
        }
    }

    fn redo(&mut self) {
        match self.teacher.history.redo(&self.editor.text()) {
            Some(again) => {
                self.editor.replace_text(&again.code);
                self.follow_text();
                self.info(format!("rétabli : {} (F3 pour l'annuler)", again.label));
            }
            None => self.error("rien à rétablir — Maj+F3 ou Ctrl+Y refait ce que F3 a annulé"),
        }
    }

    // ---- EXPLIQUE and ?question --------------------------------------------

    fn explain(&mut self) {
        if self.teacher.prof_job.is_some() {
            self.mode = Mode::Prof { scroll: 0 };
            return;
        }
        self.teacher.prof_explain = true;
        self.teacher.prof_job = Some(prof::explain(&self.editor.text()));
        // The web shows the offline analysis first, then the AI's answer.
        self.teacher.prof = Some(ProfAnswer { title: "EXPLIQUE — ton morceau".into(), lines: prof::analyze(&self.editor.text()), is_error: false, demo: None });
        self.mode = Mode::Prof { scroll: 0 };
    }

    /// The F2 prompt's text: `?question` goes to the teacher. Returns the words for VIBE otherwise.
    pub(super) fn route_prompt(&mut self, input: &str) -> Option<String> {
        match route::route(input) {
            Route::Empty => None,
            Route::Vibe(words) => Some(words),
            Route::Question(question) => {
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
            }
        }
        self.mode = Mode::Prof { scroll: 0 };
    }

    fn cancel_prof(&mut self) {
        if self.teacher.prof_job.take().is_some() {
            if let Some(answer) = &mut self.teacher.prof {
                answer.lines = vec!["question annulée".into()];
            }
            self.info("le prof : annulé");
        }
    }

    fn prof_key(&mut self, key: KeyEvent) -> bool {
        let Mode::Prof { scroll } = &mut self.mode else { return false };
        match key.code {
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Down => *scroll += 1,
            KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
            KeyCode::PageDown => *scroll += 10,
            KeyCode::Home => *scroll = 0,
            KeyCode::End => *scroll = usize::MAX / 2,
            KeyCode::Char('r' | 'R' | ' ') => {
                if let Some(demo) = self.teacher.prof.as_ref().and_then(|a| a.demo.clone()) {
                    self.play_demo(&demo);
                }
            }
            KeyCode::Esc => {
                self.cancel_prof();
                self.mode = Mode::Edit;
            }
            KeyCode::Enter | KeyCode::Char('q' | 'Q') => self.mode = Mode::Edit,
            _ => {}
        }
        true
    }

    // ---- examples through the engine ---------------------------------------

    /// Play a one-bar example instead of the song (see `teacher::demo`).
    fn play_demo(&mut self, demo: &Demo) {
        let Ok(engine) = &self.engine else {
            self.error("pas de sortie audio : impossible de jouer l'exemple");
            return;
        };
        let play = DemoPlay::start(engine, demo, Instant::now(), self.teacher.demo.as_ref());
        self.teacher.demo = Some(play);
    }

    /// The example is over: give the engine the song back, stopped.
    fn finish_demo(&mut self) {
        let Some(play) = self.teacher.demo.take() else { return };
        let code = self.running_text.clone().unwrap_or_else(|| self.editor.text());
        let code = self.engine_code(&code);
        if let Ok(engine) = &self.engine {
            DemoPlay::finish(engine, &code);
        }
        if play.song_was_playing {
            self.info("exemple joué — ton morceau est arrêté, Ctrl+R le relance");
        }
    }

    /// RUN or STOP take the engine back from an example.
    pub(super) fn forget_demo(&mut self) {
        self.teacher.demo = None;
    }

    // ---- OREILLE -------------------------------------------------------------

    fn ear_listen(&mut self) {
        if let Some(demo) = self.teacher.ear.as_ref().map(|ear| ear.exercise().demo()) {
            self.play_demo(&demo);
        }
    }

    fn ear_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(ear) = &mut self.teacher.ear else {
            self.mode = Mode::Edit;
            return true;
        };
        let level_index = LEVELS.iter().position(|(l, _)| *l == ear.level).unwrap_or(0);
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Tab => self.ear_listen(),
            KeyCode::Enter if ear.input.trim().is_empty() => self.ear_listen(),
            KeyCode::Enter => {
                if ear.check().replay {
                    self.ear_listen();
                }
            }
            KeyCode::Down => {
                ear.next(fresh_seed());
                self.ear_listen();
            }
            KeyCode::Left => ear.set_level(LEVELS[(level_index + LEVELS.len() - 1) % LEVELS.len()].0, fresh_seed()),
            KeyCode::Right => ear.set_level(LEVELS[(level_index + 1) % LEVELS.len()].0, fresh_seed()),
            KeyCode::Backspace => {
                ear.input.pop();
            }
            KeyCode::Char('u') if ctrl => ear.input.clear(),
            KeyCode::Char(c) if !ctrl => ear.input.push(c),
            _ => {}
        }
        true
    }

    // ---- HUM -----------------------------------------------------------------

    fn open_hum(&mut self) {
        self.teacher.hum = Some(HumTake::new(MicInput::start(), &self.editor.text(), Instant::now()));
        self.mode = Mode::Hum;
    }

    fn hum_key(&mut self, key: KeyEvent) -> bool {
        let finished = self.teacher.hum.as_ref().map(|take| match take.step(Instant::now()) {
            Step::Heard(_) => 2,
            Step::Silent(_) | Step::NoMic(_) => 1,
            Step::Countdown(_) | Step::Recording(_) => 0,
        });
        match (key.code, finished) {
            (KeyCode::Esc, _) | (_, None) => {
                self.teacher.hum = None;
                self.mode = Mode::Edit;
            }
            (KeyCode::Enter, Some(2)) => {
                let heard = self.teacher.hum.take().and_then(|take| take.result().cloned());
                self.mode = Mode::Edit;
                if let Some(melody) = heard {
                    let when = self.replace_song("HUM", &hum::append(&self.editor.text(), &melody.line));
                    self.info(format!("{} · F3 l'enlève{when}", melody.message()));
                }
            }
            (KeyCode::Char('r' | 'R'), Some(1 | 2)) => self.open_hum(),
            _ => {}
        }
        true
    }

    // ---- LEÇONS ----------------------------------------------------------------

    fn lessons_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Mode::Lessons { tab, query, selected } = &mut self.mode else { return false };
        let mut rows = browse::rows(*tab, query);
        let mut reset = false;
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Edit;
                return true;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                *tab = tab.other();
                reset = true;
            }
            KeyCode::Up => *selected = browse::move_selection(&rows, *selected, -1),
            KeyCode::Down => *selected = browse::move_selection(&rows, *selected, 1),
            KeyCode::PageUp => *selected = browse::move_selection(&rows, *selected, -10),
            KeyCode::PageDown => *selected = browse::move_selection(&rows, *selected, 10),
            KeyCode::Backspace => {
                reset = query.pop().is_some();
            }
            KeyCode::Char('u') if ctrl => {
                query.clear();
                reset = true;
            }
            KeyCode::Char(c) if !ctrl => {
                query.push(c);
                reset = true;
            }
            KeyCode::Enter => {
                if let Some(Row::Lesson(lesson)) = rows.get(*selected).copied() {
                    self.mode = Mode::Edit;
                    self.replace_song("LEÇON", lesson.code);
                    if self.engine.is_ok() {
                        self.run();
                    }
                    self.info(format!("leçon « {} » — F3 rend ton morceau", lesson.title));
                }
                return true;
            }
            _ => {}
        }
        if reset {
            rows = browse::rows(*tab, query);
            *selected = browse::move_selection(&rows, 0, 0);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn editor_keys_for_the_teacher() {
        assert_eq!(edit_key(&key(KeyCode::Char('g'), KeyModifiers::ALT)), Some(EditKey::Generate));
        assert_eq!(edit_key(&key(KeyCode::Char('B'), KeyModifiers::ALT | KeyModifiers::SHIFT)), Some(EditKey::Develop));
        assert_eq!(edit_key(&key(KeyCode::Char('e'), KeyModifiers::ALT)), Some(EditKey::Explain));
        assert_eq!(edit_key(&key(KeyCode::Char('o'), KeyModifiers::ALT)), Some(EditKey::Ear));
        assert_eq!(edit_key(&key(KeyCode::Char('h'), KeyModifiers::ALT)), Some(EditKey::Hum));
        assert_eq!(edit_key(&key(KeyCode::Char('l'), KeyModifiers::ALT)), Some(EditKey::Lessons));
        assert_eq!(edit_key(&key(KeyCode::F(3), KeyModifiers::NONE)), Some(EditKey::Undo));
        assert_eq!(edit_key(&key(KeyCode::F(3), KeyModifiers::SHIFT)), Some(EditKey::Redo));
        assert_eq!(edit_key(&key(KeyCode::F(15), KeyModifiers::NONE)), Some(EditKey::Redo));
        assert_eq!(edit_key(&key(KeyCode::Char('y'), KeyModifiers::CONTROL)), Some(EditKey::Redo));
        // typing stays typing, and the studio keeps its Alt letters
        assert_eq!(edit_key(&key(KeyCode::Char('g'), KeyModifiers::NONE)), None);
        assert_eq!(edit_key(&key(KeyCode::Char('g'), KeyModifiers::CONTROL | KeyModifiers::ALT)), None);
        for studio in ['m', 'a', 'r', 't', 's', 'p', 'i', 'd'] {
            assert_eq!(edit_key(&key(KeyCode::Char(studio), KeyModifiers::ALT)), None, "Alt+{studio} belongs to the studio");
        }
    }
}
