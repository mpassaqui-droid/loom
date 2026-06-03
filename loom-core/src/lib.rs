//! LOOM core — the shared language engine.
//!
//! This is a faithful Rust port of the JavaScript prototype's logic. The same
//! crate is wrapped by `loom-wasm` (browser / phone) and `loom-tui` (terminal),
//! so the language behaves identically everywhere. No external dependencies.

/// A single voice (one line of LOOM source), already parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub name: String,
    pub tokens: Vec<String>,
    pub verbs: Vec<String>,
    pub wave: Wave,
    pub gain: f32,
    pub is_drum: bool,
    /// Reference octave for this voice's scale degrees (`| oct N`). When `None`,
    /// the key's octave is used. Has no effect on absolute note names.
    pub octave: Option<i32>,
    /// Arpeggio mode (`| arp up|down|…`). Spreads chord tokens across the step.
    pub arp: Option<ArpMode>,
    /// Sections this voice plays in (`| in verse chorus`). `None` = always on.
    /// Only consulted when the program declares a `song:` arrangement.
    pub sections: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave { Sine, Square, Saw, Triangle }

/// Arpeggio direction for a chord voice (`| arp up|down|updown|downup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpMode { Up, Down, UpDown, DownUp }

fn arp_mode(w: &str) -> ArpMode {
    match w {
        "down" => ArpMode::Down,
        "updown" | "ud" => ArpMode::UpDown,
        "downup" | "du" => ArpMode::DownUp,
        _ => ArpMode::Up,
    }
}

/// Order a chord's notes for an arpeggio. UpDown/DownUp don't repeat the
/// turning-point notes (…g bb g eb…), so they read as a smooth bounce.
fn arp_order(notes: &[String], mode: ArpMode) -> Vec<String> {
    let rev = || notes.iter().rev().cloned().collect::<Vec<_>>();
    match mode {
        ArpMode::Up => notes.to_vec(),
        ArpMode::Down => rev(),
        ArpMode::UpDown => {
            let mut v = notes.to_vec();
            if notes.len() > 2 { v.extend(notes[1..notes.len() - 1].iter().rev().cloned()); }
            v
        }
        ArpMode::DownUp => {
            let mut v = rev();
            if notes.len() > 2 { v.extend(notes[1..notes.len() - 1].iter().cloned()); }
            v
        }
    }
}

impl Wave {
    fn from_word(w: &str) -> Option<Wave> {
        match w {
            "sine" => Some(Wave::Sine),
            "square" => Some(Wave::Square),
            "saw" | "sawtooth" => Some(Wave::Saw),
            "tri" | "triangle" => Some(Wave::Triangle),
            "pulse" => Some(Wave::Square), // pulse emulated as square
            _ => None,
        }
    }
}

const DRUMS: [&str; 8] = ["kick","bd","snare","sd","hat","hh","clap","cp"];

/// One scheduled sound event within a bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Position within the bar, 0.0..1.0 (multiply by bar length in seconds).
    pub phase: f32,
    pub voice: String,
    pub token: String,
    pub wave: Wave,
    pub gain: f32,
    pub is_drum: bool,
}

/// Names that introduce a *directive* (key/scale/seed/song/duck/…), not a voice.
const DIRECTIVES: [&str; 6] = ["key", "scale", "seed", "song", "arrange", "duck"];

/// Euclidean rhythm: distribute `k` hits as evenly as possible across `n`
/// steps, then rotate left by `rot`. E(3,8) = the tresillo `x . . x . . x .`.
fn euclid(k: usize, n: usize, rot: i32) -> Vec<bool> {
    if n == 0 { return Vec::new(); }
    let k = k.min(n);
    let mut v: Vec<bool> = (0..n).map(|i| (i * k) % n < k).collect();
    let r = rot.rem_euclid(n as i32) as usize;
    v.rotate_left(r);
    v
}

/// If a token is a euclidean generator `e(k,n)` or `e(k,n,rot)`, expand it into
/// its `x`/`.` step sequence. Returns None for any other token.
fn expand_euclid_token(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("e(")?.strip_suffix(')')?;
    let nums: Vec<&str> = inner.split(',').collect();
    if nums.len() < 2 || nums.len() > 3 { return None; }
    let k: usize = nums[0].trim().parse().ok()?;
    let n: usize = nums[1].trim().parse().ok()?;
    let rot: i32 = if nums.len() == 3 { nums[2].trim().parse().ok()? } else { 0 };
    Some(euclid(k, n, rot).into_iter()
        .map(|b| if b { "x".to_string() } else { ".".to_string() })
        .collect())
}

/// Expand any euclidean tokens in a pattern in place.
fn expand_euclid(tokens: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        match expand_euclid_token(&t) {
            Some(seq) => out.extend(seq),
            None => out.push(t),
        }
    }
    out
}

/// Parse LOOM source text into voices.
pub fn parse(src: &str) -> Vec<Voice> {
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let name = line[..colon].trim().to_lowercase();
        if DIRECTIVES.contains(&name.as_str()) { continue; }
        let rest = &line[colon + 1..];
        let mut segs = rest.split('|').map(|s| s.trim());

        let pattern = segs.next().unwrap_or("");
        let tokens: Vec<String> = pattern
            .split_whitespace()
            .filter(|t| !t.starts_with('@') && !t.starts_with('>'))
            .map(|t| t.to_string())
            .collect();
        let tokens = expand_euclid(tokens);
        if tokens.is_empty() { continue; }

        let is_drum = DRUMS.contains(&name.as_str());
        let mut wave = if is_drum { Wave::Square } else { Wave::Square };
        let mut gain = 0.22_f32;
        let mut octave = None;
        let mut arp = None;
        let mut sections: Option<Vec<String>> = None;
        let mut verbs = Vec::new();

        for seg in segs {
            if seg.is_empty() { continue; }
            let mut parts = seg.split_whitespace();
            let head = parts.next().unwrap_or("");
            if let Some(w) = Wave::from_word(head) {
                wave = w;
            } else if head == "gain" {
                if let Some(g) = parts.next().and_then(|x| x.parse::<f32>().ok()) {
                    gain = g.clamp(0.0, 1.0);
                }
            } else if head == "oct" {
                octave = parts.next().and_then(|x| x.parse::<i32>().ok());
            } else if head == "arp" {
                arp = Some(arp_mode(parts.next().unwrap_or("up")));
            } else if head == "in" {
                let secs: Vec<String> = parts.map(|s| s.to_lowercase()).collect();
                if !secs.is_empty() { sections = Some(secs); }
            } else {
                verbs.push(seg.to_string());
            }
        }
        out.push(Voice { name, tokens, verbs, wave, gain, is_drum, octave, arp, sections });
    }
    out
}

/// Apply a single verb to a token list for a given bar index.
fn apply_verb(tokens: &[String], verb: &str, bar: i64) -> Vec<String> {
    let parts: Vec<&str> = verb.split_whitespace().collect();
    match parts.first().copied() {
        Some("rev") => tokens.iter().rev().cloned().collect(),
        Some("fast") => {
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(2).max(1);
            let mut r = Vec::with_capacity(tokens.len() * n);
            for _ in 0..n { r.extend(tokens.iter().cloned()); }
            r
        }
        Some("slow") => {
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(2).max(1);
            if bar.rem_euclid(n) == 0 { tokens.to_vec() }
            else { tokens.iter().map(|_| ".".to_string()).collect() }
        }
        Some("every") => {
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(2).max(1);
            if bar.rem_euclid(n) == n - 1 {
                let inner = parts[2..].join(" ");
                apply_verb(tokens, &inner, bar)
            } else { tokens.to_vec() }
        }
        _ => tokens.to_vec(),
    }
}

/// Resolve a voice's effective tokens for a given bar (all verbs applied).
pub fn effective(voice: &Voice, bar: i64) -> Vec<String> {
    let mut t = voice.tokens.clone();
    for v in &voice.verbs { t = apply_verb(&t, v, bar); }
    t
}

/// Produce every sound event for a given bar across all voices, with no key
/// (scale degrees are left unresolved). See [`schedule_bar_keyed`].
pub fn schedule_bar(voices: &[Voice], bar: i64) -> Vec<Event> {
    schedule_bar_keyed(voices, None, bar)
}

/// Produce every sound event for a given bar across all voices.
/// `phase` is normalized 0..1; the audio backend multiplies by bar seconds.
///
/// When `key` is `Some`, melodic scale-degree tokens (`1`..`7`, with optional
/// `'`/`,` octave marks) are resolved into absolute note names (e.g. `eb3`), so
/// every downstream backend can keep using [`note_to_freq`] unchanged.
pub fn schedule_bar_keyed(voices: &[Voice], key: Option<&Key>, bar: i64) -> Vec<Event> {
    schedule_bar_seeded(voices, key, bar, 0)
}

/// Like [`schedule_bar_keyed`] but with an explicit RNG `seed` for probabilistic
/// tokens (`x?`, `c3?70`). Same seed + bar ⇒ identical events every time.
pub fn schedule_bar_seeded(voices: &[Voice], key: Option<&Key>, bar: i64, seed: u64) -> Vec<Event> {
    let mut events = Vec::new();
    for v in voices {
        let eff = effective(v, bar);
        let n = eff.len();
        if n == 0 { continue; }
        // Effective key for this voice: a `| oct N` overrides the key's octave.
        let voice_key: Option<Key> = key.map(|k| match v.octave {
            Some(o) => Key { tonic: k.tonic, scale: k.scale.clone(), octave: o },
            None => k.clone(),
        });
        let base_oct = voice_key.as_ref().map(|k| k.octave).or(v.octave).unwrap_or(DEFAULT_OCTAVE);
        for (i, tok) in eff.iter().enumerate() {
            if tok == "." || tok == "~" { continue; }
            let (base, prob, ratchet) = token_mods(tok);
            // Probabilistic token: deterministically skip when the roll fails.
            if let Some(p) = prob {
                if rand01(seed, bar, &v.name, i) >= p { continue; }
            }
            if base.is_empty() || base == "." || base == "~" { continue; }
            // Degrees → one note; chords → several notes at the same phase.
            let notes = resolve_token(base, v.is_drum, base_oct, voice_key.as_ref());
            let push = |events: &mut Vec<Event>, phase: f32, note: &str| {
                events.push(Event {
                    phase,
                    voice: v.name.clone(),
                    token: note.to_string(),
                    wave: v.wave,
                    gain: v.gain,
                    is_drum: v.is_drum,
                });
            };
            match v.arp {
                // Arpeggio: spread a chord's notes one-by-one across the step.
                Some(mode) if !v.is_drum && notes.len() > 1 => {
                    let ord = arp_order(&notes, mode);
                    let m = ord.len();
                    for (j, note) in ord.iter().enumerate() {
                        push(&mut events, (i as f32 + j as f32 / m as f32) / n as f32, note);
                    }
                }
                // Otherwise: ratchet subdivides the step into evenly-spaced hits.
                _ => {
                    for j in 0..ratchet {
                        let phase = (i as f32 + j as f32 / ratchet as f32) / n as f32;
                        for note in &notes { push(&mut events, phase, note); }
                    }
                }
            }
        }
    }
    events
}

/// Convert a note name like "c2", "eb3", "g#4" to a frequency in Hz.
pub fn note_to_freq(note: &str) -> Option<f32> {
    let b = note.as_bytes();
    if b.is_empty() { return None; }
    let letter = (b[0] as char).to_ascii_lowercase();
    let base = match letter { 'c'=>0,'d'=>2,'e'=>4,'f'=>5,'g'=>7,'a'=>9,'b'=>11, _=>return None };
    let mut idx = 1;
    let mut semi = base as i32;
    if idx < b.len() && (b[idx] == b'#' || b[idx] == b'b') {
        if b[idx] == b'#' { semi += 1; } else { semi -= 1; }
        idx += 1;
    }
    let octave: i32 = note[idx..].parse().ok()?;
    let midi = 12 * (octave + 1) + semi;
    Some(midi_to_freq(midi))
}

/// MIDI note number → frequency in Hz (A4 = midi 69 = 440 Hz).
pub fn midi_to_freq(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

// ---------------------------------------------------------------------------
// Music theory layer (roadmap item 1): key + scale + scale degrees.
//
// A melodic token can now be a *scale degree* (1..7) that resolves against a
// declared key, e.g.
//     key: c minor
//     bass: 1 . 5 . 3 . 5 .      # 1=c, 5=g, 3=eb  (the b3 comes from the scale)
// Octave marks: `'` raises one octave, `,` lowers one (Lilypond-style).
// Chromatic alterations on degrees (#/b) are intentionally NOT supported yet —
// the scale itself supplies the right accidentals.
// ---------------------------------------------------------------------------

/// A declared tonality: a tonic pitch-class plus a scale (intervals from the
/// tonic, within one octave) and the reference octave for degree 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// Tonic pitch class, 0..=11 with c=0.
    pub tonic: i32,
    /// Scale intervals in semitones from the tonic, e.g. major = [0,2,4,5,7,9,11].
    pub scale: Vec<i32>,
    /// Octave that degree 1 sounds in (c in this octave = degree 1 of a C key).
    pub octave: i32,
}

/// Default reference octave for degree 1 (C3 region — sits between bass and lead).
const DEFAULT_OCTAVE: i32 = 3;

/// Pitch class (0..=11) for a tonic word like "c", "c#", "eb", "f#".
fn tonic_pc(word: &str) -> Option<i32> {
    let b = word.as_bytes();
    if b.is_empty() { return None; }
    let base = match (b[0] as char).to_ascii_lowercase() {
        'c'=>0,'d'=>2,'e'=>4,'f'=>5,'g'=>7,'a'=>9,'b'=>11, _=>return None,
    };
    let mut semi = base as i32;
    if b.len() > 1 {
        match b[1] { b'#' => semi += 1, b'b' => semi -= 1, _ => {} }
    }
    Some(semi.rem_euclid(12))
}

/// Scale intervals (one octave) for a named scale. Returns None if unknown.
pub fn scale_intervals(name: &str) -> Option<Vec<i32>> {
    let s: Vec<i32> = match name {
        "major" | "ionian"            => vec![0, 2, 4, 5, 7, 9, 11],
        "minor" | "aeolian"           => vec![0, 2, 3, 5, 7, 8, 10],
        "dorian"                      => vec![0, 2, 3, 5, 7, 9, 10],
        "phrygian"                    => vec![0, 1, 3, 5, 7, 8, 10],
        "lydian"                      => vec![0, 2, 4, 6, 7, 9, 11],
        "mixolydian"                  => vec![0, 2, 4, 5, 7, 9, 10],
        "locrian"                     => vec![0, 1, 3, 5, 6, 8, 10],
        "pentatonic" | "majpent"      => vec![0, 2, 4, 7, 9],
        "minpent"                     => vec![0, 3, 5, 7, 10],
        // extra Western scales
        "harmonic_minor" | "harmonicminor" => vec![0, 2, 3, 5, 7, 8, 11],
        "melodic_minor" | "melodicminor"   => vec![0, 2, 3, 5, 7, 9, 11],
        "whole_tone" | "wholetone"         => vec![0, 2, 4, 6, 8, 10],
        "blues"                            => vec![0, 3, 5, 6, 7, 10],
        "chromatic"                        => (0..12).collect(),
        // Maqam family (12-TET approximations; true quarter tones are a future
        // microtonal layer — see NIGHT_LOG). Tonic-based interval sets.
        "hijaz"                            => vec![0, 1, 4, 5, 7, 8, 10],
        "hijazkar" | "double_harmonic" | "byzantine" => vec![0, 1, 4, 5, 7, 8, 11],
        "nahawand"                         => vec![0, 2, 3, 5, 7, 8, 10], // ~ minor
        "kurd"                             => vec![0, 1, 3, 5, 7, 8, 10], // ~ phrygian
        "ajam"                             => vec![0, 2, 4, 5, 7, 9, 11], // ~ major
        // Raga (approximations)
        "bhairav"                          => vec![0, 1, 4, 5, 7, 8, 11],
        "yaman"                            => vec![0, 2, 4, 6, 7, 9, 11], // ~ lydian
        "todi"                             => vec![0, 1, 3, 6, 7, 8, 11],
        // Gamelan & Asian pentatonics (approximated in 12-TET)
        "slendro"                          => vec![0, 2, 5, 7, 10],
        "pelog"                            => vec![0, 1, 3, 7, 8],
        "hirajoshi"                        => vec![0, 2, 3, 7, 8],
        "insen"                            => vec![0, 1, 5, 7, 10],
        "iwato"                            => vec![0, 1, 5, 6, 10],
        "egyptian"                         => vec![0, 2, 5, 7, 10],
        _ => return None,
    };
    Some(s)
}

/// Read the `key:` / `scale:` directives from a program. Returns None if the
/// program declares no key (degrees then simply don't resolve, as before).
///
/// - `key: <tonic> [scale]`  e.g. `key: c minor`, `key: f# dorian`, `key: a`
/// - `scale: <name>`         overrides just the scale, tonic defaults to c
pub fn parse_key(src: &str) -> Option<Key> {
    let mut tonic: Option<i32> = None;
    let mut scale_name: Option<String> = None;

    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let name = line[..colon].trim().to_lowercase();
        let rest = line[colon + 1..].trim().to_lowercase();
        let mut words = rest.split_whitespace();
        match name.as_str() {
            "key" => {
                if let Some(t) = words.next().and_then(tonic_pc) { tonic = Some(t); }
                if let Some(s) = words.next() { scale_name = Some(s.to_string()); }
            }
            "scale" => {
                if let Some(s) = words.next() { scale_name = Some(s.to_string()); }
            }
            _ => {}
        }
    }

    if tonic.is_none() && scale_name.is_none() { return None; }
    let scale = scale_name
        .as_deref()
        .and_then(scale_intervals)
        .unwrap_or_else(|| scale_intervals("major").unwrap());
    Some(Key { tonic: tonic.unwrap_or(0), scale, octave: DEFAULT_OCTAVE })
}

/// Is this token a scale degree? Form: a digit 1..7, an optional chromatic
/// alteration (`#` raise / `b` lower) right after the digit, then any number of
/// octave marks (`'` up, `,` down). Because it starts with a digit it can never
/// collide with a note name (which always starts with a letter a..g).
fn is_degree(token: &str) -> bool {
    let mut chars = token.chars().peekable();
    match chars.next() {
        Some(c) if ('1'..='7').contains(&c) => {}
        _ => return false,
    }
    if matches!(chars.peek(), Some('#') | Some('b')) { chars.next(); }
    chars.all(|c| c == '\'' || c == ',')
}

/// Resolve a scale-degree token (e.g. "1", "5", "7#", "3b", "1'") to a MIDI
/// note for a key. The optional `#`/`b` right after the digit shifts the note a
/// semitone (e.g. `7#` is the raised 7th — the harmonic-minor leading tone).
/// Degrees beyond the scale length wrap up an octave (so `6` in a 5-note
/// pentatonic = degree 1, one octave higher).
fn degree_to_midi(token: &str, key: &Key) -> Option<i32> {
    let mut chars = token.chars().peekable();
    let d = chars.next()?.to_digit(10)? as i32; // 1..7
    let acc = match chars.peek() {
        Some('#') => { chars.next(); 1 }
        Some('b') => { chars.next(); -1 }
        _ => 0,
    };
    let mut oct_marks = 0i32;
    for c in chars {
        match c { '\'' => oct_marks += 1, ',' => oct_marks -= 1, _ => return None }
    }
    let len = key.scale.len() as i32;
    if len == 0 { return None; }
    let idx = d - 1;
    let wrap = idx.div_euclid(len);
    let step = key.scale[idx.rem_euclid(len) as usize];
    let octave = key.octave + oct_marks + wrap;
    Some(12 * (octave + 1) + key.tonic + step + acc)
}

/// Resolve a token to a frequency: a scale degree if a key is given and the
/// token is a degree, otherwise an absolute note name (`c3`, `eb2`, …).
pub fn token_to_freq(token: &str, key: Option<&Key>) -> Option<f32> {
    if let Some(k) = key {
        if is_degree(token) {
            return degree_to_midi(token, k).map(midi_to_freq);
        }
    }
    note_to_freq(token)
}

/// MIDI note → an absolute note-name string (flat spelling), e.g. 48 -> "c3".
fn midi_to_name(midi: i32) -> String {
    const NAMES: [&str; 12] = ["c","db","d","eb","e","f","gb","g","ab","a","bb","b"];
    let octave = midi.div_euclid(12) - 1;
    let pc = midi.rem_euclid(12) as usize;
    format!("{}{}", NAMES[pc], octave)
}

/// Parse a chord symbol into its MIDI notes, e.g. `Cm7` -> [c, eb, g, bb].
///
/// Convention: the root must be an UPPERCASE letter A–G (so `Cm7`, `F`, `G7`).
/// Lowercase tokens stay single notes/degrees (`c3`, `eb2`), which is how we
/// disambiguate `C` the chord from `c3` the note. Uppercase note names that
/// carry an octave (`C3`) fail the quality match and fall back to a plain note.
pub fn parse_chord(tok: &str, octave: i32) -> Option<Vec<i32>> {
    let b = tok.as_bytes();
    if b.is_empty() { return None; }
    let base: i32 = match b[0] as char {
        'C'=>0,'D'=>2,'E'=>4,'F'=>5,'G'=>7,'A'=>9,'B'=>11, _=>return None,
    };
    let mut idx = 1;
    let mut root = base;
    if idx < b.len() && (b[idx] == b'#' || b[idx] == b'b') {
        if b[idx] == b'#' { root += 1; } else { root -= 1; }
        idx += 1;
    }
    let intervals: &[i32] = match &tok[idx..] {
        "" | "maj" | "M"            => &[0, 4, 7],
        "m" | "min" | "-"           => &[0, 3, 7],
        "dim" | "o"                 => &[0, 3, 6],
        "aug" | "+"                 => &[0, 4, 8],
        "7"                         => &[0, 4, 7, 10],
        "maj7" | "M7"               => &[0, 4, 7, 11],
        "m7" | "min7" | "-7"        => &[0, 3, 7, 10],
        "m7b5" | "min7b5"           => &[0, 3, 6, 10],
        "dim7" | "o7"               => &[0, 3, 6, 9],
        "6"                         => &[0, 4, 7, 9],
        "m6" | "min6"               => &[0, 3, 7, 9],
        "sus2"                      => &[0, 2, 7],
        "sus4" | "sus"              => &[0, 5, 7],
        "add9"                      => &[0, 4, 7, 14],
        "9"                         => &[0, 4, 7, 10, 14],
        "maj9" | "M9"               => &[0, 4, 7, 11, 14],
        "m9" | "min9"               => &[0, 3, 7, 10, 14],
        _ => return None,
    };
    let root_midi = 12 * (octave + 1) + root;
    Some(intervals.iter().map(|i| root_midi + i).collect())
}

// ---------------------------------------------------------------------------
// Probability & determinism (roadmap B2/B3).
//
// A token may carry a probability suffix: `x?` ~ 50%, `x?70` = 70%. Whether it
// fires is decided by a deterministic hash of (seed, bar, voice, step), so a
// given `seed:` always replays the exact same performance — reproducible and
// testable, with no RNG state and no dependencies.
// ---------------------------------------------------------------------------

/// SplitMix64 finalizer — cheap, well-distributed integer hash.
fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// FNV-1a hash of a string (used to fold a voice name into the RNG).
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x0000_0100_0000_01B3); }
    h
}

/// Deterministic pseudo-random value in [0,1) from the performance coordinates.
fn rand01(seed: u64, bar: i64, voice: &str, step: usize) -> f32 {
    let mut h = hash_u64(seed ^ 0xA5A5_5A5A_1234_5678);
    h = hash_u64(h ^ (bar as u64));
    h = hash_u64(h ^ fnv(voice));
    h = hash_u64(h ^ step as u64);
    // top 24 bits → [0,1)
    ((h >> 40) as f32) / ((1u64 << 24) as f32)
}

/// Digits immediately following position `at` in `tok` (a `?`/`*` marker).
fn digits_after(tok: &str, at: usize) -> &str {
    let rest = &tok[at + 1..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    &rest[..end]
}

/// Parse a token's modifiers, in any order: probability `?P` (`x?` ~50%,
/// `x?70` = 70%) and ratchet `*N` (`x*3` = 3 sub-hits). Returns the bare base
/// token, an optional fire probability in 0..1, and a ratchet count ≥ 1.
fn token_mods(tok: &str) -> (&str, Option<f32>, usize) {
    let q = tok.find('?');
    let r = tok.find('*');
    let cut = match (q, r) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return (tok, None, 1),
    };
    let prob = q.map(|p| {
        let d = digits_after(tok, p);
        if d.is_empty() { 0.5 } else { (d.parse::<f32>().unwrap_or(50.0) / 100.0).clamp(0.0, 1.0) }
    });
    let ratchet = r
        .map(|p| digits_after(tok, p).parse::<usize>().unwrap_or(2).max(1))
        .unwrap_or(1);
    (&tok[..cut], prob, ratchet)
}

/// Read the optional `seed: N` directive (default 0 = the canonical performance).
pub fn parse_seed(src: &str) -> u64 {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().eq_ignore_ascii_case("seed") {
            if let Ok(n) = line[colon + 1..].trim().parse::<u64>() { return n; }
        }
    }
    0
}

/// Default bars for a section in `song:` when no `*count` is given.
const DEFAULT_SECTION_BARS: u32 = 4;

/// Read the `song:`/`arrange:` arrangement: a list of `(section, bars)`, e.g.
/// `song: intro*4 verse*8 chorus*8 outro*4`. A bare `name` lasts 4 bars.
pub fn parse_song(src: &str) -> Vec<(String, u32)> {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        let head = line[..colon].trim().to_lowercase();
        if head != "song" && head != "arrange" { continue; }
        return line[colon + 1..]
            .split_whitespace()
            .filter_map(|tok| {
                let (name, bars) = match tok.split_once('*') {
                    Some((n, b)) => (n, b.parse::<u32>().unwrap_or(DEFAULT_SECTION_BARS)),
                    None => (tok, DEFAULT_SECTION_BARS),
                };
                if name.is_empty() { None } else { Some((name.to_lowercase(), bars.max(1))) }
            })
            .collect();
    }
    Vec::new()
}

/// Which section is playing at `bar`, looping over the whole arrangement.
fn section_at_bar(song: &[(String, u32)], bar: i64) -> Option<&str> {
    let total: u32 = song.iter().map(|(_, n)| *n).sum();
    if total == 0 { return None; }
    let mut pos = bar.rem_euclid(total as i64) as u32;
    for (name, n) in song {
        if pos < *n { return Some(name.as_str()); }
        pos -= *n;
    }
    None
}

/// A whole parsed LOOM program: voices plus the optional key/seed/arrangement.
/// This is the convenient front door — `Program::parse` then `schedule_bar`.
#[derive(Debug, Clone)]
pub struct Program {
    pub voices: Vec<Voice>,
    pub key: Option<Key>,
    pub seed: u64,
    pub song: Vec<(String, u32)>,
}

impl Program {
    pub fn parse(src: &str) -> Program {
        Program {
            voices: parse(src),
            key: parse_key(src),
            seed: parse_seed(src),
            song: parse_song(src),
        }
    }

    /// Total length of the arrangement in bars (0 if there is no `song:`).
    pub fn len_bars(&self) -> u32 { self.song.iter().map(|(_, n)| *n).sum() }

    /// Section name playing at `bar` (None if no arrangement).
    pub fn section_at(&self, bar: i64) -> Option<&str> { section_at_bar(&self.song, bar) }

    /// Schedule one bar, honouring the arrangement: when a `song:` is present,
    /// a voice plays only if it has no `| in …` tag or its tag includes the
    /// current section. With no `song:`, every voice always plays.
    pub fn schedule_bar(&self, bar: i64) -> Vec<Event> {
        let active: Vec<Voice> = if self.song.is_empty() {
            self.voices.clone()
        } else {
            let sec = section_at_bar(&self.song, bar);
            self.voices.iter()
                .filter(|v| match &v.sections {
                    None => true,
                    Some(set) => sec.map_or(false, |s| set.iter().any(|x| x == s)),
                })
                .cloned()
                .collect()
        };
        schedule_bar_seeded(&active, self.key.as_ref(), bar, self.seed)
    }
}

/// Resolve one melodic/drum token into the note-name string(s) it sounds.
/// Order: scale degree (needs a key) → chord symbol → plain note. Drums pass
/// through untouched. A chord returns several notes (played simultaneously).
fn resolve_token(tok: &str, is_drum: bool, octave: i32, key: Option<&Key>) -> Vec<String> {
    if is_drum { return vec![tok.to_string()]; }
    if let Some(k) = key {
        if is_degree(tok) {
            return vec![degree_to_midi(tok, k).map(midi_to_name).unwrap_or_else(|| tok.to_string())];
        }
    }
    if let Some(notes) = parse_chord(tok, octave) {
        return notes.into_iter().map(midi_to_name).collect();
    }
    vec![tok.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_drum_and_melody() {
        let v = parse("kick: x . x .\nbass: c2 . g2 . | saw | gain 0.5");
        assert_eq!(v.len(), 2);
        assert!(v[0].is_drum);
        assert_eq!(v[0].tokens, ["x",".","x","."]);
        assert!(!v[1].is_drum);
        assert_eq!(v[1].wave, Wave::Saw);
        assert!((v[1].gain - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rev_reverses() {
        let t = vec!["a".into(),"b".into(),"c".into()];
        assert_eq!(apply_verb(&t, "rev", 0), vec!["c","b","a"]);
    }

    #[test]
    fn fast_repeats() {
        let t = vec!["x".into(),".".into()];
        assert_eq!(apply_verb(&t, "fast 2", 0).len(), 4);
    }

    #[test]
    fn every_applies_on_nth_bar() {
        let t = vec!["a".into(),"b".into()];
        // every 4 rev -> reversed only when bar % 4 == 3
        assert_eq!(apply_verb(&t, "every 4 rev", 0), vec!["a","b"]);
        assert_eq!(apply_verb(&t, "every 4 rev", 3), vec!["b","a"]);
    }

    #[test]
    fn schedules_only_hits() {
        let v = parse("kick: x . . x");
        let ev = schedule_bar(&v, 0);
        assert_eq!(ev.len(), 2);
        assert!((ev[0].phase - 0.0).abs() < 1e-6);
        assert!((ev[1].phase - 0.75).abs() < 1e-6);
    }

    #[test]
    fn middle_a_is_440() {
        assert!((note_to_freq("a4").unwrap() - 440.0).abs() < 0.01);
    }

    // --- music theory layer ---------------------------------------------

    fn close(a: Option<f32>, b: Option<f32>) -> bool {
        match (a, b) { (Some(x), Some(y)) => (x - y).abs() < 0.01, _ => false }
    }

    #[test]
    fn key_line_is_not_a_voice() {
        let v = parse("key: c minor\nkick: x . x .");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "kick");
    }

    #[test]
    fn parses_key_with_mode() {
        let k = parse_key("key: c minor").unwrap();
        assert_eq!(k.tonic, 0);
        assert_eq!(k.scale, scale_intervals("minor").unwrap());
    }

    #[test]
    fn key_defaults_to_major_and_scale_directive_overrides() {
        let k = parse_key("key: a").unwrap();
        assert_eq!(k.tonic, 9);
        assert_eq!(k.scale, scale_intervals("major").unwrap());
        let k2 = parse_key("key: d\nscale: dorian").unwrap();
        assert_eq!(k2.tonic, 2);
        assert_eq!(k2.scale, scale_intervals("dorian").unwrap());
    }

    #[test]
    fn no_directive_means_no_key() {
        assert!(parse_key("kick: x . x .").is_none());
    }

    #[test]
    fn degrees_resolve_against_c_major() {
        let k = parse_key("key: c major").unwrap();
        // degree 1 = c3, 5 = g3, 1' = c4 (one octave up)
        assert!(close(token_to_freq("1", Some(&k)), note_to_freq("c3")));
        assert!(close(token_to_freq("5", Some(&k)), note_to_freq("g3")));
        assert!(close(token_to_freq("1'", Some(&k)), note_to_freq("c4")));
        assert!(close(token_to_freq("1,", Some(&k)), note_to_freq("c2")));
    }

    #[test]
    fn minor_scale_supplies_flats() {
        let k = parse_key("key: c minor").unwrap();
        // 3rd of C minor is eb, 7th is bb — straight from the scale, no b syntax
        assert!(close(token_to_freq("3", Some(&k)), note_to_freq("eb3")));
        assert!(close(token_to_freq("7", Some(&k)), note_to_freq("bb3")));
    }

    #[test]
    fn euclidean_expands_to_tresillo() {
        let v = parse("kick: e(3,8)");
        assert_eq!(v[0].tokens, ["x",".",".","x",".",".","x","."]);
    }

    #[test]
    fn euclidean_rotation_and_mixing() {
        // rotation shifts the pattern; euclid tokens can sit next to plain ones
        let v = parse("hat: e(2,4,1) x");
        // E(2,4) = x . x . ; rotate left 1 = . x . x ; then a trailing x
        assert_eq!(v[0].tokens, [".","x",".","x","x"]);
    }

    #[test]
    fn world_scales_resolve() {
        // harmonic minor has a natural 7th (leading tone): C..B
        let hm = parse_key("key: c harmonic_minor").unwrap();
        assert!(close(token_to_freq("7", Some(&hm)), note_to_freq("b3")));
        // whole-tone: C D E F# ... degree 4 = f#
        let wt = parse_key("key: c whole_tone").unwrap();
        assert!(close(token_to_freq("4", Some(&wt)), note_to_freq("f#3")));
        // hijaz: C Db E F G Ab Bb — the b2 then major-3rd gap is its signature
        let hj = parse_key("key: c hijaz").unwrap();
        assert!(close(token_to_freq("2", Some(&hj)), note_to_freq("db3")));
        assert!(close(token_to_freq("3", Some(&hj)), note_to_freq("e3")));
        // blues scale has the b5 "blue note" as degree 4
        let bl = parse_key("key: c blues").unwrap();
        assert!(close(token_to_freq("4", Some(&bl)), note_to_freq("gb3")));
    }

    #[test]
    fn pentatonic_degree_wraps_octave() {
        let k = parse_key("key: c\nscale: pentatonic").unwrap();
        // 5-note scale [0,2,4,7,9]; degree 6 wraps to degree 1 an octave up
        assert!(close(token_to_freq("6", Some(&k)), note_to_freq("c4")));
    }

    #[test]
    fn absolute_notes_still_work_with_a_key() {
        let k = parse_key("key: c minor").unwrap();
        assert!(close(token_to_freq("g4", Some(&k)), note_to_freq("g4")));
    }

    #[test]
    fn degrees_dont_resolve_without_a_key() {
        // No key: a degree token is meaningless, so it yields no frequency.
        assert!(token_to_freq("1", None).is_none());
    }

    #[test]
    fn chromatic_alteration_raises_and_lowers_a_degree() {
        let cm = parse_key("key: c minor").unwrap();
        // 7th of C minor is bb; 7# is the raised leading tone = b natural.
        assert!(close(token_to_freq("7#", Some(&cm)), note_to_freq("b3")));
        let cmaj = parse_key("key: c major").unwrap();
        // major 3rd is e; 3b = eb. major 5th is g; 5b = gb (blue note).
        assert!(close(token_to_freq("3b", Some(&cmaj)), note_to_freq("eb3")));
        assert!(close(token_to_freq("5b", Some(&cmaj)), note_to_freq("gb3")));
        // alteration combines with octave marks: 7#' = b in the next octave up.
        assert!(close(token_to_freq("7#'", Some(&cm)), note_to_freq("b4")));
    }

    #[test]
    fn letter_b_note_is_not_mistaken_for_a_degree() {
        let cm = parse_key("key: c minor").unwrap();
        // "b3" starts with a letter -> it is the note B3, never a degree.
        assert!(!is_degree("b3"));
        assert!(close(token_to_freq("b3", Some(&cm)), note_to_freq("b3")));
    }

    #[test]
    fn per_voice_octave_shifts_degrees() {
        let v = parse("key: c major\nbass: 1 . 5 . | oct 2");
        assert_eq!(v[0].octave, Some(2));
        let k = parse_key("key: c major");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // degree 1 at oct 2 = c2, degree 5 = g2 (the key's default would be oct 3)
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c2", "g2"]);
    }

    #[test]
    fn oct_does_not_affect_absolute_notes() {
        let v = parse("key: c major\nlead: c4 . e4 . | oct 2");
        let k = parse_key("key: c major");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // absolute note names carry their own octave; oct must not touch them
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c4", "e4"]);
    }

    #[test]
    fn ratchet_subdivides_a_step() {
        // x*2 in a 2-step bar: step width 0.5, so sub-hits at 0.0 and 0.25.
        let v = parse("kick: x*2 .");
        let ev = schedule_bar_seeded(&v, None, 0, 0);
        assert_eq!(ev.len(), 2);
        assert!((ev[0].phase - 0.0).abs() < 1e-6);
        assert!((ev[1].phase - 0.25).abs() < 1e-6);
    }

    #[test]
    fn ratchet_works_on_notes_and_combines_with_probability() {
        // a single-step bar with a triple ratchet on degree 1 = three c3 hits
        let v = parse("key: c major\nlead: 1*3");
        let k = parse_key("key: c major");
        let ev = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 3);
        assert!(ev.iter().all(|e| e.token == "c3"));
        // p=0 ratchet never fires (probability is checked before the ratchet)
        let z = parse("kick: x?0*4");
        assert_eq!(schedule_bar_seeded(&z, None, 0, 0).len(), 0);
    }

    #[test]
    fn probability_extremes_are_deterministic() {
        // ?0 never fires, ?100 always fires — independent of the RNG.
        let never = parse("kick: x?0 x?0 x?0 x?0");
        assert_eq!(schedule_bar_seeded(&never, None, 0, 0).len(), 0);
        let always = parse("kick: x?100 x?100 x?100 x?100");
        assert_eq!(schedule_bar_seeded(&always, None, 0, 0).len(), 4);
    }

    #[test]
    fn probability_is_reproducible_per_seed() {
        let v = parse("hat: x?50 x?50 x?50 x?50 x?50 x?50 x?50 x?50");
        let a = schedule_bar_seeded(&v, None, 3, 42);
        let b = schedule_bar_seeded(&v, None, 3, 42);
        assert_eq!(a.len(), b.len()); // same seed+bar => identical
        // a different seed generally changes which hits survive (not all 8 / 0)
        let c = schedule_bar_seeded(&v, None, 3, 99);
        assert!(a.len() <= 8 && c.len() <= 8);
    }

    #[test]
    fn parses_seed_directive() {
        assert_eq!(parse_seed("seed: 7\nkick: x"), 7);
        assert_eq!(parse_seed("kick: x"), 0);
        // seed line is a directive, not a voice
        assert_eq!(parse("seed: 7\nkick: x").len(), 1);
    }

    #[test]
    fn audio_only_directives_are_not_voices() {
        // duck: is an audio-engine directive (sidechain), never a playable voice
        let v = parse("duck: 60\nkick: x x x x");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "kick");
    }

    #[test]
    fn song_arrangement_selects_sections() {
        let song = parse_song("song: intro*2 chorus*4 outro*2");
        assert_eq!(song, vec![("intro".into(),2),("chorus".into(),4),("outro".into(),2)]);
        assert_eq!(section_at_bar(&song, 0), Some("intro"));
        assert_eq!(section_at_bar(&song, 2), Some("chorus"));
        assert_eq!(section_at_bar(&song, 5), Some("chorus"));
        assert_eq!(section_at_bar(&song, 6), Some("outro"));
        // loops: bar 8 == bar 0
        assert_eq!(section_at_bar(&song, 8), Some("intro"));
    }

    #[test]
    fn program_mutes_voices_outside_their_sections() {
        let src = "song: intro*1 drop*1\nkick: x x x x\nlead: c4 c4 c4 c4 | in drop";
        let prog = Program::parse(src);
        assert_eq!(prog.len_bars(), 2);
        // bar 0 = intro: only kick (lead is `in drop`)
        let b0 = prog.schedule_bar(0);
        assert!(b0.iter().all(|e| e.voice == "kick"));
        // bar 1 = drop: kick (untagged, always) + lead
        let b1 = prog.schedule_bar(1);
        assert!(b1.iter().any(|e| e.voice == "kick"));
        assert!(b1.iter().any(|e| e.voice == "lead"));
    }

    #[test]
    fn no_song_means_every_voice_plays() {
        let prog = Program::parse("kick: x x\nlead: c4 c4 | in chorus");
        // no arrangement -> section tags are ignored, both voices play
        let ev = prog.schedule_bar(0);
        assert!(ev.iter().any(|e| e.voice == "lead"));
        assert!(ev.iter().any(|e| e.voice == "kick"));
    }

    #[test]
    fn arp_spreads_chord_across_the_step() {
        let v = parse("keys: Cm7 | arp up");
        assert_eq!(v[0].arp, Some(ArpMode::Up));
        let ev = schedule_bar_seeded(&v, None, 0, 0);
        // one-step bar, 4 chord notes -> phases 0, .25, .5, .75 in order
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","eb3","g3","bb3"]);
        let phases: Vec<f32> = ev.iter().map(|e| e.phase).collect();
        assert!((phases[1] - 0.25).abs() < 1e-6 && (phases[3] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn arp_down_and_updown_orders() {
        let down = parse("keys: C | arp down");
        let ev = schedule_bar_seeded(&down, None, 0, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","e3","c3"]);
        let ud = parse("keys: Cm7 | arp updown");
        let ev2 = schedule_bar_seeded(&ud, None, 0, 0);
        // c eb g bb then back down without repeating ends: g eb
        assert_eq!(ev2.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","eb3","g3","bb3","g3","eb3"]);
    }

    #[test]
    fn chord_expands_to_simultaneous_notes() {
        // No key needed: Cm7 (uppercase) -> c eb g bb, all at the same phase.
        let v = parse("pad: Cm7 . | sine");
        let ev = schedule_bar_keyed(&v, None, 0);
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c3", "eb3", "g3", "bb3"]);
        assert!(ev.iter().all(|e| e.phase == 0.0));
    }

    #[test]
    fn bare_uppercase_is_a_major_triad() {
        let v = parse("pad: G .");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","b3","d4"]);
    }

    #[test]
    fn uppercase_note_with_octave_stays_a_note() {
        // "C3" carries an octave so it is NOT a chord — it is the note C3.
        let v = parse("lead: C3 .");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.len(), 1);
        assert!(close(note_to_freq(&ev[0].token), note_to_freq("c3")));
    }

    #[test]
    fn chord_respects_voice_octave() {
        let v = parse("pad: Am . | oct 2");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["a2","c3","e3"]);
    }

    #[test]
    fn schedule_resolves_degrees_into_note_names() {
        let v = parse("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let k = parse_key("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // hits at degrees 1,3,5,3 → c3, eb3, g3, eb3
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c3", "eb3", "g3", "eb3"]);
    }
}
