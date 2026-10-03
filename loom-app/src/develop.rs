//! DÉVELOPPE — develop the whole song offline, in one click, undoable.
//!
//! Port of the browser's `developAll()` (the `developBtn` handler in
//! `web/index.html`). Line by line:
//! - a melodic voice that doesn't vary yet gets `| develop` (a motivic arc:
//!   rotation, sequence, retrograde, inversion — a different one each bar),
//! - a drum voice that doesn't vary yet gets `| every 4 rot 3` (a fill every 4 bars),
//! - the `seed:` goes up by one (or a `seed:` line is added), so every seeded
//!   dice (`?`, `mutate`, `walk`…) rolls differently.
//!
//! Settings, comments and blank lines are kept exactly. Three small fixes over
//! the web: the verb is inserted *before* an end-of-line comment (the web put
//! it inside the comment), and `def` macros and `morph:` are left alone (the
//! web would append a verb to them and break the song).

use crate::vibe::{is_setting, strip_comment};

/// The drum names the web engine knows (its `DRUMS` set).
const DRUMS: [&str; 13] = ["kick", "bd", "snare", "sd", "hat", "hh", "clap", "cp", "tom", "cowbell", "cow", "rim", "rimshot"];
/// Verbs that already make a line evolve: such a line is left as it is.
const VARYING_VERBS: [&str; 7] = ["develop", "mutate", "shuffle", "every", "iter", "markov", "stutter"];
/// Generators that already make a line evolve.
const VARYING_GENERATORS: [&str; 11] = ["walk", "drunk", "ca", "logistic", "perlin", "lsys", "l-sys", "xys", "evolve", "recaman", "cantor"];

/// Added to melodic voices.
pub const MELODY_VERB: &str = "develop";
/// Added to drum voices.
pub const DRUM_VERB: &str = "every 4 rot 3";

/// Said when the editor is empty.
pub const EMPTY: &str = "[ ✨ ] écris ou charge d'abord un morceau";
/// Said when every voice already varies.
pub const NOTHING_TO_DO: &str = "[ ✨ ] tout varie déjà — rien à développer (essaie un seed différent)";

/// The developed song and what was done.
#[derive(Debug, Clone, PartialEq)]
pub struct Developed {
    pub code: String,
    /// Melodic voices that got `| develop`.
    pub melodies: usize,
    /// Drum voices that got `| every 4 rot 3`.
    pub drums: usize,
}

impl Developed {
    /// The web's log line: `[ ✨ ] développé : 2 mélodie(s) + 3 batterie(s) · seed changé — ↩ annuler`.
    pub fn message(&self) -> String {
        format!("[ ✨ ] développé : {} mélodie(s) + {} batterie(s) · seed changé — ↩ annuler", self.melodies, self.drums)
    }
}

/// Develop `src`; the song unchanged if there is nothing to develop.
/// `seed` is only used when the song has no `seed:` line yet (the web always
/// wrote `seed: 7`); an existing seed goes up by one.
pub fn song(src: &str, seed: u64) -> String {
    develop(src, seed).map_or_else(|_| src.to_string(), |d| d.code)
}

/// Develop `src`, or say why nothing changes ([`EMPTY`], [`NOTHING_TO_DO`]).
pub fn develop(src: &str, seed: u64) -> Result<Developed, String> {
    if src.trim().is_empty() {
        return Err(EMPTY.into());
    }
    let (mut melodies, mut drums) = (0, 0);
    let lines: Vec<String> = src
        .split('\n')
        .map(|line| match classify(line) {
            Kind::Melody => {
                melodies += 1;
                add_verb(line, MELODY_VERB)
            }
            Kind::Drum => {
                drums += 1;
                add_verb(line, DRUM_VERB)
            }
            Kind::Keep => line.to_string(),
        })
        .collect();
    if melodies + drums == 0 {
        return Err(NOTHING_TO_DO.into());
    }
    Ok(Developed { code: bump_seed(&lines.join("\n"), seed), melodies, drums })
}

enum Kind {
    Melody,
    Drum,
    Keep,
}

fn classify(line: &str) -> Kind {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return Kind::Keep;
    }
    let code = strip_comment(t);
    let Some((label, rest)) = code.split_once(':') else { return Kind::Keep };
    let label = label.trim().to_lowercase();
    let head = label.split('@').next().unwrap_or("");
    if is_setting(head) || label.starts_with("def ") || varies(code) {
        return Kind::Keep;
    }
    let pattern = rest.split('|').next().unwrap_or("");
    if DRUMS.contains(&head) {
        Kind::Drum
    } else if looks_melodic(pattern) {
        Kind::Melody
    } else {
        Kind::Keep
    }
}

/// Already evolving: a varying verb after a `|`, or a generator call like `walk(16)`.
fn varies(code: &str) -> bool {
    let verb = code.split('|').skip(1).any(|seg| seg.split_whitespace().next().is_some_and(|w| VARYING_VERBS.contains(&w)));
    verb || VARYING_GENERATORS.iter().any(|g| calls(code, g))
}

/// `name(` or `name (` as a whole word.
fn calls(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(i, _)| {
        let before = code[..i].chars().next_back();
        let word_start = !before.is_some_and(|c| c.is_alphanumeric() || c == '_');
        word_start && code[i + name.len()..].trim_start().starts_with('(')
    })
}

/// The web's test: a degree 1-7, a `^` chord tone, a note like `eb3`, or a chord like `Am`/`C7`.
fn looks_melodic(pattern: &str) -> bool {
    let c: Vec<char> = pattern.chars().collect();
    c.iter().enumerate().any(|(i, &ch)| {
        let next = c.get(i + 1).copied().unwrap_or(' ');
        let after_acc = if matches!(next, '#' | 'b') { c.get(i + 2).copied().unwrap_or(' ') } else { next };
        ('1'..='7').contains(&ch)
            || ch == '^'
            || (matches!(ch.to_ascii_lowercase(), 'a'..='g') && after_acc.is_ascii_digit())
            || (matches!(ch, 'A'..='G') && (matches!(next, '#' | 'm' | 'b') || next.is_ascii_digit()))
    })
}

/// `line | verb`, keeping any end-of-line comment after the verb.
fn add_verb(line: &str, verb: &str) -> String {
    let code = strip_comment(line);
    let comment = &line[code.len()..];
    let code = code.trim_end();
    if comment.is_empty() {
        format!("{code} | {verb}")
    } else {
        format!("{code} | {verb}  {comment}")
    }
}

/// The first `seed: N` becomes `seed: N+1`; with no seed line, `seed: <seed>` goes on top.
fn bump_seed(src: &str, seed: u64) -> String {
    let mut lines: Vec<String> = src.split('\n').map(str::to_string).collect();
    let is_seed = |l: &str| l.trim_start().starts_with("seed:");
    match lines.iter().position(|l| is_seed(l)) {
        Some(i) => {
            let line = &lines[i];
            let colon = line.find(':').expect("seed line has a colon") + 1;
            let after = &line[colon..];
            let start = colon + (after.len() - after.trim_start().len());
            let digits = line[start..].chars().take_while(char::is_ascii_digit).count();
            if let Ok(n) = line[start..start + digits].parse::<u64>() {
                lines[i] = format!("{}{}{}", &line[..start], n.saturating_add(1), &line[start + digits..]);
            }
            lines.join("\n")
        }
        None => format!("seed: {seed}\n{src}"),
    }
}

/// One-slot undo for big actions (develop, generate, AI answers), like the web's `↩`.
#[derive(Debug, Clone, Default)]
pub struct Undo(Option<String>);

impl Undo {
    /// Said after undoing.
    pub const DONE: &'static str = "[ ↩ ] action annulée";

    /// Remember the code before replacing it (forgets any older one).
    pub fn push(&mut self, code: &str) {
        self.0 = Some(code.to_string());
    }

    /// The code to restore, once (the button then hides).
    pub fn take(&mut self) -> Option<String> {
        self.0.take()
    }

    /// Whether the `↩` button should show.
    pub fn available(&self) -> bool {
        self.0.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laisse_les_lignes_vis() {
        let src = "vis: osc 14 0.05 0.9 | kaleid 6\nvis: shape 6 0.2\nlead: c4 e4 g4 c5\n";
        let d = develop(src, 1).unwrap();
        assert!(d.code.contains("vis: osc 14 0.05 0.9 | kaleid 6\n") && d.code.contains("vis: shape 6 0.2\n"), "{}", d.code);
        assert_eq!(loom_core::Program::parse(&d.code).vis.layers.len(), 2);
    }

    const SONG: &str = "# mon morceau — ne pas toucher\n\
title: Essai\n\
bpm: 120\n\
key: a minor\n\
prog: Am F C G\n\
seed: 41\n\
\n\
kick: x . . . x . . . | sample bd\n\
hat:  x x x x | every 2 fast 2\n\
bass: 1 . 5, . | saw | oct 2  # la basse\n\
pad:  ^ . . . | sine\n\
arp:  walk(8) | pluck\n\
perc: x . x . | sample perc\n\
def riff: 1 2 3\n\
lead@drop: $riff 5 | tri\n";

    #[test]
    fn develops_melodies_and_drums_and_bumps_the_seed() {
        let d = develop(SONG, 3).unwrap();
        assert_eq!((d.melodies, d.drums), (3, 1), "{}", d.code);
        let lines: Vec<&str> = d.code.lines().collect();
        assert_eq!(lines[7], "kick: x . . . x . . . | sample bd | every 4 rot 3");
        assert_eq!(lines[8], "hat:  x x x x | every 2 fast 2", "already varies");
        assert_eq!(lines[9], "bass: 1 . 5, . | saw | oct 2 | develop  # la basse", "verb before the comment");
        assert_eq!(lines[10], "pad:  ^ . . . | sine | develop");
        assert_eq!(lines[11], "arp:  walk(8) | pluck", "a generator already varies");
        assert_eq!(lines[12], "perc: x . x . | sample perc", "not a drum name, not melodic");
        assert_eq!(lines[13], "def riff: 1 2 3", "macros untouched");
        assert_eq!(lines[14], "lead@drop: $riff 5 | tri | develop");
        assert_eq!(lines[5], "seed: 42");
        assert_eq!(d.message(), "[ ✨ ] développé : 3 mélodie(s) + 1 batterie(s) · seed changé — ↩ annuler");
    }

    #[test]
    fn keeps_comments_settings_and_parses() {
        let out = song(SONG, 3);
        for (a, b) in SONG.lines().zip(out.lines()) {
            let t = a.trim();
            if t.is_empty() || t.starts_with('#') || t.split(':').next().is_some_and(|h| is_setting(h.trim()) && h.trim() != "seed") {
                assert_eq!(a, b);
            }
        }
        assert_eq!(SONG.lines().count(), out.lines().count());
        let (before, after) = (loom_core::Program::parse(SONG), loom_core::Program::parse(&out));
        assert_eq!(before.voices.len(), after.voices.len());
        assert_eq!((after.seed, after.key.is_some(), after.prog.len()), (42, true, 4));
        crate::generate::assert_fully_recognised(&out);
        // something really changes in the music: bar 1 of the bass differs
        let bass = |p: &loom_core::Program| p.voices.iter().find(|v| v.name == "bass").map(|v| loom_core::effective(v, 1)).unwrap();
        assert_ne!(bass(&before), bass(&after));
    }

    #[test]
    fn develops_generated_songs() {
        for seed in 0..50 {
            let src = crate::generate::song(seed);
            let out = song(&src, seed);
            assert_ne!(out, src, "seed {seed}");
            crate::generate::assert_fully_recognised(&out);
            assert_eq!(loom_core::Program::parse(&out).seed, loom_core::Program::parse(&src).seed + 1);
        }
    }

    #[test]
    fn adds_a_seed_when_missing_and_stops_when_all_varies() {
        let d = develop("lead: 1 2 3 5\n", 9).unwrap();
        assert_eq!(d.code, "seed: 9\nlead: 1 2 3 5 | develop\n");
        // developing twice: the second time only the seed would change, so nothing happens
        assert_eq!(develop(&d.code, 9).unwrap_err(), NOTHING_TO_DO);
        assert_eq!(song(&d.code, 9), d.code);
        assert_eq!(bump_seed("bpm: 90\n  seed: 41 # le hasard\nseed: 3", 0), "bpm: 90\n  seed: 42 # le hasard\nseed: 3");
        assert_eq!(develop("  \n", 1).unwrap_err(), EMPTY);
        assert_eq!(develop("# rien\nbpm: 90\n", 1).unwrap_err(), NOTHING_TO_DO);
    }

    #[test]
    fn melodic_detection_matches_the_web() {
        for yes in ["1 . 3", "^ . . .", "c3 eb2", "Am . F .", "C7", "G#m", ". x?70"] {
            assert!(looks_melodic(yes), "{yes}");
        }
        for no in ["x . x .", "e(x)", ". . .", "tresillo"] {
            assert!(!looks_melodic(no), "{no}");
        }
        assert!(calls("hat: ca (90,16)", "ca") && !calls("hat: pica(3)", "ca") && !calls("cascade", "ca"));
    }

    #[test]
    fn undo_holds_one_version() {
        let mut undo = Undo::default();
        assert!(!undo.available());
        undo.push("a");
        undo.push("b");
        assert!(undo.available());
        assert_eq!(undo.take().as_deref(), Some("b"));
        assert_eq!(undo.take(), None);
    }
}
