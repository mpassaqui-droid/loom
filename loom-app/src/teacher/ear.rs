//! OREILLE — the ear-training panel's state: the level, the current
//! question, what the student typed, the teacher's verdict and the score.
//!
//! The questions and the grading are `crate::ear`; this adds what a panel
//! needs: a « Réponse » button (the web's hint `(clique ?quinte pour la
//! réponse)` gave the answer away, so it is replaced), a colour for each
//! verdict, and « a wrong answer replays the example ».

use crate::ear::{Exercise, Level, Score, Verdict};

/// The levels, in the order the panel shows them, with their French names.
pub const LEVELS: [(Level, &str); 4] =
    [(Level::Intervalles, "Intervalles"), (Level::Accords, "Accords"), (Level::Rythmes, "Rythmes"), (Level::Tout, "Tout")];

/// The French name of a level.
pub fn level_name(level: Level) -> &'static str {
    LEVELS.iter().find(|(l, _)| *l == level).map_or("Tout", |(_, name)| name)
}

/// How a verdict is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Correct (green / accent).
    Good,
    /// Wrong (red).
    Bad,
    /// Not an answer (dim).
    Neutral,
}

/// What the student typed is checked: play the example again?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    pub replay: bool,
}

/// The panel's state.
#[derive(Debug, Clone)]
pub struct EarSession {
    pub level: Level,
    pub score: Score,
    /// The answer field.
    pub input: String,
    exercise: Exercise,
    verdict: Option<Verdict>,
    revealed: bool,
}

impl EarSession {
    pub fn new(level: Level, seed: u64) -> EarSession {
        EarSession {
            level,
            score: Score::default(),
            input: String::new(),
            exercise: Exercise::new(level, seed),
            verdict: None,
            revealed: false,
        }
    }

    pub fn exercise(&self) -> &Exercise {
        &self.exercise
    }

    /// Another level: a new question from it (the score is kept).
    pub fn set_level(&mut self, level: Level, seed: u64) {
        self.level = level;
        self.next(seed);
    }

    /// « Suivant »: a new question.
    pub fn next(&mut self, seed: u64) {
        self.exercise = Exercise::new(self.level, seed);
        self.verdict = None;
        self.revealed = false;
        self.input.clear();
    }

    /// Check the answer field. `?` alone reveals the answer, like the web's `?quinte`.
    /// Once the answer was revealed, answers are still graded but not counted.
    pub fn check(&mut self) -> Checked {
        let typed = self.input.trim().to_string();
        if typed == "?" {
            self.input.clear();
            self.reveal();
            return Checked { replay: false };
        }
        let verdict = self.exercise.check(&typed);
        if !self.revealed {
            self.score.record(&verdict);
        }
        let replay = matches!(verdict, Verdict::Wrong(_));
        if !matches!(verdict, Verdict::NotAnAnswer(_)) {
            self.input.clear(); // a real answer was read; a typo stays to be fixed
        }
        self.verdict = Some(verdict);
        Checked { replay }
    }

    /// « Réponse »: show the answer.
    pub fn reveal(&mut self) {
        self.revealed = true;
    }

    /// The answer, once revealed.
    pub fn revealed(&self) -> Option<&'static str> {
        self.revealed.then(|| self.exercise.answer())
    }

    /// The question, without the web's « reclique » button hint.
    pub fn prompt(&self) -> String {
        let prompt = self.exercise.prompt();
        match prompt.find(" · 👂") {
            Some(i) => prompt[..i].to_string(),
            None => prompt,
        }
    }

    /// The last verdict, ready to show, and its colour.
    pub fn verdict(&self) -> Option<(String, Tone)> {
        let verdict = self.verdict.as_ref()?;
        let tone = match verdict {
            Verdict::Correct(_) => Tone::Good,
            Verdict::Wrong(_) => Tone::Bad,
            Verdict::NotAnAnswer(_) => Tone::Neutral,
        };
        Some((without_spoiler(verdict.message()), tone))
    }

    /// `score : 3 trouvés · 1 erreur · essais sur cette question : 2`.
    pub fn progress(&self) -> String {
        format!("{} · essais ratés sur cette question : {}", self.score.summary(), self.exercise.tries())
    }
}

/// The wrong-answer message without `(clique ?quinte pour la réponse)`.
pub fn without_spoiler(message: &str) -> String {
    match message.find(" (clique ?") {
        Some(start) => {
            let end = message[start..].find(')').map_or(message.len(), |i| start + i + 1);
            format!("{}{} — tu peux demander la réponse", &message[..start], &message[end..])
        }
        None => message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrong_answer(session: &EarSession) -> &'static str {
        let ex = session.exercise();
        ex.category().choices().iter().copied().find(|c| *c != ex.answer()).unwrap()
    }

    #[test]
    fn wrong_replays_right_counts_and_the_answer_is_not_spoiled() {
        let mut s = EarSession::new(Level::Intervalles, 3);
        assert!(s.prompt().starts_with("[ 👂 ] devine l'intervalle"));
        assert!(!s.prompt().contains("reclique"));
        s.input = wrong_answer(&s).to_uppercase();
        assert_eq!(s.check(), Checked { replay: true });
        let (text, tone) = s.verdict().unwrap();
        assert_eq!(tone, Tone::Bad);
        assert!(!text.contains(s.exercise().answer()), "spoiler: {text}");
        assert!(text.contains("demander la réponse"), "{text}");
        assert_eq!((s.score.wrong, s.exercise().tries()), (1, 1));
        assert!(s.input.is_empty());

        s.input = format!(" {} ", s.exercise().answer());
        assert_eq!(s.check(), Checked { replay: false });
        assert_eq!(s.verdict().unwrap().1, Tone::Good);
        assert_eq!((s.score.correct, s.score.wrong), (1, 1));
        assert!(s.progress().starts_with("score : 1 trouvé · 1 erreur"), "{}", s.progress());
    }

    #[test]
    fn not_an_answer_keeps_the_text_and_counts_nothing() {
        let mut s = EarSession::new(Level::Accords, 1);
        s.input = "bof".into();
        assert_eq!(s.check(), Checked { replay: false });
        assert_eq!(s.verdict().unwrap().1, Tone::Neutral);
        assert_eq!(s.input, "bof");
        assert_eq!(s.score, Score::default());
    }

    #[test]
    fn reveal_then_next_and_levels() {
        let mut s = EarSession::new(Level::Rythmes, 5);
        assert_eq!(s.revealed(), None);
        s.input = "?".into();
        s.check();
        let answer = s.revealed().unwrap();
        assert!(crate::ear::RHYTHMS.contains(&answer));
        // answering after seeing the answer is graded but not counted
        s.input = answer.into();
        s.check();
        assert_eq!(s.verdict().unwrap().1, Tone::Good);
        assert_eq!(s.score.correct, 0);

        s.set_level(Level::Accords, 9);
        assert_eq!((s.level, s.revealed(), s.verdict()), (Level::Accords, None, None));
        assert!(crate::ear::CHORDS.contains(&s.exercise().answer()));
        assert_eq!(level_name(Level::Tout), "Tout");
    }

    #[test]
    fn spoiler_removal() {
        assert_eq!(
            without_spoiler("[ 👂 ❌ ] pas tout à fait — réécoute. (clique ?quinte pour la réponse)"),
            "[ 👂 ❌ ] pas tout à fait — réécoute. — tu peux demander la réponse"
        );
        assert_eq!(without_spoiler("bravo"), "bravo");
    }
}
