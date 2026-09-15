//! HUM — sing or whistle, LOOM writes the melody.
//!
//! Port of the browser's `humToMelody()` + `autoCorrelate()` + `midiToToken()`
//! (`web/index.html`), as pure functions on recorded samples:
//!
//! 1. [`detect_pitch`] — the web's autocorrelation pitch detector on one frame,
//! 2. [`step_notes`] — cut the recording into 16 eighth-note steps at the
//!    song's tempo and give each step one MIDI note or a rest,
//! 3. [`midi_to_token`] — snap each note to the song's `key:` as a scale
//!    degree (or a note name when there is no key),
//! 4. [`line()`] — the `hum:` voice line, which [`append`] adds to the song.
//!
//! Adapted from the browser: the web read the microphone live (one 2048-sample
//! snapshot at the end of each step, with timers); here the UI records
//! [`Settings::seconds`] of audio first (e.g. with `loom_audio::input::MicInput`)
//! and passes the samples. Each step looks at up to three frames and keeps the
//! median note, which is steadier than one snapshot. The web had no clarity
//! check, so breath and noise came out as random notes; here a frame must be
//! clearly periodic to count as a note.

use loom_core::Key;

/// Samples per analysis frame (the web's `fftSize`).
pub const WINDOW: usize = 2048;
/// Lowest and highest pitch accepted, in Hz (the web's `f>50 && f<1200`).
pub const MIN_HZ: f32 = 50.0;
pub const MAX_HZ: f32 = 1200.0;
/// Below this RMS a frame is silence (the web's gate).
pub const SILENCE_RMS: f32 = 0.01;
/// How alike a frame must be to itself one period later (0..1) to be a note.
pub const MIN_CLARITY: f32 = 0.5;
/// Steps sung, one eighth note each (the web's 16).
pub const STEPS: usize = 16;

/// Tempo and length of a hum take.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub bpm: f64,
    pub steps: usize,
}

impl Settings {
    /// The song's `bpm:` (120 when absent), 16 steps.
    pub fn for_song(src: &str) -> Settings {
        Settings { bpm: loom_core::parse_bpm(src), steps: STEPS }
    }

    /// One step = an eighth note.
    pub fn step_seconds(&self) -> f64 {
        60.0 / self.bpm / 2.0
    }

    /// How long to record.
    pub fn seconds(&self) -> f64 {
        self.step_seconds() * self.steps as f64
    }

    /// What to say while recording: `[ hum ] chante 16 temps… (4.0s)`.
    pub fn prompt(&self) -> String {
        format!("[ hum ] chante {} temps… ({:.1}s)", self.steps, self.seconds())
    }
}

/// A transcribed take.
#[derive(Debug, Clone, PartialEq)]
pub struct Hum {
    /// One MIDI note (or rest) per step.
    pub notes: Vec<Option<i32>>,
    /// The LOOM tokens, e.g. `1 3 . 5'`.
    pub tokens: Vec<String>,
    /// The voice line: `hum: 1 3 . 5' | tri | lp 2500 | reverb 0.35 | gain 0.16`.
    pub line: String,
}

impl Hum {
    /// Nothing was sung (every step is a rest).
    pub fn is_silent(&self) -> bool {
        self.notes.iter().all(Option::is_none)
    }

    /// `[ hum ] mélodie ajoutée : 1 3 . 5'` (or a hint when nothing was heard).
    pub fn message(&self) -> String {
        if self.is_silent() {
            "[ hum ] je n'ai rien entendu — rapproche-toi du micro et chante plus fort".into()
        } else {
            format!("[ hum ] mélodie ajoutée : {}", self.tokens.join(" "))
        }
    }
}

/// The whole pipeline: `samples` (mono, -1..1) recorded at `sample_rate`,
/// quantised to the tempo and key of `song`.
pub fn melody(samples: &[f32], sample_rate: f32, song: &str) -> Hum {
    let settings = Settings::for_song(song);
    let key = loom_core::parse_key(song);
    let notes = step_notes(samples, sample_rate, settings);
    let tokens: Vec<String> = notes.iter().map(|n| n.map_or(".".to_string(), |m| midi_to_token(m, key.as_ref()))).collect();
    let line = line(&tokens);
    Hum { notes, tokens, line }
}

/// The `hum:` voice line for these tokens (the web's timbre).
pub fn line(tokens: &[String]) -> String {
    format!("hum: {} | tri | lp 2500 | reverb 0.35 | gain 0.16", tokens.join(" "))
}

/// The song with the hum line added at the end.
pub fn append(song: &str, hum_line: &str) -> String {
    format!("{}\n{hum_line}", song.trim_end())
}

/// One note (or rest) per step. Each step is analysed on up to three frames
/// spread across it; the step is a note when most frames are, and the note is
/// their median. Steps past the end of the recording are rests.
pub fn step_notes(samples: &[f32], sample_rate: f32, settings: Settings) -> Vec<Option<i32>> {
    let step_len = settings.step_seconds() * sample_rate as f64;
    (0..settings.steps)
        .map(|s| {
            let start = (s as f64 * step_len) as usize;
            let end = (((s + 1) as f64 * step_len) as usize).min(samples.len());
            if end <= start || ((end - start) as f64) < step_len / 2.0 {
                return None; // not recorded, or only a sliver of the step
            }
            let span = end - start;
            let starts: Vec<usize> = if span >= WINDOW {
                let room = span - WINDOW;
                vec![start, start + room / 2, start + room]
            } else {
                // a short step: one frame centred on it, as far as the recording allows
                let centre = start + span / 2;
                vec![centre.saturating_sub(WINDOW / 2).min(samples.len().saturating_sub(WINDOW))]
            };
            let mut found: Vec<i32> = starts
                .iter()
                .filter_map(|&i| detect_pitch(&samples[i..(i + WINDOW).min(samples.len())], sample_rate))
                .filter(|f| *f > MIN_HZ && *f < MAX_HZ)
                .map(freq_to_midi)
                .collect();
            if found.len() * 2 <= starts.len() {
                return None;
            }
            found.sort_unstable();
            Some(found[found.len() / 2])
        })
        .collect()
}

/// The pitch of one frame in Hz, or `None` for silence / no clear pitch.
/// The web's autocorrelation: trim the frame to its first and last quiet
/// samples, correlate it with itself, skip the first dip, take the highest
/// peak and refine it with a parabola. Lags stop just past the lowest
/// accepted pitch (the web went to the end of the frame, which only found
/// pitches it then threw away).
pub fn detect_pitch(frame: &[f32], sample_rate: f32) -> Option<f32> {
    if frame.len() < 4 {
        return None;
    }
    let rms = (frame.iter().map(|x| x * x).sum::<f32>() / frame.len() as f32).sqrt();
    if rms < SILENCE_RMS {
        return None;
    }
    let (half, thr) = (frame.len() / 2, 0.2);
    let r1 = (0..half).find(|&i| frame[i].abs() < thr).unwrap_or(0);
    let r2 = (1..half).find(|&i| frame[frame.len() - i].abs() < thr).map_or(frame.len() - 1, |i| frame.len() - i);
    let b = &frame[r1..r2];
    let n = b.len();
    let max_lag = ((sample_rate / MIN_HZ) as usize + 2).min(n.saturating_sub(1));
    if max_lag < 3 {
        return None;
    }
    let c: Vec<f32> = (0..=max_lag).map(|lag| (0..n - lag).map(|i| b[i] * b[i + lag]).sum()).collect();
    let mut d = 0;
    while d < max_lag && c[d] > c[d + 1] {
        d += 1;
    }
    let (mut best, mut peak) = (0, f32::MIN);
    for (i, &v) in c.iter().enumerate().take(max_lag).skip(d) {
        if v > peak {
            peak = v;
            best = i;
        }
    }
    if best == 0 || c[0] <= 0.0 {
        return None;
    }
    // clarity: the correlation at the period, per overlapping sample, against the energy per sample
    let clarity = (peak / (n - best) as f32) / (c[0] / n as f32);
    if clarity < MIN_CLARITY {
        return None;
    }
    let (x1, x2, x3) = (c[best - 1], c[best], c[best + 1]);
    let a = (x1 + x3 - 2.0 * x2) / 2.0;
    let slope = (x3 - x1) / 2.0;
    let period = if a != 0.0 { best as f32 - slope / (2.0 * a) } else { best as f32 };
    Some(sample_rate / period)
}

/// Nearest MIDI note (A4 = 440 Hz = 69).
pub fn freq_to_midi(freq: f32) -> i32 {
    (69.0 + 12.0 * (freq / 440.0).log2()).round() as i32
}

/// `c4`, `f#3` — a MIDI note as a LOOM note name (sharps, like the web).
pub fn midi_to_name(midi: i32) -> String {
    const NAMES: [&str; 12] = ["c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b"];
    format!("{}{}", NAMES[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1)
}

/// A MIDI note as a LOOM token: the nearest scale degree of `key` with octave
/// marks (`5`, `3'`, `1,,`), or a note name without a key. Ties go to the lower
/// note, as in the web. Only degrees 1-7 are written (LOOM has no degree 8),
/// so the extra notes of 8- to 12-note scales are not used.
pub fn midi_to_token(midi: i32, key: Option<&Key>) -> String {
    let Some(key) = key.filter(|k| !k.scale.is_empty()) else { return midi_to_name(midi) };
    let (mut best, mut best_dist, mut best_oct) = (1, f64::MAX, 0);
    for oct in -2..=3 {
        for (d, step) in key.scale.iter().enumerate().take(7) {
            let m = (12 * (key.octave + oct + 1) + key.tonic) as f64 + step;
            let dist = (m - midi as f64).abs();
            if dist < best_dist {
                (best, best_dist, best_oct) = (d + 1, dist, oct);
            }
        }
    }
    let mark = if best_oct > 0 { "'" } else { "," };
    format!("{best}{}", mark.repeat(best_oct.unsigned_abs() as usize))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 44_100.0;

    fn midi_freq(m: i32) -> f32 {
        440.0 * 2f32.powf((m - 69) as f32 / 12.0)
    }

    /// A voice-like tone: fundamental plus two softer harmonics, phase kept continuous.
    fn tone(out: &mut Vec<f32>, sr: f32, freq: f32, seconds: f64, amp: f32, phase: &mut f32) {
        for _ in 0..(seconds * sr as f64) as usize {
            *phase = (*phase + TAU * freq / sr) % TAU;
            out.push(amp * (phase.sin() + 0.4 * (2.0 * *phase).sin() + 0.2 * (3.0 * *phase).sin()) / 1.6);
        }
    }

    /// Deterministic white noise in -amp..amp.
    fn noise(len: usize, amp: f32, seed: &mut u32) -> Vec<f32> {
        (0..len)
            .map(|_| {
                *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                amp * ((*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0)
            })
            .collect()
    }

    #[test]
    fn detects_pitches_across_the_range() {
        for sr in [16_000.0, 44_100.0, 48_000.0] {
            for m in [45, 52, 57, 64, 69, 76, 81, 88] {
                let f = midi_freq(m);
                let frame: Vec<f32> = (0..WINDOW).map(|i| 0.5 * (TAU * f * i as f32 / sr).sin()).collect();
                let got = detect_pitch(&frame, sr).unwrap_or_else(|| panic!("no pitch for {f} Hz at {sr}"));
                assert!((12.0 * (got / f).log2()).abs() < 0.3, "{f} Hz at {sr} Hz → {got}");
            }
        }
    }

    #[test]
    fn silence_and_noise_are_rests() {
        assert_eq!(detect_pitch(&[0.0; WINDOW], SR), None);
        assert_eq!(detect_pitch(&[0.005; WINDOW], SR), None, "under the gate");
        let mut seed = 7;
        for amp in [0.05, 0.3, 0.9] {
            let n = noise(WINDOW, amp, &mut seed);
            assert_eq!(detect_pitch(&n, SR).filter(|f| *f > MIN_HZ && *f < MAX_HZ), None, "noise at {amp}");
        }
        assert_eq!(detect_pitch(&[], SR), None);
    }

    #[test]
    fn a_sung_scale_becomes_degrees_in_key() {
        // c minor: c3 d3 eb3 f3 g3 ab3 bb3 c4, one per eighth note at 120 bpm, then silence
        let song = "bpm: 120\nkey: c minor\nkick: x . x .\n";
        let settings = Settings::for_song(song);
        assert_eq!((settings.step_seconds(), settings.seconds()), (0.25, 4.0));
        let (mut samples, mut phase) = (Vec::new(), 0.0);
        for m in [48, 50, 51, 53, 55, 56, 58, 60] {
            tone(&mut samples, SR, midi_freq(m), 0.25, 0.4, &mut phase);
        }
        samples.extend(std::iter::repeat_n(0.0, (4.0 * SR) as usize - samples.len()));
        let hum = melody(&samples, SR, song);
        assert_eq!(hum.tokens.join(" "), "1 2 3 4 5 6 7 1' . . . . . . . .");
        assert_eq!(hum.line, "hum: 1 2 3 4 5 6 7 1' . . . . . . . . | tri | lp 2500 | reverb 0.35 | gain 0.16");
        assert!(hum.message().starts_with("[ hum ] mélodie ajoutée : 1 2 3"));
        // the result is a real voice that plays the sung pitches
        let out = append(song, &hum.line);
        assert!(out.starts_with(song.trim_end()) && out.ends_with("gain 0.16"));
        crate::generate::assert_fully_recognised(&out);
        let events = loom_core::Program::parse(&out).schedule_bar(0);
        let hummed: Vec<i32> = events.iter().filter(|e| e.voice == "hum").map(|e| freq_to_midi(e.freq.unwrap())).collect();
        assert_eq!(hummed, [48, 50, 51, 53, 55, 56, 58, 60]);
    }

    #[test]
    fn out_of_key_notes_snap_and_no_key_gives_names() {
        let key = loom_core::parse_key("key: c major").unwrap();
        assert_eq!(midi_to_token(61, Some(&key)), "1'", "c#4 → c4, a tie goes low");
        assert_eq!(midi_to_token(42, Some(&key)), "4,", "f#2 → f2, an octave down");
        assert_eq!(midi_to_token(79, Some(&key)), "5''");
        assert_eq!(midi_to_token(61, None), "c#4");
        assert_eq!(midi_to_name(-1), "b-2");
        let penta = loom_core::parse_key("key: a minpent").unwrap();
        assert_eq!(midi_to_token(58, Some(&penta)), "1", "a#3 isn't in a minor pentatonic");
        assert_eq!(midi_to_token(59, Some(&penta)), "2", "b3 → c4");
    }

    #[test]
    fn a_noisy_sweep_rises_and_breath_is_ignored() {
        // 16 steps at 100 bpm: a glide from a3 to a4 with background noise,
        // but steps 5-6 are only breath (noise) and step 11 is silence
        let settings = Settings { bpm: 100.0, steps: 16 };
        let step = (settings.step_seconds() * SR as f64) as usize;
        let mut seed = 3;
        let mut samples = Vec::new();
        let mut phase: f32 = 0.0;
        for s in 0..16 {
            if s == 5 || s == 6 {
                samples.extend(noise(step, 0.2, &mut seed));
                continue;
            }
            if s == 11 {
                samples.extend(std::iter::repeat_n(0.0, step));
                continue;
            }
            let bg = noise(step, 0.03, &mut seed);
            for (i, n) in bg.iter().enumerate() {
                let f = 220.0 * 2f32.powf((s as f32 + i as f32 / step as f32) / 16.0);
                phase = (phase + TAU * f / SR) % TAU;
                samples.push(0.4 * phase.sin() + n);
            }
        }
        let notes = step_notes(&samples, SR, settings);
        assert_eq!(notes.len(), 16);
        assert_eq!((notes[5], notes[6], notes[11]), (None, None, None), "{notes:?}");
        let sung: Vec<i32> = notes.iter().flatten().copied().collect();
        assert_eq!(sung.len(), 13, "{notes:?}");
        assert!(sung.windows(2).all(|w| w[1] >= w[0]), "rising: {sung:?}");
        assert!((57..=58).contains(&sung[0]) && (68..=69).contains(sung.last().unwrap()), "{sung:?}");
    }

    #[test]
    fn short_or_empty_recordings() {
        let settings = Settings { bpm: 120.0, steps: 16 };
        assert_eq!(step_notes(&[], SR, settings), vec![None; 16]);
        let hum = melody(&[], SR, "lead: 1 2 3\n");
        assert!(hum.is_silent() && hum.message().contains("rien entendu"));
        // one second of a4 at 120 bpm: 4 steps sung, the rest are rests
        let mut samples = Vec::new();
        tone(&mut samples, SR, 440.0, 1.0, 0.5, &mut 0.0);
        let notes = step_notes(&samples, SR, settings);
        assert_eq!(&notes[..4], &[Some(69); 4]);
        assert!(notes[4..].iter().all(Option::is_none));
        // a fast tempo at a low sample rate: steps shorter than a frame still work
        let fast = Settings { bpm: 300.0, steps: 8 };
        let mut samples = Vec::new();
        tone(&mut samples, 8_000.0, 330.0, fast.seconds(), 0.5, &mut 0.0);
        assert!(step_notes(&samples, 8_000.0, fast).iter().all(|n| *n == Some(64)), "{:?}", step_notes(&samples, 8_000.0, fast));
    }
}
