//! OREILLE — ear training: the teacher plays an interval, a chord or a world
//! rhythm, the student types what it was.
//!
//! Port of the browser's `earRound()` / `gradeEar()` / `loreDemo()`
//! (`web/index.html`). Same questions, same answer words, same messages.
//! The web played notes straight into the synth with timers; here each
//! exercise gives a one-bar LOOM program ([`Demo`]) with the same notes and
//! timing, so the UI plays it through the engine. The web had no levels and
//! no score: [`Level`] picks one family (or all, like the web) and [`Score`]
//! counts the answers.

use crate::generate::Rng;
use crate::hum::midi_to_name;
use crate::lessons::normalize;
use crate::prof::lore::LORE;

/// Interval answers (the web's `EAR_INT`).
pub const INTERVALS: [&str; 7] = ["octave", "quinte", "quarte", "tierce", "triton", "seconde", "sixte"];
/// Chord answers (`EAR_CH`).
pub const CHORDS: [&str; 5] = ["maj7", "m7", "dim", "sus4", "7"];
/// Rhythm answers (`EAR_RH`).
pub const RHYTHMS: [&str; 5] = ["tresillo", "son", "bossa", "bembe", "samba"];

/// Which questions to ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Intervalles,
    Accords,
    Rythmes,
    /// Everything mixed, like the web.
    Tout,
}

/// What kind of thing is being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Intervalle,
    Accord,
    Rythme,
}

impl Category {
    /// The accepted answers for this kind of question.
    pub fn choices(self) -> &'static [&'static str] {
        match self {
            Category::Intervalle => &INTERVALS,
            Category::Accord => &CHORDS,
            Category::Rythme => &RHYTHMS,
        }
    }

    /// `l'intervalle`, `l'accord`, `le rythme`.
    pub fn with_article(self) -> &'static str {
        match self {
            Category::Intervalle => "l'intervalle",
            Category::Accord => "l'accord",
            Category::Rythme => "le rythme",
        }
    }

    fn of(answer: &str) -> Category {
        if INTERVALS.contains(&answer) {
            Category::Intervalle
        } else if CHORDS.contains(&answer) {
            Category::Accord
        } else {
            Category::Rythme
        }
    }
}

/// A LOOM program that plays a musical example once: play one bar of `code`,
/// which lasts `seconds`.
#[derive(Debug, Clone, PartialEq)]
pub struct Demo {
    pub code: String,
    pub seconds: f64,
}

/// What the teacher says about an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Right: the exercise is over, ask for a new one.
    Correct(String),
    /// Wrong: play the demo again and let the student retry.
    Wrong(String),
    /// Not one of the answer words (the web then treated the text as a VIBE
    /// request), or the exercise is already solved. Nothing is counted.
    NotAnAnswer(String),
}

impl Verdict {
    pub fn message(&self) -> &str {
        match self {
            Verdict::Correct(m) | Verdict::Wrong(m) | Verdict::NotAnAnswer(m) => m,
        }
    }
}

/// One question: what was played, and how the student is doing on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Exercise {
    answer: &'static str,
    category: Category,
    tries: u32,
    solved: bool,
}

impl Exercise {
    /// A random question from `level`, chosen by `seed`.
    pub fn new(level: Level, seed: u64) -> Exercise {
        let all: Vec<&'static str> = INTERVALS.iter().chain(&CHORDS).chain(&RHYTHMS).copied().collect();
        let pool: &[&'static str] = match level {
            Level::Intervalles => &INTERVALS,
            Level::Accords => &CHORDS,
            Level::Rythmes => &RHYTHMS,
            Level::Tout => &all,
        };
        let answer = *Rng::new(seed).pick(pool);
        Exercise { answer, category: Category::of(answer), tries: 0, solved: false }
    }

    /// The right answer (to reveal it).
    pub fn answer(&self) -> &'static str {
        self.answer
    }

    pub fn category(&self) -> Category {
        self.category
    }

    /// Wrong answers so far.
    pub fn tries(&self) -> u32 {
        self.tries
    }

    pub fn is_solved(&self) -> bool {
        self.solved
    }

    /// What to say when the question starts.
    pub fn prompt(&self) -> String {
        format!(
            "[ 👂 ] devine {} — écris ta réponse ({}) · 👂 reclique = réécoute",
            self.category.with_article(),
            self.category.choices().join("/")
        )
    }

    /// The example to play (again).
    pub fn demo(&self) -> Demo {
        demo(self.answer).expect("every answer has a demo")
    }

    /// Grade a typed answer. Case, accents and surrounding spaces/punctuation don't matter.
    pub fn check(&mut self, typed: &str) -> Verdict {
        if self.solved {
            return Verdict::NotAnAnswer("[ 👂 ] c'est déjà trouvé — reclique 👂 pour un nouvel exercice".into());
        }
        let guess = normalize(typed.trim().trim_matches(|c: char| c.is_whitespace() || matches!(c, '.' | '!' | '«' | '»' | '"')));
        let known = INTERVALS.iter().chain(&CHORDS).chain(&RHYTHMS).any(|a| *a == guess);
        if !known {
            return Verdict::NotAnAnswer(format!("[ 👂 ] réponds par un de ces mots : {}", self.category.choices().join("/")));
        }
        if guess == self.answer {
            self.solved = true;
            let lore = LORE.iter().find(|(k, _)| *k == self.answer).map_or(String::new(), |(_, v)| format!("  ({v})"));
            Verdict::Correct(format!("[ 👂 ✅ ] bravo — c'était bien « {} » !{lore}", self.answer))
        } else {
            self.tries += 1;
            Verdict::Wrong(format!("[ 👂 ❌ ] pas tout à fait — réécoute. (clique ?{} pour la réponse)", self.answer))
        }
    }
}

/// Right and wrong answers over a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Score {
    pub correct: u32,
    pub wrong: u32,
}

impl Score {
    /// Count a verdict (`NotAnAnswer` doesn't count).
    pub fn record(&mut self, verdict: &Verdict) {
        match verdict {
            Verdict::Correct(_) => self.correct += 1,
            Verdict::Wrong(_) => self.wrong += 1,
            Verdict::NotAnAnswer(_) => {}
        }
    }

    /// e.g. `score : 3 trouvés · 2 erreurs`.
    pub fn summary(&self) -> String {
        let s = |n: u32| if n > 1 { "s" } else { "" };
        format!("score : {} trouvé{} · {} erreur{}", self.correct, s(self.correct), self.wrong, s(self.wrong))
    }
}

/// A playable example for a concept, as the web's `loreDemo` (intervals incl.
/// `septieme`, the quiz chords, `ii-v-i`, `cadence`, any named scale) plus the
/// quiz's world rhythms. `None` when there is nothing to play.
pub fn demo(term: &str) -> Option<Demo> {
    let interval = match term {
        "octave" => Some(12),
        "quinte" => Some(7),
        "quarte" => Some(5),
        "tierce" => Some(4),
        "triton" => Some(6),
        "seconde" => Some(2),
        "sixte" => Some(9),
        "septieme" => Some(11),
        _ => None,
    };
    if let Some(iv) = interval {
        return Some(groups(&[vec![60], vec![60 + iv], vec![60, 60 + iv]], 0.42, 0.55));
    }
    let chord: Option<&[i32]> = match term {
        "maj7" => Some(&[0, 4, 7, 11]),
        "m7" => Some(&[0, 3, 7, 10]),
        "dim" => Some(&[0, 3, 6]),
        "sus4" => Some(&[0, 5, 7]),
        "7" => Some(&[0, 4, 7, 10]),
        _ => None,
    };
    if let Some(ch) = chord {
        return Some(groups(&[ch.iter().map(|s| 60 + s).collect()], 0.5, 1.2));
    }
    match term {
        "ii-v-i" => return Some(groups(&[vec![62, 65, 69, 72], vec![67, 71, 74, 77], vec![60, 64, 67, 71]], 0.7, 0.75)),
        "cadence" => return Some(groups(&[vec![67, 71, 74, 77], vec![60, 64, 67, 72]], 0.7, 0.85)),
        _ => {}
    }
    if let Some(pattern) = loom_core::world_rhythm(term).filter(|_| RHYTHMS.contains(&term)) {
        return Some(rhythm(pattern));
    }
    let scale = loom_core::scale_intervals(term)?;
    let notes: Vec<Vec<i32>> = scale.iter().chain(&[12]).map(|x| vec![60 + x]).collect();
    Some(groups(&notes, 0.28, 0.32))
}

/// Chords/notes one after the other, `gap` seconds apart, each held `hold`
/// seconds: one step per group (plus rests so the last one can ring), the
/// notes of a group spread over `;` stacks.
fn groups(groups: &[Vec<i32>], gap: f64, hold: f64) -> Demo {
    let ring = ((hold - gap).max(0.0) / gap).ceil() as usize;
    let steps = groups.len() + ring;
    let layers = groups.iter().map(Vec::len).max().unwrap_or(1);
    let stacks: Vec<String> = (0..layers)
        .map(|k| {
            (0..steps)
                .map(|i| groups.get(i).and_then(|g| g.get(k)).map_or(".".to_string(), |m| midi_to_name(*m)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let code = format!(
        "# 👂 le prof joue (une mesure)\nbpm: {}\nmeter: {steps}\nprof: {} | sine | gate {} | gain 0.3\n",
        short(60.0 / gap),
        stacks.join(" ; "),
        short(hold / gap)
    );
    Demo { code, seconds: steps as f64 * gap }
}

/// A rhythm tapped on a high note over 1.6 s, 90 ms per hit, like the web.
fn rhythm(pattern: &str) -> Demo {
    let steps: Vec<&str> = pattern.split_whitespace().collect();
    let tokens: Vec<&str> = steps.iter().map(|s| if *s == "x" { "c6" } else { "." }).collect();
    let gate = (0.09 / (1.6 / steps.len() as f64)).max(0.05);
    // 150 bpm × 4 beats = a 1.6 s bar
    let code = format!("# 👂 le prof joue (une mesure)\nbpm: 150\nprof: {} | sine | gate {} | gain 0.3\n", tokens.join(" "), short(gate));
    Demo { code, seconds: 1.6 }
}

/// A number with at most two decimals and no trailing zeros.
fn short(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::Program;

    fn midis(demo: &Demo) -> Vec<(f32, Vec<i32>)> {
        let p = Program::parse(&demo.code);
        let mut by_phase: Vec<(f32, Vec<i32>)> = Vec::new();
        let mut events = p.schedule_bar(0);
        events.sort_by(|a, b| a.phase.total_cmp(&b.phase));
        for e in events {
            let m = (69.0 + 12.0 * (e.freq.unwrap() / 440.0).log2()).round() as i32;
            match by_phase.last_mut() {
                Some((ph, ms)) if (*ph - e.phase).abs() < 1e-4 => ms.push(m),
                _ => by_phase.push((e.phase, vec![m])),
            }
        }
        for (_, ms) in &mut by_phase {
            ms.sort();
        }
        by_phase
    }

    #[test]
    fn interval_plays_low_high_then_both() {
        let d = demo("quinte").unwrap();
        let p = Program::parse(&d.code);
        assert!((p.bar_seconds(0) - d.seconds).abs() < 0.01, "{} vs {}", p.bar_seconds(0), d.seconds);
        let got = midis(&d);
        assert_eq!(got.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>(), [vec![60], vec![67], vec![60, 67]]);
        assert!((got[1].0 - 0.25).abs() < 1e-3);
        crate::generate::assert_fully_recognised(&d.code);
    }

    #[test]
    fn chords_rhythms_and_scales_play() {
        assert_eq!(midis(&demo("maj7").unwrap()), [(0.0, vec![60, 64, 67, 71])]);
        assert_eq!(midis(&demo("7").unwrap())[0].1, [60, 64, 67, 70]);
        let t = demo("tresillo").unwrap();
        assert!((Program::parse(&t.code).bar_seconds(0) - 1.6).abs() < 1e-9);
        assert_eq!(midis(&t).iter().map(|(ph, _)| (ph * 8.0).round() as i32).collect::<Vec<_>>(), [0, 3, 6]);
        let ii_v_i = midis(&demo("ii-v-i").unwrap());
        assert_eq!(ii_v_i.len(), 3);
        assert_eq!(midis(&demo("dorian").unwrap()).len(), 8, "7 notes + the octave");
        assert!(demo("banane").is_none());
        for term in INTERVALS.iter().chain(&CHORDS).chain(&RHYTHMS).chain(&["septieme", "cadence", "minpent"]) {
            let d = demo(term).unwrap();
            crate::generate::assert_fully_recognised(&d.code);
            let p = Program::parse(&d.code);
            assert!(!p.schedule_bar(0).is_empty(), "{term}");
            assert!((p.bar_seconds(0) - d.seconds).abs() < 0.02, "{term}");
        }
    }

    #[test]
    fn levels_pick_from_their_family() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..300 {
            assert_eq!(Exercise::new(Level::Intervalles, seed).category(), Category::Intervalle);
            assert_eq!(Exercise::new(Level::Accords, seed).category(), Category::Accord);
            assert_eq!(Exercise::new(Level::Rythmes, seed).category(), Category::Rythme);
            seen.insert(Exercise::new(Level::Tout, seed).answer());
        }
        assert_eq!(seen.len(), 17, "every question comes up");
        assert_eq!(Exercise::new(Level::Tout, 5), Exercise::new(Level::Tout, 5));
    }

    #[test]
    fn grading_like_the_web() {
        let mut ex = Exercise::new(Level::Intervalles, 1);
        let answer = ex.answer();
        let wrong = INTERVALS.iter().find(|a| **a != answer).unwrap();
        assert!(ex.prompt().starts_with("[ 👂 ] devine l'intervalle — écris ta réponse (octave/quinte/"));
        let mut score = Score::default();

        let v = ex.check("banane");
        assert!(matches!(v, Verdict::NotAnAnswer(_)) && v.message().contains("octave/quinte"));
        score.record(&v);
        let v = ex.check(wrong);
        assert_eq!(v, Verdict::Wrong(format!("[ 👂 ❌ ] pas tout à fait — réécoute. (clique ?{answer} pour la réponse)")));
        score.record(&v);
        assert!(ex.check("tresillo").message().contains("❌"), "another family's word is just wrong");
        let v = ex.check(&format!("  {} ! ", answer.to_uppercase()));
        assert!(matches!(v, Verdict::Correct(_)), "{v:?}");
        assert!(v.message().starts_with(&format!("[ 👂 ✅ ] bravo — c'était bien « {answer} » !")));
        score.record(&v);
        assert!(ex.is_solved() && ex.tries() == 2);
        assert!(matches!(ex.check(answer), Verdict::NotAnAnswer(_)));
        assert_eq!(score, Score { correct: 1, wrong: 1 });
        assert_eq!(score.summary(), "score : 1 trouvé · 1 erreur");
    }

    #[test]
    fn accents_and_lore() {
        let mut ex = Exercise { answer: "quinte", category: Category::Intervalle, tries: 0, solved: false };
        assert!(ex.check("Quinté").message().contains("ratio 3:2"), "accents ignored and the lore is quoted");
        let mut ex = Exercise { answer: "bembe", category: Category::Rythme, tries: 0, solved: false };
        assert!(matches!(ex.check("bembé"), Verdict::Correct(_)));
        assert_eq!(ex.demo(), demo("bembe").unwrap());
    }
}
