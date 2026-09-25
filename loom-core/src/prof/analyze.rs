//! Instant, OFFLINE analysis of a piece — the "why it sounds the way it does"
//! report the teacher shows before (and without) the AI brain. Faithful port of
//! the browser's `analyzePiece` + `noveltyReport` + `chordRoughness` from
//! `web/index.html`, so the native apps AND the web give exactly the same
//! knowledge-base answer: tonality, microtuning, acoustics of the tonic triad
//! (Plomp–Levelt), harmony in roman numerals + cadence, the obvious "why", the
//! form, and a measured novelty score (variety / pitch entropy / rhythm entropy).
//! Lives in `loom-core` (std-only, no dependencies) so the browser can run it
//! through `loom-wasm` and native apps through `loom-app` — one analyser everywhere.

use std::collections::{HashMap, HashSet};

use crate::{parse_accel, parse_key, parse_meter_groups, parse_prog, parse_song, token_to_freq, Program, Key};

use super::lore::SCALE_LORE;

/// French solfège for the tonic's pitch class (the web's `NN`).
const NOTE_NAMES: [&str; 12] = ["do", "do#", "ré", "ré#", "mi", "fa", "fa#", "sol", "sol#", "la", "la#", "si"];

/// Format beats for prose: `7` -> "7", `3.5` -> "3,5", `2.25` -> "2,25".
fn beautify_beats(m: f64) -> String {
    if (m - m.round()).abs() < 1e-9 { format!("{}", m.round() as i64) } else { format!("{m}").replace('.', ",") }
}

/// Which tradition a scale belongs to (the web's `TRAD_OF`).
const TRAD_OF: &[(&str, &str)] = &[
    ("bhairav", "raga"), ("bhairavi", "raga"), ("yaman", "raga"), ("malkauns", "raga"),
    ("todi", "raga"), ("marwa", "raga"), ("purvi", "raga"),
    ("hijaz", "maqam"), ("rast", "maqam"), ("bayati", "maqam"), ("persian", "maqam"),
    ("byzantine", "maqam"), ("double_harmonic", "maqam"),
    ("slendro", "gamelan"), ("pelog", "gamelan"),
    ("chinese", "wuxing"),
    ("pentatonic", "pentatonique"), ("minpent", "pentatonique"), ("egyptian", "pentatonique"),
    ("blues", "blues"),
    ("major", "modes"), ("minor", "modes"), ("dorian", "modes"), ("phrygian", "modes"),
    ("lydian", "modes"), ("mixolydian", "modes"), ("locrian", "modes"),
];

/// The `tune:` directive the way the web's `parseTune` reads it (equal = None).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tune {
    Just,
    Pyth,
    Werck,
    Kirn,
    Meantone,
    Adaptive,
    /// N-tone equal temperament, 3..=96.
    Edo(u32),
}

impl Tune {
    /// The web's `String(tune)` shown in the "accordage" line ("19" for 19edo).
    fn label(&self) -> String {
        match self {
            Tune::Just => "just".into(),
            Tune::Pyth => "pyth".into(),
            Tune::Werck => "werck".into(),
            Tune::Kirn => "kirn".into(),
            Tune::Meantone => "meantone".into(),
            Tune::Adaptive => "adaptive".into(),
            Tune::Edo(n) => n.to_string(),
        }
    }
}

/// `tune:` directive, like the web's `parseTune`. Returns None for equal/unknown
/// (the web returns the string "equal" — same effect: nothing reported).
fn parse_tune(text: &str) -> Option<Tune> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        if !line[..colon].trim().eq_ignore_ascii_case("tune") { continue; }
        let w = line[colon + 1..].trim().to_lowercase();
        if let Some(rest) = w.strip_suffix("edo") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return Some(Tune::Edo(n.clamp(3, 96)));
            }
        }
        return Some(
            if w.starts_with("just") || w == "ji" { Tune::Just }
            else if w.starts_with("pyth") { Tune::Pyth }
            else if w.starts_with("werck") { Tune::Werck }
            else if w.starts_with("kirn") { Tune::Kirn }
            else if w.starts_with("mean") || w.starts_with("quart") { Tune::Meantone }
            else if w.starts_with("adapt") { Tune::Adaptive }
            else { return None }, // "equal" or unknown
        );
    }
    None
}

/// The last declared scale name (the web's `currentScaleName`), skipping words
/// that parse as numbers (a custom numeric microtonal scale).
fn current_scale_name(text: &str) -> Option<String> {
    let mut nm: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let head = line[..colon].trim().to_lowercase();
        let words: Vec<String> = line[colon + 1..].trim().to_lowercase().split_whitespace().map(str::to_string).collect();
        if head == "key" && !words.is_empty() {
            let w = words.get(1);
            if w.is_some_and(|w| !is_numeric_word(w)) { nm = w.cloned(); }
        } else if head == "scale" {
            let w = words.first();
            if w.is_some_and(|w| !is_numeric_word(w)) { nm = w.cloned(); }
        }
    }
    nm
}

/// Starts with an (optional minus then) ASCII digit or dot, like the web's `/^-?[\d.]/`.
fn is_numeric_word(w: &str) -> bool {
    let mut chars = w.chars();
    if chars.clone().next() == Some('-') { chars.next(); }
    matches!(chars.next(), Some(c) if c.is_ascii_digit() || c == '.')
}

/// Whether the text declares a custom numeric (microtonal) key scale:
/// `key: <tonic> <num> <num> …` with at least two numeric steps (web regex
/// `key:\s*\S+\s+-?[\d.]+\s+-?[\d.]+`).
fn has_custom_key_scale(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim();
        let Some(colon) = l.find(':') else { return false };
        if !l[..colon].trim().eq_ignore_ascii_case("key") { return false; }
        let mut words = l[colon + 1..].split_whitespace();
        let _tonic = words.next();
        let mut nums = 0usize;
        for w in words {
            if is_numeric_word(w) { nums += 1; } else { break; }
        }
        nums >= 2
    })
}

/// `\b(kw…)\s*[:@]` anywhere: a word that introduces a held/drone voice (pad, sub…).
fn has_kw_directive(text: &str, kws: &[&str]) -> bool {
    let lower = text.to_lowercase();
    let b = lower.as_bytes();
    for kw in kws {
        for i in 0..b.len().saturating_sub(kw.len()) {
            let start_ok = i == 0 || !b[i - 1].is_ascii_alphanumeric();
            if start_ok && lower[i..i + kw.len()] == **kw {
                let mut j = i + kw.len();
                while j < b.len() && b[j].is_ascii_whitespace() { j += 1; }
                if j < b.len() && (b[j] == b':' || b[j] == b'@') { return true; }
            }
        }
    }
    false
}

/// `|\s*verb\b` — a pipe-verb starts with one of these words (the web's
/// `/\|\s*slide|bend|wow\b/`). `after` = legal char after the verb ("" for `\b`).
fn has_pipe_verb(text: &str, verbs: &[(&str, bool)]) -> bool {
    for line in text.lines() {
        for part in line.split('|').skip(1) {
            let p = part.trim_start().to_lowercase();
            for (v, needs_boundary) in verbs {
                if let Some(rest) = p.strip_prefix(v) {
                    let after = rest.chars().next();
                    let boundary_ok = !*needs_boundary || after.is_none() || !after.unwrap().is_alphanumeric();
                    if boundary_ok { return true; }
                }
            }
        }
    }
    false
}

/// `\b(kw…)\s*\(` — a generative call like `walk(`, `e(`, `ca(` (the web's `has`).
fn has_generator_call(text: &str, kws: &[&str]) -> bool {
    let lower = text.to_lowercase();
    let b = lower.as_bytes();
    for kw in kws {
        for i in 0..b.len().saturating_sub(kw.len()) {
            let start_ok = i == 0 || !b[i - 1].is_ascii_alphanumeric();
            if start_ok && lower[i..i + kw.len()] == **kw {
                let mut j = i + kw.len();
                while j < b.len() && b[j].is_ascii_whitespace() { j += 1; }
                if j < b.len() && b[j] == b'(' { return true; }
            }
        }
    }
    false
}

/// `\be(` plus a digit — an euclidean rhythm (web `/\be\(\d/`).
fn has_euclid(text: &str) -> bool {
    let b = text.as_bytes();
    for i in 0..b.len().saturating_sub(2) {
        let start_ok = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if start_ok && b[i] == b'e' && b[i + 1] == b'(' && b[i + 2].is_ascii_digit() { return true; }
    }
    false
}

/// A lone `| swing …` / `| groove: …`, or a `groove:` line — the web's
/// `/\|\s*(swing|groove:)/` plus `/^groove:/m`.
fn has_swing_groove(text: &str) -> bool {
    for line in text.lines() {
        let t = line.trim();
        if t.to_lowercase().starts_with("groove:") { return true; }
        for part in t.split('|').skip(1) {
            let p = part.trim().to_lowercase();
            if p.starts_with("swing") || p.starts_with("groove:") { return true; }
        }
    }
    false
}

/// The web's `| gate 2..9` (a held, breathing gate — it "anchors the ear").
fn has_musical_gate(text: &str) -> bool {
    has_pipe_verb(text, &[
        ("gate", true),
    ]) && text.lines().any(|l| {
        // `|\s*gate\s+[2-9]`
        l.split('|').skip(1).any(|part| {
            let p = part.trim();
            let rest = p.strip_prefix("gate").unwrap_or(p).trim_start();
            rest.chars().next().is_some_and(|c| ('2'..='9').contains(&c)) && rest.chars().all(|c| c.is_ascii_digit() || c.is_whitespace())
        })
    })
}

/// Plomp–Levelt sensory-dissonance of two partials (the web's `partialRoughness`).
fn partial_roughness(f1: f32, f2: f32) -> f32 {
    let (f1, f2) = if f1 > f2 { (f2, f1) } else { (f1, f2) };
    let s = 0.24 / (0.0207 * f1 + 18.96);
    let x = f2 - f1;
    (-3.5 * s * x).exp() - (-5.75 * s * x).exp()
}

/// Summed sensory dissonance of a chord: each frequency's first `nh` partials,
/// weighted by harmonic strength (0.88^(k-1)), over all pairs. Low = consonant.
fn chord_roughness(freqs: &[f32], nh: u32) -> f32 {
    let nh = nh.max(1);
    let mut partials: Vec<(f32, f32)> = Vec::with_capacity(freqs.len() * nh as usize);
    for &f in freqs {
        for k in 1..=nh {
            partials.push((f * k as f32, 0.88f32.powi(k as i32 - 1)));
        }
    }
    let mut d = 0.0;
    let mut n = 0.0;
    for i in 0..partials.len() {
        for j in (i + 1)..partials.len() {
            let w = partials[i].1 * partials[j].1;
            d += w * partial_roughness(partials[i].0, partials[j].0);
            n += w;
        }
    }
    if n > 0.0 { d / n } else { 0.0 }
}

/// Roman-numeral analysis of a chord progression against a key (the web's
/// `progRoman`): `Am7 F C G` → `im7 VI III VII` — minor chords lowercase.
fn prog_roman(key: &Key, prog: &[String]) -> Option<String> {
    const RN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    let mut seen: Vec<&str> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for chord in prog {
        let chord = chord.trim();
        if chord.is_empty() || seen.contains(&chord) { continue; }
        seen.push(chord);
        // already a roman numeral (web `/^[ivx]+\d*$/i` and not an uppercase note letter)?
        let b = chord.as_bytes();
        let mut i = 0;
        while i < b.len() && matches!(b[i], b'i' | b'I' | b'v' | b'V' | b'x' | b'X') { i += 1; }
        while i < b.len() && b[i].is_ascii_digit() { i += 1; }
        if i == b.len() && !chord.starts_with(|c: char| matches!(c, 'A'..='G')) {
            out.push(chord.to_string());
            continue;
        }
        // chord root letter → pitch class
        let mut it = chord.chars();
        let Some(root) = it.next() else { continue };
        let mut pc: i32 = match root {
            'C' => 0, 'D' => 2, 'E' => 4, 'F' => 5, 'G' => 7, 'A' => 9, 'B' => 11,
            _ => { out.push(chord.to_string()); continue; }
        };
        match it.next() {
            Some('#') => pc += 1,
            Some('b') => pc -= 1,
            _ => {}
        }
        pc = pc.rem_euclid(12);
        // which scale degree has that pitch class?
        let mut deg = -1i32;
        for (i, &step) in key.scale.iter().enumerate() {
            if (key.tonic + step.round() as i32).rem_euclid(12) == pc { deg = i as i32; break; }
        }
        if deg < 0 { out.push(chord.to_string()); continue; }
        let lower = chord.to_lowercase();
        let minor = lower.contains("dim") || (lower.contains('m') && !lower.contains("maj"));
        let mut r = RN[deg.rem_euclid(7) as usize].to_string();
        if minor { r.make_ascii_lowercase(); }
        out.push(r);
    }
    if out.is_empty() { None } else { Some(out.join(" ")) }
}

/// The web's `cadenceOf`: name a resolution inside the roman-numeral chain.
fn cadence_of(roman: &str) -> Option<&'static str> {
    let upper = roman.to_uppercase();
    let t: Vec<&str> = upper.split_whitespace().collect();
    for i in 0..t.len().saturating_sub(2) {
        if t[i] == "II" && t[i + 1] == "V" && t[i + 2] == "I" {
            return Some("ii–V–I, la cadence reine du jazz (double tension qui résout)");
        }
    }
    for i in 0..t.len().saturating_sub(1) {
        if t[i] == "V" && t[i + 1] == "I" { return Some("cadence parfaite (V→I) : la résolution la plus forte, conclusive"); }
        if t[i] == "IV" && t[i + 1] == "I" { return Some("cadence plagale (IV→I) : douce, le « amen » des hymnes"); }
        if t[i] == "V" && t[i + 1] == "VI" { return Some("cadence rompue (V→vi) : la résolution attendue est déjouée, suspense"); }
    }
    if t.last() == Some(&"V") { return Some("demi-cadence (…→V) : on reste suspendu, en attente"); }
    None
}

/// Normalised Shannon entropy (0..1) of a bag of strings (the web's `ent`).
fn entropy(items: &[String]) -> f64 {
    if items.len() < 2 { return 0.0; }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for x in items { *counts.entry(x.as_str()).or_insert(0) += 1; }
    let n = items.len() as f64;
    let mut h = 0.0;
    let mut u = 0usize;
    for &c in counts.values() {
        let p = c as f64 / n;
        h -= p * p.log2();
        u += 1;
    }
    if u > 1 { h / (u as f64).log2() } else { 0.0 }
}

/// How much the music actually changes bar to bar (the web's `noveltyReport`):
/// the real engine schedules bars spread across the whole arrangement and
/// measures per-voice variety, pitch entropy and rhythm entropy, then flags
/// dense voices that loop identically. No AI — pure schedule + measurement.
pub fn novelty_report(text: &str) -> Vec<String> {
    let program = Program::parse(text);
    // Same default length as the web: no `song:` arrangement -> a nominal 8-bar
    // piece (not 1 bar, which would just re-sample the same bar and inflate the
    // per-bar variety score).
    let declared = program.len_bars();
    let total = if declared > 0 { declared as i64 } else { 8 };
    let n = (total as usize).clamp(2, 10);
    // Sample bars spread across the WHOLE arrangement, so a piece that builds
    // and contrasts is judged fairly.
    let bars: Vec<i64> = (0..n).map(|i| (i as i64) * total / n as i64).collect();

    let mut per_voice: HashMap<String, Vec<String>> = HashMap::new();
    let mut mel: Vec<String> = Vec::new();
    let mut onsets: Vec<String> = Vec::new();
    for bar in bars {
        let events = program.schedule_bar(bar);
        let mut by_voice: HashMap<String, Vec<String>> = HashMap::new();
        for e in &events {
            // the web fingerprints each hit as (phase in ms):token
            let phase = format!("{:.3}", (e.phase * 1000.0).round() / 1000.0);
            by_voice.entry(e.voice.clone()).or_default().push(format!("{phase}:{}", e.token));
            if !e.is_drum && !e.token.starts_with(['.', '_', '~']) {
                mel.push(e.token.clone());
            }
            onsets.push(format!("{}", (e.phase * 16.0).round() as i64));
        }
        for (voice, mut sigs) in by_voice {
            sigs.sort();
            per_voice.entry(voice).or_default().push(sigs.join(","));
        }
    }

    let mut var_sum = 0.0;
    let mut vc = 0usize;
    let mut repeats: Vec<String> = Vec::new();
    for (voice, sigs) in &per_voice {
        let uniq: HashSet<&String> = sigs.iter().collect();
        var_sum += uniq.len() as f64 / sigs.len().max(1) as f64;
        vc += 1;
        // flag only DENSE voices (>=6 hits/bar) that loop identically — a sparse
        // bourdon/pad repeating is intentional, not a fault (same as the web).
        if sigs.len() >= 3 && uniq.len() == 1 && sigs[0].split(',').count() >= 6 {
            repeats.push(voice.clone());
        }
    }
    let avg_var = if vc > 0 { var_sum / vc as f64 } else { 1.0 };
    let pitch_ent = entropy(&mel);
    let rhythm_ent = entropy(&onsets);
    let score = (100.0 * (0.6 * avg_var + 0.22 * pitch_ent + 0.18 * rhythm_ent)).round() as i64;

    let mut out = vec![format!(
        "[ nouveauté ] score {score}/100  (variété/mesure {}% · hauteurs {}% · rythme {}%)",
        (avg_var * 100.0).round() as i64,
        (pitch_ent * 100.0).round() as i64,
        (rhythm_ent * 100.0).round() as i64,
    )];
    if !repeats.is_empty() {
        out.push(format!(
            "   ⤷ ces voix denses bouclent à l'identique : {} — varie-les (générateur walk/drunk/ca, ou | every N rot/mutate/shuffle)",
            repeats.join(", ")
        ));
    }
    if score < 50 {
        out.push("   ⤷ assez répétitif : fais évoluer les sections (@section), varie la batterie, ou monte le seed".into());
    } else if score >= 75 {
        out.push("   ⤷ riche & changeant : ça ne boucle pas bêtement 👏".into());
    }
    out
}

/// The full offline teacher's analysis of a song (the web's `analyzePiece`):
/// tonality, microtuning, tonic-triad acoustics, roman-numeral harmony +
/// cadence, "why it sounds like this", the form, and the novelty report.
/// Pure knowledge base — zero AI — so it works without internet.
pub fn analyze(text: &str) -> Vec<String> {
    // ignore whole comment lines (the same as the web's first line)
    let src: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out: Vec<String> = Vec::new();

    let key = parse_key(&src);
    let scale_name = current_scale_name(&src);
    if let Some(k) = &key {
        let tonic = NOTE_NAMES[k.tonic.rem_euclid(12) as usize];
        let mut s = format!("[ analyse ] tonalité : {tonic}");
        if let Some(sn) = &scale_name {
            s.push(' ');
            s.push_str(sn);
        }
        if let Some((_, lore)) = scale_name.as_deref().and_then(|sn| SCALE_LORE.iter().find(|(n, _)| *n == sn)) {
            s.push_str(" — ");
            s.push_str(lore);
        }
        out.push(s);
        if let Some((_, trad)) = scale_name.as_deref().and_then(|sn| TRAD_OF.iter().find(|(n, _)| *n == sn)) {
            out.push(format!("   tradition : {trad} (tape ?{trad} pour l'histoire & le sens)"));
        }
    }

    let tune = parse_tune(&src);
    let tune_word = tune.map(|t| t.label());
    let micro_word = tune_word.as_deref().is_some_and(|w| {
        const WORDS: [&str; 7] = ["edo", "maqam", "bayati", "rast", "saba", "sikah", "huzam"];
        w.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| WORDS.contains(&w) || (w.ends_with("edo") && w[..w.len() - 3].chars().all(|c| c.is_ascii_digit())))
    });
    if has_custom_key_scale(&src) || micro_word {
        out.push("   microtonal : des notes « entre les touches » (quarts de ton / EDO) — couleur hors du piano. ?microtonal".into());
    } else if let Some(t) = &tune {
        out.push(format!("   accordage {} : intervalles aux ratios purs → plus consonant, « plus juste ». ?pythagore", t.label()));
    }

    // ACOUSTIQUE — Plomp–Levelt consonance of the tonic triad (1·3·5), judged
    // register-normalised so it's the CHORD, not its octave, that's assessed.
    if let Some(k) = &key {
        let mut triad: Vec<f32> = ["1", "3", "5"]
            .iter()
            .filter_map(|d| token_to_freq(d, Some(k)))
            .filter(|f| f.is_finite())
            .collect();
        if triad.len() >= 2 {
            let lo = triad.iter().cloned().fold(f32::INFINITY, f32::min);
            for f in triad.iter_mut() { *f = *f * 261.63 / lo; }
            let r = chord_roughness(&triad, 6);
            let cons = (100.0 * (1.0 - (r / 0.02).min(1.0))).round() as i64;
            let lbl = if cons >= 75 { "très consonant (reposé)" }
                      else if cons >= 50 { "consonant, doux" }
                      else if cons >= 30 { "tendu" }
                      else { "dissonant, âpre" };
            out.push(format!("[ acoustique ] accord de tonique (1·3·5) : consonance {cons}/100 — {lbl} · rugosité = battements des partiels dans la bande critique (?pythagore)"));
        }
    }

    // POURQUOI (obvious features read straight off the code)
    let mut why: Vec<String> = Vec::new();
    if has_kw_directive(&src, &["drone", "tanpura", "bourdon", "pad", "sub", "sa"])
        || has_pipe_verb(&src, &[("bow", true)])
        || has_musical_gate(&src)
    {
        why.push("un bourdon/nappe tenu ancre l'oreille (le repos)".into());
    }
    if has_pipe_verb(&src, &[("slide", true), ("bend", true), ("wow", true)]) {
        why.push("les glissés/ornements (slide·bend) rendent les notes expressives, vivantes".into());
    }
    if has_generator_call(&src, &["walk", "drunk", "ca", "lsys", "pi", "perlin", "logistic", "fib", "morse", "bin", "recaman", "cantor"]) {
        why.push("des suites génératives font varier le motif à chaque mesure (jamais figé)".into());
    }
    if has_euclid(&src) {
        why.push("des rythmes euclidiens répartissent les coups régulièrement-irrégulièrement (groove du monde)".into());
    }
    let prog = parse_prog(&src);
    let roman = key.as_ref().and_then(|k| prog_roman(k, &prog)).filter(|r| !r.is_empty());
    if let Some(r) = &roman {
        let mut line = format!("[ harmonie ] progression : {r}");
        if let Some(c) = cadence_of(r) {
            line.push_str(" — ");
            line.push_str(c);
        }
        out.push(line);
        why.push("la progression enchaîne tension (dominante) → détente (tonique)".into());
    }
    if has_swing_groove(&src) {
        why.push("le swing/groove décale les temps → ça balance (humain, pas robotique)".into());
    }
    if let Some((meter, groups)) = parse_meter_groups(&src) {
        if meter > 0.0 && (meter - 4.0).abs() > 0.01 {
            match groups {
                Some(g) => why.push(format!("une métrique composée {} ({} temps) déstabilise joliment le pas",
                    g.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("+"), beautify_beats(meter))),
                None => why.push(format!("une métrique inhabituelle ({} temps) déstabilise joliment le pas", beautify_beats(meter))),
            }
        }
    }
    if parse_accel(&src).is_some() {
        why.push("le tempo accélère/respire au fil du morceau".into());
    }
    if !why.is_empty() {
        let text = why.join(" · ");
        out.push(format!("[ pourquoi ça sonne ] {text}."));
    }

    let song = parse_song(&src);
    if !song.is_empty() {
        let form = song.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(" → ");
        out.push(format!("[ forme ] {form}"));
    }
    out.extend(novelty_report(&src));
    if out.is_empty() {
        out.push("[ analyse ] morceau libre (pas de tonalité déclarée) — les notes sont absolues.".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tune_matches_the_web() {
        assert_eq!(parse_tune("tune: equal"), None);
        assert_eq!(parse_tune("bpm: 120\ntune: pyth"), Some(Tune::Pyth));
        assert_eq!(parse_tune("tune: 19 edo"), Some(Tune::Edo(19)));
        assert_eq!(parse_tune("tune: quarter-comma meantone"), Some(Tune::Meantone));
        assert_eq!(parse_tune("tune: adaptive"), Some(Tune::Adaptive));
        assert_eq!(parse_tune("hole: 1"), None);
    }

    #[test]
    fn current_scale_name_skips_numeric_keys() {
        let s = "key: c minor\nlead: 1 3 5\n";
        assert_eq!(current_scale_name(s).as_deref(), Some("minor"));
        assert_eq!(current_scale_name("scale: dorian").as_deref(), Some("dorian"));
        assert_eq!(current_scale_name("key: d 0 2 3.5 5 7 8 10"), None, "numeric custom scale → no name");
        assert_eq!(current_scale_name("key: c major").as_deref(), Some("major"));
    }

    #[test]
    fn detects_microtonal_and_tune_words() {
        assert!(has_custom_key_scale("key: d 0 1.5 3 5"));
        assert!(!has_custom_key_scale("key: c major"));
        assert!(!has_custom_key_scale("lead: 1 3 5"));
    }

    #[test]
    fn door_verbs_and_generators() {
        let text = "drone: 1 | saw\npad: c3 | bow\ngate: x | gate 4\n";
        assert!(has_kw_directive(text, &["drone", "pad", "sub"]));
        assert!(has_musical_gate("hat: x | gate 7"));
        assert!(!has_musical_gate("hat: x | gate 0"));
        assert!(has_pipe_verb("lead: 1 | bend\n", &[("bend", true)]));
        assert!(!has_pipe_verb("lead: 1 | bent\n", &[("bend", true)]));
        assert!(has_generator_call("bass: walk(3,7) 1", &["walk"]));
        assert!(has_euclid("hat: e(3,8)"));
        assert!(!has_euclid("lead: 1 3"));
        assert!(has_swing_groove("hat: x | swing 0.2"));
        assert!(has_swing_groove("\ngroove: dilla"));
        assert!(!has_swing_groove("lead: 1 | saw"));
    }

    #[test]
    fn roman_numerals_and_cadences() {
        let k = parse_key("key: c major").unwrap();
        let to = |p: &[&str]| prog_roman(&k, &p.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        assert_eq!(to(&["C", "Am7", "F", "G"]), "I vi IV V");
        assert_eq!(to(&["Dm", "G7", "C"]), "ii V I");
        assert_eq!(prog_roman(&k, &[]), None);
        let roman = to(&["Dm", "G7", "C"]);
        assert!(cadence_of(&roman).unwrap().contains("jazz"));
        assert!(cadence_of("IV I").unwrap().starts_with("cadence plagale"));
        assert!(cadence_of("C").is_none());
        assert!(cadence_of("I IV").is_none(), "forwards IV I is not a cadence, the web agrees");
    }

    #[test]
    fn roughness_and_entropy() {
        let uni = chord_roughness(&[261.63, 329.63, 392.0], 6); // C E G
        let triton = chord_roughness(&[261.63, 369.99], 6);     // C F#
        assert!(triton > uni, "a tritone is rougher than a major triad");
        let e = entropy(&["a".into(), "b".into(), "a".into(), "b".into()]);
        assert!((e - 1.0).abs() < 1e-9, "two balanced symbols → entropy 1");
        assert_eq!(entropy(&["a".into()]), 0.0);
    }

    #[test]
    fn analyze_known_song_reports_the_web_lines() {
        let text = "key: c major\nprog: Dm G7 C\ntune: pyth\nkick: e(3,8) | every 4 rot 3\nlead: 1 3 5 6 | walk(3,7)\nsong: intro 4 verse 8\n";
        let out = analyze(text);
        let joined = out.join("\n");
        assert!(out.iter().any(|l| l.starts_with("[ analyse ] tonalité : do major")), "{out:?}");
        assert!(joined.contains("accordage pyth"), "{joined}");
        assert!(joined.contains("[ harmonie ] progression : ii V I"), "{joined}");
        assert!(joined.contains("cadence"), "{joined}");
        assert!(out.iter().any(|l| l.starts_with("[ forme ] intro → 4 → verse → 8")), "{out:?}");
        // generated + euclid => the "why" carries both reasons
        assert!(joined.contains("génératives") && joined.contains("euclidiens"), "{joined}");
        assert!(out.iter().any(|l| l.starts_with("[ nouveauté ]")), "{out:?}");
        assert!(out.iter().any(|l| l.starts_with("[ acoustique ]")), "{out:?}");
    }

    #[test]
    fn analyze_free_song_still_scores_novelty() {
        // the web always appends the novelty report, so a free song gets the
        // fallback line only when nothing else (even novelty) was produced.
        let out = analyze("lead: c3 e3 g3");
        assert!(out.iter().any(|l| l.starts_with("[ nouveauté ]")), "{out:?}");
        assert!(!out.iter().any(|l| l.contains("morceau libre")), "novelty is non-empty, so no fallback: {out:?}");
    }

    #[test]
    fn composed_meter_is_spelled_out_in_the_why_line() {
        // `meter: 3+2+2/8` is 7/8 split 3+2+2: the analyser names the groups.
        let out = analyze("meter: 3+2+2/8\nkick: x x x x x x x\nbell: 1 2 3");
        let joined = out.join("\n");
        assert!(joined.contains("métrique composée 3+2+2 (3,5 temps)"), "{joined}");
        // a plain 7/8 keeps the short form
        let out2 = analyze("meter: 7/8\nkick: x x x x x x x");
        assert!(out2.join("\n").contains("métrique inhabituelle (3,5 temps)"), "{out2:?}");
    }

    #[test]
    fn analyze_skips_comment_lines() {
        let out = analyze("# key: c major\nlead: 1 3 5");
        assert!(!out.iter().any(|l| l.starts_with("[ analyse ] tonalité")), "a commented key is ignored");
        let out2 = analyze("key: c major\nlead: 1 3 5");
        assert!(out2.iter().any(|l| l.starts_with("[ analyse ] tonalité")));
    }

    #[test]
    fn microtonal_custom_key_reported() {
        let out = analyze("key: d 0 1.5 3 5 6.5 8 10\nlead: 1 3 5");
        assert!(out.iter().any(|l| l.contains("microtonal")), "{out:?}");
    }

    #[test]
    fn novelty_without_song_samples_8_bars_like_the_web() {
        // No `song:` -> the web treats the piece as a nominal 8 bars. A dense
        // voice looping identically is then FLAGGED (8 sampled signatures, uniq 1);
        // the old len_bars().max(1) bug sampled the same bar twice (uniq 2) and
        // hid the repetition entirely.
        let out = analyze("kick: x x x x x x x\nbell: 1 2 3");
        let joined = out.join("\n");
        assert!(joined.contains("bouclent à l'identique : kick"), "{joined}");
        assert!(!joined.contains("bouclent à l'identique : bell"), "a 3-step voice is not dense -> not flagged: {joined}");
    }

    #[test]
    fn accel_is_a_reason_in_the_why_line() {
        let out = analyze("key: c major\naccel: 60 140\nlead: 1 3 5");
        assert!(out.iter().any(|l| l.contains("le tempo accélère/respire au fil du morceau")), "{out:?}");
    }
}
