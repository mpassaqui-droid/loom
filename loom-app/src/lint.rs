//! Le correcteur bienveillant, comme `lint()` du web : il repère les erreurs de
//! débutant et les explique, sans jamais bloquer la lecture. Chaque remarque
//! porte le numéro de la ligne fautive pour que l'éditeur la souligne.

use loom_core::Program;

/// Une remarque : la ligne (0 = première) et l'explication.
#[derive(Debug, Clone, PartialEq)]
pub struct Lint {
    pub line: usize,
    pub msg: String,
}

/// Les mots que LOOM connaît après un `|` (copie de `KEYWORDS` du web, plus
/// les quelques verbes que le web ajoute à la main). Le test
/// `meme_liste_que_le_web` vérifie qu'elle suit le web.
pub const KNOWN: &[&str] = &[
    "auto", "master", "lin", "exp", "log", "step", "smooth", "ease",
    "rev", "fast", "ply", "linger", "segment", "range", "loopAt", "loopat", "slow", "every", "jux", "chunk", "off",
    "add", "semi", "palindrome", "sometimes", "often", "rarely", "rot", "iter", "poly", "markov", "invert", "sequence",
    "mutate", "develop", "stutter", "cresc", "dim", "arp", "pink", "sieve", "arc", "rise", "fall", "gendy", "spectrum",
    "tonnetz", "hocket", "bus", "room", "morph", "solve", "saw", "sine", "tri", "pulse", "square", "pwm", "fm", "wt",
    "pluck", "bow", "reed", "bell", "grain", "vowel", "vocode", "supersaw", "sync", "fold", "ring", "partials", "organ",
    "harmonize", "double", "sub", "noise", "gain", "oct", "unison", "detune", "spread", "drive", "crush", "redux",
    "flange", "reson", "stereo", "chorus", "phaser", "comp", "reverb", "delay", "echo", "ir", "autopan", "haas", "tgate",
    "chance", "lfo", "lfo2", "slide", "bend", "wow", "orbit", "swing", "shift", "humanize", "nudge", "degrade", "gate",
    "strum", "flam", "pan", "eq", "tape", "voicing", "fenv", "env", "drift", "arpoct", "chop", "reverse", "warp",
    "stretch", "freeze", "adaptive", "begin", "end", "root", "sample", "midi", "viz", "hue", "width", "key", "scale",
    "prog", "tune", "bpm", "tempo", "meter", "accel", "groove", "seed", "song", "duck", "feel", "warmth", "scene", "film",
    "title", "major", "minor", "dorian", "phrygian", "lydian", "mixolydian", "locrian", "pentatonic", "harmonic_minor",
    "melodic_minor", "melodic", "khamaj", "desh", "blues", "chromatic", "hijaz", "bhairav", "bhairavi", "yaman", "pelog", "slendro", "bayati", "rast",
    "saba", "kurd", "nahawand", "malkauns", "ryukyu", "warm", "rich", "soft", "up", "down", "updown", "downup",
    "converge", "diverge", "thumb", "pinky", "open", "drop2", "wide", "lead", "just", "pyth", "werck", "kirn", "meantone",
    "cutoff", "pitch", "reso", "hall", "plate", "spring", "cathedral", "auto", "bd", "sd", "hh", "cp", "oh", "rim", "cow",
    "perc", "tom", "ride", "tabla", "amen", "break", "sitar", "arpy", "sax", "juno", "vox", "yeah", "breath", "glass",
    "metal", "wood",
    // lus par le parseur (synth.rs, lib.rs) : verbes, gammes, accords, grooves
    "7b5", "7b9", "M13", "acoustic", "add11", "add9", "aeolian", "air", "ajam", "altered",
    "aug", "augmented", "ayoub", "baiao", "bebop", "bebop_dominant", "bebop_major", "bembe", "blues_major", "bossa",
    "byzantine", "chinese", "cinquillo", "conv", "decresc", "dilla", "dim7", "diminished", "div", "dom7",
    "double_harmonic", "drawbars", "egyptian", "enigmatic", "fifth", "flutter", "formant", "glide", "grace", "granular",
    "gypsy", "harm", "harmonicminor", "hijazkar", "hirajoshi", "hmz", "humanish", "hungarian_major", "hungarian_minor", "huzam",
    "insen", "ionian", "iwato", "jiharkah", "kumoi", "laidback", "lydian_augmented", "lydian_dominant", "m11", "m13",
    "m7b5", "maj", "maj13", "maj7", "maj9", "majblues", "majpent", "maqsum", "marwa", "melodicminor",
    "messiaen3", "messiaen4", "min", "min6", "min7", "min7b5", "min9", "minpent", "modal", "montuno",
    "mpc", "neapolitan_major", "neapolitan_minor", "octatonic", "octatonic_hw", "octatonic_wh", "octup", "overtone", "persian", "phrygian_dominant",
    "pinkyup", "prometheus", "purvi", "push", "rand", "ritusen", "rotate", "rumba", "samba", "sawtooth",
    "send", "shiko", "sikah", "someby", "son", "spanish", "string", "super_locrian", "sus", "sus2",
    "sus4", "swing16", "thumbup", "todi", "trancegate", "transpose", "tresillo", "triangle", "vocoder", "voice",
    "whole_tone", "wholetone", "widen", "wind",
    "mmaj7", "mMaj7", "aug7",
    // ajoutés à la main par le web
    "in", "lp", "hp", "bp", "notch", "q", "mute", "solo", "over", "chord", "s",
];

fn code(line: &str) -> &str {
    // un `#` précédé d'un blanc ouvre un commentaire (`c#4` reste une note)
    let mut prev = ' ';
    for (i, ch) in line.char_indices() {
        if ch == '#' && prev.is_whitespace() { return &line[..i]; }
        prev = ch;
    }
    line
}

fn lev(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 2 { return 9; }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn is_degree(t: &str) -> bool {
    let mut c = t.chars().peekable();
    if !matches!(c.next(), Some('1'..='7')) { return false; }
    if matches!(c.peek(), Some('#' | 'b')) { c.next(); }
    while matches!(c.peek(), Some('\'' | ',')) { c.next(); }
    c.peek().is_none_or(|ch| matches!(ch, '?' | '*' | '!'))
}

/// La ligne où la voix `name` (ou `name@section`) est définie.
fn voice_line(src: &str, name: &str, section: Option<&str>) -> usize {
    let want = match section { Some(s) => format!("{name}@{s}"), None => name.to_string() };
    src.lines().position(|l| {
        let head = code(l).split(':').next().unwrap_or("").trim();
        head == want || head.split('(').next().is_some_and(|h| h.trim() == want)
    }).unwrap_or(0)
}

/// Les remarques sur `src`. `samples` = les sons connus du moteur (`None` : ne pas vérifier).
pub fn lint(src: &str, samples: Option<&[String]>) -> Vec<Lint> {
    let program = Program::parse(src);
    let mut out: Vec<Lint> = Vec::new();
    let song: Vec<&str> = program.song.iter().map(|(s, _)| s.as_str()).collect();
    let (mut deg_line, mut prog_line) = (None, None);
    for v in &program.voices {
        let line = voice_line(src, &v.name, v.section.as_deref());
        if let (Some(known), Some(name)) = (samples, v.synth.sample.as_ref()) {
            if !known.iter().any(|k| k == name) {
                out.push(Lint { line, msg: format!("sample « {name} » inconnu ({})", v.name) });
            }
        }
        if let Some(secs) = &v.sections {
            for s in secs {
                if !song.is_empty() && !song.contains(&s.as_str()) {
                    out.push(Lint { line, msg: format!("« {} » joue dans « {s} » mais cette section n'est pas dans song:", v.name) });
                }
            }
        }
        if let Some(s) = &v.section {
            if !song.is_empty() && !song.contains(&s.as_str()) {
                out.push(Lint { line, msg: format!("« {}@{s} » : section absente de song:", v.name) });
            }
        }
        for t in v.stacks.iter().flatten() {
            if deg_line.is_none() && !v.is_drum && is_degree(t) { deg_line = Some(line); }
            if prog_line.is_none() && t.starts_with('^') { prog_line = Some(line); }
        }
    }
    if let Some(line) = deg_line {
        if program.key.is_none() && program.key_map.is_empty() {
            out.push(Lint { line, msg: "des degrés (1-7) sont utilisés sans « key: », ajoute p.ex. key: a minor".into() });
        }
    }
    if let Some(line) = prog_line {
        if program.prog.is_empty() {
            out.push(Lint { line, msg: "le token ^ est utilisé sans « prog: », ajoute p.ex. prog: Am F C G".into() });
        }
    }
    // faute de frappe : un verbe inconnu proche d'un connu
    let mut seen: Vec<String> = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        for seg in code(raw).split('|').skip(1) {
            let w = seg.split_whitespace().next().unwrap_or("").to_lowercase();
            if w.chars().count() < 3 || KNOWN.contains(&w.as_str()) || seen.contains(&w)
                || w.starts_with(|c: char| c.is_ascii_digit() || c == '.') { continue; }
            seen.push(w.clone());
            let best = KNOWN.iter().map(|k| (lev(&w, k), *k)).filter(|(d, _)| *d <= 2).min_by_key(|(d, _)| *d);
            if let Some((_, k)) = best {
                out.push(Lint { line: i, msg: format!("« {w} » n'existe pas, tu voulais « {k} » ?") });
            }
        }
    }
    out.sort_by_key(|l| l.line);
    out
}

/// La ligne d'aide affichée après RUN (4 remarques au plus, comme le web).
pub fn summary(lints: &[Lint]) -> Option<String> {
    if lints.is_empty() { return None; }
    let mut s = lints.iter().take(4).map(|l| format!("l.{} {}", l.line + 1, l.msg)).collect::<Vec<_>>().join("  ·  ");
    if lints.len() > 4 { s.push_str(&format!("  …(+{})", lints.len() - 4)); }
    Some(format!("[ aide ] {s}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_of(src: &str, needle: &str) -> usize { src.lines().position(|l| l.contains(needle)).unwrap() }

    #[test]
    fn faute_de_frappe() {
        let src = "bpm: 100\nlead: c4 e4 | revreb 0.3\n";
        let l = lint(src, None);
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!(l[0].line, line_of(src, "revreb"));
        assert!(l[0].msg.contains("reverb"));
    }

    #[test]
    fn degres_sans_tonalite_et_progression() {
        let l = lint("lead: 1 3 5 3\npad: ^ ^\n", None);
        assert!(l.iter().any(|x| x.msg.contains("key:") && x.line == 0), "{l:?}");
        assert!(l.iter().any(|x| x.msg.contains("prog:") && x.line == 1), "{l:?}");
        assert!(lint("key: a minor\nlead: 1 3 5 3\n", None).is_empty());
    }

    #[test]
    fn sections_et_samples() {
        let src = "song: intro 2 verse 4\nkick: x . x . | in drop\nbass@chorus: c2 c2\nperc: x x | sample zorglub\n";
        let l = lint(src, Some(&["bd".to_string()]));
        assert!(l.iter().any(|x| x.msg.contains("drop") && x.line == 1), "{l:?}");
        assert!(l.iter().any(|x| x.msg.contains("bass@chorus") && x.line == 2), "{l:?}");
        assert!(l.iter().any(|x| x.msg.contains("zorglub") && x.line == 3), "{l:?}");
    }

    #[test]
    fn un_morceau_propre_ne_dit_rien() {
        let src = "bpm: 120\nkick: x . x . | gain 0.8\nlead: c4 eb4 g4 | saw | reverb 0.3   # c#4 en commentaire\n";
        assert!(lint(src, None).is_empty());
        assert_eq!(summary(&[]), None);
    }

    /// Chaque mot que le parseur accepte en tête de branche est connu du correcteur
    /// (sinon `| send …` passait pour une faute de `bend`).
    #[test]
    fn connait_tout_ce_que_le_parseur_lit() {
        for f in ["synth.rs", "lib.rs"] {
            let src = std::fs::read_to_string(format!("{}/../loom-core/src/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            for line in src.lines() {
                let l = line.trim_start();
                let Some(arm) = l.split("=>").next().filter(|_| l.starts_with('"') && l.contains("=>")) else { continue };
                let arm = arm.split(" if ").next().unwrap_or(arm);
                for w in arm.split('|').map(|x| x.trim().trim_matches('"')) {
                    let ok = w.len() < 3 || !w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                    assert!(ok || KNOWN.contains(&w), "{f} : « {w} » lu par le parseur mais inconnu du correcteur");
                }
            }
        }
        assert!(lint("lead: c4 | send rev 0.4 | glide 0.1 | widen 1.2\n", None).is_empty());
        let auto = "send: rev reverb hall\npad: c4 | lp 200..4000@exp..800 | over 8\nauto: master 0..1@smooth..0 over 8\nauto: send rev 0..0.8\n";
        assert!(lint(auto, None).is_empty(), "{:?}", lint(auto, None));
    }

    #[test]
    fn meme_liste_que_le_web() {
        let web = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../web/index.html")).unwrap();
        let i = web.find("const KEYWORDS=[").unwrap();
        let j = i + web[i..].find(']').unwrap();
        for w in web[i..j].split('"').skip(1).step_by(2) {
            assert!(KNOWN.contains(&w), "mot du web absent du correcteur : {w}");
        }
    }
}
