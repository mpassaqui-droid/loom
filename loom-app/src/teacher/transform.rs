//! TRANSFORME — the teacher *explains AND changes* the piece, offline.
//!
//! The prof looks at the code, picks one idea ("plus sombre", "transposer la
//! mélodie", "épaissir"…), applies it to a few lines and then says out loud
//! what changed and why — so the writer hears the difference, not just reads
//! it. Each transform is small, keeps the comments and the settings, and the
//! result must re-parse. The one-slot undo brings the original back.
//!
//! No API needed: everything here is grammar + ear-friendly defaults.

use crate::vibe::{is_setting, strip_comment};

/// The drums the web engine knows (same list as `develop`).
const DRUMS: [&str; 13] = ["kick", "bd", "snare", "sd", "hat", "hh", "clap", "cp", "tom", "cowbell", "cow", "rim", "rimshot"];
/// Verbs that already do the job: such a line is left alone (no double dose).
const WANTS_SWING: [&str; 2] = ["swing", "groove"];
const WANTS_DENSE: [&str; 3] = ["unison", "spread", "thick"];
const WANTS_AIR: [&str; 2] = ["reverb", "room"];
const WANTS_TRANSPOSE: [&str; 2] = ["semi", "transpose"];

/// One way to change the music, understandable by ear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Transpose the melodies by `n` semitones (positive = brighter, negative = deeper).
    Transpose(i32),
    /// Rewrite the key's mode towards the dark (major→minor, minor→phrygian…).
    Dark,
    /// Rewrite the key's mode towards the light (minor→major, dorian→mixolydian…).
    Bright,
    /// Fatten the melodic voices (`| unison 3`).
    Dense,
    /// Put air around the notes (`| reverb 0.4`).
    Open,
    /// Make the drums groove (`| swing 0.25` on the percussion).
    Groove,
    /// Add variation through the existing `develop` engine.
    Develop,
    /// Jazz reharmonization of `prog:` (each dominant prepared by its ii, then tritone-substituted).
    Jazz,
    /// Tritone substitution of every dominant seventh in `prog:`.
    Tritone,
}

/// The chips every app shows, in this order.
pub const CHIPS: [Kind; 9] = [Kind::Transpose(1), Kind::Transpose(-1), Kind::Dark, Kind::Bright, Kind::Dense, Kind::Open, Kind::Groove, Kind::Jazz, Kind::Tritone];

impl Kind {
    /// The short label shown on the chip.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Transpose(1) => "plus aigu",
            Kind::Transpose(-1) => "plus grave",
            Kind::Transpose(_) => "transposer",
            Kind::Dark => "plus sombre",
            Kind::Bright => "plus clair",
            Kind::Dense => "plus dense",
            Kind::Open => "plus d'air",
            Kind::Groove => "groove",
            Kind::Develop => "varier",
            Kind::Jazz => "réharmo jazz",
            Kind::Tritone => "substitution triton",
        }
    }

    /// The prof's explanation, said once the transform is applied.
    pub fn explanation(self) -> String {
        match self {
            Kind::Transpose(n) if n > 0 => format!("j'ai transposé les mélodies {n} demi-ton(s) plus haut — écoute la lumière gagner sur les notes tenues."),
            Kind::Transpose(n) if n < 0 => format!("j'ai transposé les mélodies {} demi-ton(s) plus bas — ça assombrit et creuse l'ambiance.", -n),
            Kind::Transpose(_) => "transposer d'un demi-ton ne change pas le caractère, juste la tessiture (plus commode pour chanter ou jouer).".into(),
            Kind::Dark => "j'ai basculé le mode vers le sombre (ex. majeur → mineur) — la tierce baissée change toute la couleur émotionnelle du morceau.".into(),
            Kind::Bright => "j'ai basculé le mode vers le clair (ex. mineur → majeur) — la tierce relevée ouvre le son vers la lumière.".into(),
            Kind::Dense => "j'ai épaissi les voix (| unison 3) — des copies légèrement désaccordées remplissent le spectre, comme plusieurs violons au lieu d'un.".into(),
            Kind::Open => "j'ai ouvert l'espace (| reverb 0.4) — chaque note respire dans une pièce plus grande, le rythme se dilate.".into(),
            Kind::Groove => "j'ai mis du swing (| swing 0.25) sur les percussions — les contretemps glissent un peu, le rythme « balance » comme joué par un humain.".into(),
            Kind::Jazz => "j'ai réharmonisé prog: à la jazz : chaque dominante est préparée par son ii, puis remplacée par sa substitution de triton (G7 devient Dm7 Db7). La basse descend par demi-tons vers la tonique.".into(),
            Kind::Tritone => "j'ai remplacé chaque accord de dominante par celui qui est à un triton (G7 devient Db7) : ils partagent le même triton (si et fa), la tension reste, la basse glisse d'un demi-ton vers la tonique.".into(),
            Kind::Develop => "j'ai développé les voix qui étaient figées (| develop) et changé le seed — les motifs bougent d'une mesure à l'autre.".into(),
        }
    }
}

/// The transformed song and the prof's account of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Transformed {
    pub code: String,
    /// Short human lines, one per thing changed ("bass · | semi -2").
    pub touches: Vec<String>,
}

impl Transformed {
    /// A one-line summary for the status bar.
    pub fn summary(&self) -> String {
        let first = self.touches.first().map_or("rien", |t| t);
        format!("[ prof ] transformé : {} ligne(s) — {first} · ↩ pour annuler", self.touches.len())
    }
}

/// Apply one [`Kind`] to `src`. Says why nothing happened when it can't.
pub fn transform(src: &str, kind: Kind, seed: u64) -> Result<Transformed, String> {
    if src.trim().is_empty() {
        return Err("écris ou charge d'abord un morceau".into());
    }
    match kind {
        Kind::Develop => {
            let d = crate::develop::develop(src, seed)?;
            Ok(Transformed { code: d.code, touches: vec!["les voix figées varient maintenant (| develop) + seed changé".into()] })
        }
        Kind::Dark | Kind::Bright => dark_bright(src, kind),
        Kind::Transpose(n) => add_verb_to_voices(src, &format!("semi {n}"), &WANTS_TRANSPOSE, false),
        Kind::Dense => add_verb_to_voices(src, "unison 3", &WANTS_DENSE, false),
        Kind::Open => add_verb_to_voices(src, "reverb 0.4", &WANTS_AIR, false),
        Kind::Groove => add_verb_to_voices(src, "swing 0.25", &WANTS_SWING, true),
        Kind::Jazz | Kind::Tritone => {
            use loom_core::harmony::{reharm_source, Reharm};
            let how = if kind == Kind::Jazz { Reharm::Jazz } else { Reharm::Tritone };
            let code = reharm_source(src, how)?;
            let line = code.lines().find(|l| l.trim_start().to_lowercase().starts_with("prog")).unwrap_or("").trim().to_string();
            Ok(Transformed { code, touches: vec![line] })
        }
    }
}

/// True when the line's pattern is a melody (1-7, `^`, note name, chord).
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

/// Append `verb` to the informative voices (drums for a groove, melodies
/// otherwise), skipping settings/macros and lines that already use the family.
fn add_verb_to_voices(src: &str, verb: &str, family: &[&str], only_drums: bool) -> Result<Transformed, String> {
    let mut touches = Vec::new();
    let lines: Vec<String> = src
        .split('\n')
        .map(|line| {
            let code = strip_comment(line);
            let Some((label, rest)) = code.split_once(':') else { return line.to_string() };
            let head = label.trim().split('@').next().unwrap_or("").to_lowercase();
            if is_setting(&head) || head.starts_with("def ") {
                return line.to_string();
            }
            let is_drum = DRUMS.contains(&head.as_str());
            if only_drums {
                if !is_drum {
                    return line.to_string();
                }
            } else if !looks_melodic(rest.split('|').next().unwrap_or("")) {
                return line.to_string();
            }
            let already = |v: &str| code.split('|').skip(1).any(|seg| {
                let s = seg.trim_start();
                s == v || s.starts_with(&format!("{v} "))
            });
            if family.iter().any(|v| already(v)) {
                return line.to_string();
            }
            touches.push(format!("{head} · | {verb}"));
            append_verb(line, verb)
        })
        .collect();
    if touches.is_empty() {
        let why = if only_drums { "aucune percussions à faire balancer" } else { "déjà fait (ou aucune voix mélodique du bon type)" };
        return Err(format!("rien à transformer ici — {why}. Essaie une autre transformée."));
    }
    Ok(Transformed { code: lines.join("\n"), touches })
}

/// `line | verb`, keeping any end-of-line comment after the verb.
fn append_verb(line: &str, verb: &str) -> String {
    let code = strip_comment(line);
    let comment = &line[code.len()..];
    let code = code.trim_end();
    if comment.is_empty() {
        format!("{code} | {verb}")
    } else {
        format!("{code} | {verb}  {comment}")
    }
}

/// One step of the darkness ladder, light → dark.
const DARKER: [&str; 7] = ["lydian", "major", "mixolydian", "dorian", "minor", "phrygian", "locrian"];

/// The neighbouring mode towards dark or light, or `None` for an unknown mode
/// (or one already at the end of the ladder).
fn next_mode(mode: &str, darker: bool) -> Option<&'static str> {
    let i = DARKER.iter().position(|m| *m == mode)?;
    if darker {
        DARKER.get(i + 1).copied()
    } else {
        (i > 0).then(|| DARKER[i - 1])
    }
}

/// Move the key's mode one step towards dark or light. Only touches `key:` /
/// `scale:` lines whose mode it knows; unknown keys are left alone.
fn dark_bright(src: &str, kind: Kind) -> Result<Transformed, String> {
    let darker = kind == Kind::Dark;
    let mut touches = Vec::new();
    let lines: Vec<String> = src
        .split('\n')
        .map(|line| {
            let code = strip_comment(line);
            let Some((label, rest)) = code.split_once(':') else { return line.to_string() };
            let head = label.trim().to_lowercase();
            if !matches!(head.as_str(), "key" | "scale") {
                return line.to_string();
            }
            let Some(mode) = rest.split_whitespace().last().map(|m| m.to_lowercase()) else {
                return line.to_string();
            };
            let Some(next) = next_mode(&mode, darker) else { return line.to_string() };
            let comment = &line[code.len()..];
            let before = code.trim_end();
            // replace the last occurrence of the mode word (ASCII: same byte length)
            let Some(rel) = before.to_lowercase().rfind(&mode) else { return line.to_string() };
            let mut changed = format!("{}{}{}", &before[..rel], next, &before[rel + mode.len()..]);
            if !comment.is_empty() {
                changed.push_str("  ");
                changed.push_str(comment);
            }
            touches.push(format!("key: {mode} → {next}"));
            changed
        })
        .collect();
    if touches.is_empty() {
        return Err("je ne reconnais pas ce mode — change le `key:` selon l'humeur (ex. `minor` = plus triste, `phrygian` = plus sombre, `major` = clair)".into());
    }
    Ok(Transformed { code: lines.join("\n"), touches })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONG: &str = "# mon morceau\n\
title: Essai\n\
bpm: 120\n\
key: a minor\n\
seed: 41\n\
\n\
kick: x . . . x . . . | sample bd\n\
hat:  x x x x  # commentaire de la hat\n\
bass: 1 . 5, . | saw | oct 2\n\
lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18\n";

    #[test]
    fn transpose_moves_melodies_up_and_stays_parseable() {
        let t = transform(SONG, Kind::Transpose(2), 0).unwrap();
        let lines: Vec<&str> = t.code.lines().collect();
        assert_eq!(lines[6], "kick: x . . . x . . . | sample bd", "drums untouched");
        assert_eq!(lines[7], "hat:  x x x x  # commentaire de la hat", "hat untouched");
        assert_eq!(lines[8], "bass: 1 . 5, . | saw | oct 2 | semi 2");
        assert_eq!(lines[9], "lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18 | semi 2");
        assert!(t.summary().contains("2 ligne"), "{}", t.summary());
        let p = loom_core::Program::parse(&t.code);
        assert_eq!(p.voices.len(), 4);
    }

    #[test]
    fn dark_and_bright_walk_the_ladder() {
        let d = transform(SONG, Kind::Dark, 0).unwrap();
        assert_eq!(d.touches, ["key: minor → phrygian"]);
        assert!(d.code.lines().any(|l| l.starts_with("key: a phrygian")));
        // back to minor by going brighter one step
        let b = transform(&d.code, Kind::Bright, 0).unwrap();
        assert!(b.code.lines().any(|l| l.starts_with("key: a minor")));
        assert_eq!(next_mode("minor", true), Some("phrygian"));
        assert_eq!(next_mode("lydian", true), Some("major"));
        assert_eq!(next_mode("locrian", true), None, "already the darkest");
        assert_eq!(next_mode("major", false), Some("lydian"));
        assert_eq!(next_mode("lydian", false), None, "already the lightest");
        assert_eq!(next_mode("myxolidian", true), None, "unknown mode");
        // no key: polite refusal
        assert!(transform("lead: 1 2 3\n", Kind::Dark, 0).is_err());
    }

    #[test]
    fn dense_open_groove_hit_the_right_voices() {
        let d = transform(SONG, Kind::Dense, 0).unwrap();
        assert!(d.code.contains("bass: 1 . 5, . | saw | oct 2 | unison 3"));
        assert!(d.code.contains("lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18 | unison 3"));
        assert!(d.code.contains("kick: x . . . x . . . | sample bd"), "drums stay put");
        // the second time the family is already there: refuse politely
        assert!(transform(&d.code, Kind::Dense, 0).is_err());

        let g = transform(SONG, Kind::Groove, 0).unwrap();
        assert!(g.code.contains("kick: x . . . x . . . | sample bd | swing 0.25"));
        assert!(g.code.contains("hat:  x x x x | swing 0.25  # commentaire de la hat"), "verb kept before the comment, in the groove too");
        assert!(g.code.contains("lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18"), "melody line kept exactly");
        assert!(!g.code.contains("lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18 | swing"), "no swing on melodies");

        let o = transform(SONG, Kind::Open, 0).unwrap();
        assert!(o.code.contains("lead: 1 . 3 . 5 . 4' . | pulse | gain 0.18 | reverb 0.4"));
    }

    #[test]
    fn develop_uses_the_shared_engine() {
        let t = transform(SONG, Kind::Develop, 7).unwrap();
        assert!(t.code.contains("| develop"));
        assert!(t.code.contains("seed: 42"));
        let p = loom_core::Program::parse(&t.code);
        assert_eq!(p.seed, 42);
    }

    #[test]
    fn empty_src_is_polite() {
        assert!(transform("  \n", Kind::Dark, 0).is_err());
    }

    #[test]
    fn labels_read_like_a_choice() {
        assert_eq!(Kind::Dark.label(), "plus sombre");
        assert_eq!(Kind::Transpose(2).label(), "transposer");
        assert_eq!(Kind::Transpose(-1).label(), "plus grave");
        assert!(Kind::Groove.explanation().contains("swing"));
    }

    #[test]
    fn reharmonise_prog() {
        let src = "key: c major\nprog: C A7 Dm7 G7\nkeys: ^ | sine";
        let j = transform(src, Kind::Jazz, 0).unwrap();
        assert!(j.code.contains("prog: C Em7 Eb7 Dm7 Db7"), "{}", j.code);
        let t = transform(src, Kind::Tritone, 0).unwrap();
        assert!(t.code.contains("prog: C Eb7 Dm7 Db7"), "{}", t.code);
        assert!(transform("key: c major\nlead: 1 2 3", Kind::Jazz, 0).is_err());
        assert_eq!(CHIPS.len(), 9);
    }
}
