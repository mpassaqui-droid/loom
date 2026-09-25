//! ROLL: the melodies of the current bar as blocks — time left to right,
//! pitch bottom to top, one colour per voice. Ported from `drawRoll`.

use loom_audio::clock::name_salt;
use loom_core::Program;

/// One note block.
#[derive(Clone, Debug, PartialEq)]
pub struct RollNote {
    pub voice: String,
    /// Colour, 0..360 (`| hue`, else from the voice name like the grid and the stage).
    pub hue: f32,
    /// Start and length inside the bar, 0..1.
    pub start: f32,
    pub len: f32,
    /// MIDI note number (60 = middle C).
    pub midi: i32,
}

/// What the piano roll shows for one bar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roll {
    pub notes: Vec<RollNote>,
    /// Lowest and highest MIDI note drawn (at least 7 semitones apart).
    pub low: i32,
    pub high: i32,
}

impl Roll {
    /// Height of the pitch range in semitones (rows).
    pub fn rows(&self) -> i32 {
        self.high - self.low + 1
    }

    /// Which note is under a point (x 0..1 across, y 0..1 from the top), if any.
    pub fn note_at(&self, x: f32, y: f32) -> Option<&RollNote> {
        let pitch = self.high - (y.clamp(0.0, 0.9999) * self.rows() as f32) as i32;
        self.notes.iter().rev().find(|n| n.midi == pitch && x >= n.start && x < n.start + n.len.max(0.02))
    }
}

/// The MIDI note nearest to a frequency.
pub fn freq_to_midi(freq: f32) -> i32 {
    (69.0 + 12.0 * (freq / 440.0).log2()).round() as i32
}

/// The melodic notes (not drums, not `| mute` voices) of `bar` of the song.
pub fn roll(program: &Program, bar: i64) -> Roll {
    let hue_of = |name: &str| {
        let voice = program.voices.iter().find(|v| v.name == name && v.section.is_none());
        voice.and_then(|v| v.synth.hue).unwrap_or((name_salt(name) % 360) as f32)
    };
    let muted = |name: &str| program.voices.iter().any(|v| v.name == name && v.synth.mute);
    let mut notes = Vec::new();
    for ev in program.schedule_bar(bar) {
        let Some(freq) = ev.freq.filter(|f| *f > 0.0 && !ev.is_drum) else { continue };
        if muted(&ev.voice) {
            continue;
        }
        notes.push(RollNote {
            hue: hue_of(&ev.voice),
            start: ev.phase.clamp(0.0, 1.0),
            len: ev.dur.clamp(0.0, 1.0),
            midi: freq_to_midi(freq),
            voice: ev.voice,
        });
    }
    let (mut low, mut high) = notes.iter().fold((i32::MAX, i32::MIN), |(lo, hi), n| (lo.min(n.midi), hi.max(n.midi)));
    if notes.is_empty() {
        (low, high) = (48, 72);
    }
    if high - low + 1 < 7 {
        high = low + 6;
    }
    Roll { notes, low, high }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melodies_become_blocks() {
        let p = Program::parse("kick: x . x .\nlead: c4 . e4 g4");
        let r = roll(&p, 0);
        let pitches: Vec<i32> = r.notes.iter().map(|n| n.midi).collect();
        assert_eq!(pitches, [60, 64, 67], "drums are left out");
        assert_eq!((r.notes[1].start, r.notes[1].len), (0.5, 0.25));
        assert_eq!((r.low, r.high), (60, 67));
        // 8 rows from 67 (top) down to 60: e4 (64) is the 4th row, y 3/8..4/8
        assert_eq!(r.note_at(0.55, 0.4).map(|n| n.midi), Some(64));
        assert_eq!(r.note_at(0.30, 0.4), None);
    }

    #[test]
    fn small_ranges_are_widened() {
        let r = roll(&Program::parse("bass: c2 c2"), 0);
        assert_eq!((r.low, r.rows()), (36, 7));
        assert_eq!(freq_to_midi(440.0), 69);
        assert!(roll(&Program::parse("kick: x"), 0).notes.is_empty());
    }
}
