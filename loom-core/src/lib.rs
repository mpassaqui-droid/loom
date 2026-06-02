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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave { Sine, Square, Saw, Triangle }

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

/// Names that introduce a *directive* (key/scale/…), not a playable voice.
const DIRECTIVES: [&str; 2] = ["key", "scale"];

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
        if tokens.is_empty() { continue; }

        let is_drum = DRUMS.contains(&name.as_str());
        let mut wave = if is_drum { Wave::Square } else { Wave::Square };
        let mut gain = 0.22_f32;
        let mut octave = None;
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
            } else {
                verbs.push(seg.to_string());
            }
        }
        out.push(Voice { name, tokens, verbs, wave, gain, is_drum, octave });
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
        for (i, tok) in eff.iter().enumerate() {
            if tok == "." || tok == "~" { continue; }
            // Resolve scale degrees on melodic voices into absolute note names.
            let token = match &voice_key {
                Some(k) if !v.is_drum && is_degree(tok) => {
                    degree_to_midi(tok, k).map(midi_to_name).unwrap_or_else(|| tok.clone())
                }
                _ => tok.clone(),
            };
            events.push(Event {
                phase: i as f32 / n as f32,
                voice: v.name.clone(),
                token,
                wave: v.wave,
                gain: v.gain,
                is_drum: v.is_drum,
            });
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
    fn schedule_resolves_degrees_into_note_names() {
        let v = parse("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let k = parse_key("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // hits at degrees 1,3,5,3 → c3, eb3, g3, eb3
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c3", "eb3", "g3", "eb3"]);
    }
}
