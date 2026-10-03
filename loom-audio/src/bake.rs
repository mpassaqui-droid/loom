//! « Cuire » un programme LOOM en notes MIDI pour un DAW (Ableton Live d'abord).
//!
//! On ne réécrit pas la logique musicale : chaque voix passe par le VRAI
//! séquenceur, hors ligne, forcée en `| midi`, mesure par mesure sans calcul audio. Accords, arpèges, voicings,
//! flam, humanize, sections et tempo sortent donc exactement comme à l'écoute.
//! Les temps sont convertis en temps musicaux (noires) mesure par mesure, pour
//! que les clips restent justes même quand le tempo change d'une section à l'autre.

use std::sync::Arc;

use loom_core::{Program, Tuning};

use crate::sample::SampleBank;
use crate::seq::{MidiEvent, Sequencer};

const SR: f32 = 48_000.0;

/// Une note cuite, en temps musicaux (noires) depuis le début.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedNote { pub pitch: u8, pub beat: f64, pub dur: f64, pub vel: u8 }

/// Une voix LOOM devenue une piste MIDI.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedTrack { pub name: String, pub drum: bool, pub notes: Vec<BakedNote> }

/// Le morceau cuit : de quoi construire un set Live.
#[derive(Debug, Clone, PartialEq)]
pub struct Baked {
    pub bars: u32,
    /// Noires par mesure (`meter:`).
    pub meter_beats: f64,
    /// Tempo de chaque mesure.
    pub bar_bpm: Vec<f64>,
    /// `(section, première mesure, nombre de mesures)` dans l'ordre du `song:`.
    pub sections: Vec<(String, u32, u32)>,
    pub tracks: Vec<BakedTrack>,
    /// Fichier d'accordage Live (.ascl) quand `tune:` n'est pas tempéré égal.
    pub ascl: Option<String>,
}

/// Mesures à cuire par défaut : tout le `song:`, sinon 8.
pub fn default_bars(program: &Program) -> u32 {
    match program.len_bars() { 0 => 8, n => n }
}

pub fn bake(src: &str, bars: u32) -> Baked {
    let bars = bars.max(1);
    let program = Program::parse(src);
    let len = program.len_bars() as i64;
    let song_bar = |b: i64| if len > 0 { b.rem_euclid(len) } else { b };

    // début de chaque mesure en échantillons, exactement comme le séquenceur les compte
    let mut starts = vec![0u64];
    let mut bar_bpm = Vec::new();
    for b in 0..bars as i64 {
        let sec = program.bar_seconds(song_bar(b)) as f32;
        starts.push(starts[b as usize] + ((sec * SR) as u64).max(1));
        bar_bpm.push(program.bar_bpm(song_bar(b)));
    }
    let total = *starts.last().unwrap_or(&0) as usize;
    let beat_of = |sample: f64| -> f64 {
        let k = starts.partition_point(|s| (*s as f64) <= sample).saturating_sub(1).min(bars as usize - 1);
        let span = (starts[k + 1] - starts[k]) as f64;
        (k as f64 + (sample - starts[k] as f64) / span) * program.meter_beats
    };

    let mut names: Vec<String> = Vec::new();
    for v in &program.voices {
        if !names.contains(&v.name) { names.push(v.name.clone()); }
    }
    let tracks = names.iter().filter_map(|name| {
        let mut p = Program::parse(src);
        let drum = p.voices.iter().any(|v| v.name == *name && v.is_drum);
        for v in p.voices.iter_mut().filter(|v| v.name == *name) {
            let s = Arc::make_mut(&mut v.synth);
            if s.midi.is_none() { s.midi = Some(if v.is_drum { 10 } else { 1 }); }
        }
        let mut seq = Sequencer::new(SR, Arc::new(SampleBank::empty()));
        seq.set_stem(Some(name));
        seq.set_program(p);
        seq.play();
        let mut notes = Vec::new();
        for bar in 0..bars {
            if bar > 0 { seq.skip_to_next_bar(); }
            for ev in seq.midi_out.drain(..) {
                let MidiEvent::Note(m) = ev else { continue };
                let s = m.at * SR as f64;
                if s >= total as f64 { continue; }
                let beat = beat_of(s);
                let bpm = bar_bpm[(beat / program.meter_beats) as usize];
                notes.push(BakedNote { pitch: m.note, beat, dur: (m.duration as f64 * bpm / 60.0).max(1.0 / 64.0), vel: m.velocity });
            }
        }
        (!notes.is_empty()).then(|| BakedTrack { name: name.clone(), drum, notes })
    }).collect();

    let mut sections = Vec::new();
    let mut at = 0u32;
    for (name, n) in &program.song {
        if at >= bars { break; }
        sections.push((name.clone(), at, (*n).min(bars - at)));
        at += n;
    }
    let tonic = program.key.as_ref().map_or(0, |k| k.tonic);
    Baked { bars, meter_beats: program.meter_beats, bar_bpm, sections, tracks, ascl: ascl(&program.mix.tune, tonic) }
}

/// Accordage Live (.ascl, format Scala) d'un `tune:` à 12 notes, relatif à la tonique.
/// `None` pour le tempérament égal, l'adaptatif (il dépend de l'accord joué) et les N-EDO
/// (un clavier MIDI à 12 touches par octave ne peut pas les porter tels quels).
pub fn ascl(tune: &str, tonic: i32) -> Option<String> {
    let tuning = match tune {
        "just" => Tuning::Just, "pyth" => Tuning::Pyth, "werck" => Tuning::Werck,
        "kirn" => Tuning::Kirn, "meantone" => Tuning::Meantone,
        _ => return None,
    };
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let t = tonic.rem_euclid(12) as usize;
    let mut out = format!("! LOOM {tune} sur {}.ascl\n!\nLOOM : accordage {tune}, tonique {}\n 12\n!\n", NAMES[t], NAMES[t]);
    for iv in 1..12 {
        out += &format!(" {:.3}\n", iv as f32 * 100.0 + loom_core::tune_cents(iv, tuning));
    }
    out += " 2/1\n!\n! @ABL NOTE_NAMES";
    for k in 0..12 { out += &format!(" \"{}\"", NAMES[(t + k) % 12]); }
    // la tonique garde sa hauteur tempérée (C4 = 261.63 Hz, A4 = 440 Hz)
    out += &format!("\n! @ABL REFERENCE_PITCH 4 0 {:.4}\n", 440.0 * 2f64.powf((t as f64 - 9.0) / 12.0));
    Some(out)
}

impl Baked {
    /// JSON lisible par `tools/bake_live.py` (aucune dépendance côté Rust).
    pub fn to_json(&self) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let mut o = format!("{{\"bars\":{},\"meter_beats\":{},\"bar_bpm\":[{}],\"sections\":[",
            self.bars, self.meter_beats,
            self.bar_bpm.iter().map(|b| format!("{b}")).collect::<Vec<_>>().join(","));
        o += &self.sections.iter().map(|(n, a, l)| format!("[\"{}\",{a},{l}]", esc(n))).collect::<Vec<_>>().join(",");
        o += "],\"tracks\":[";
        o += &self.tracks.iter().map(|t| {
            let notes = t.notes.iter().map(|n| format!("[{},{:.5},{:.5},{}]", n.pitch, n.beat, n.dur, n.vel)).collect::<Vec<_>>().join(",");
            format!("{{\"name\":\"{}\",\"drum\":{},\"notes\":[{notes}]}}", esc(&t.name), t.drum)
        }).collect::<Vec<_>>().join(",");
        o += "],\"ascl\":";
        o += &match &self.ascl { Some(a) => format!("\"{}\"", esc(a).replace('\n', "\\n")), None => "null".into() };
        o + "}"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bakes_every_voice_on_the_beat_grid() {
        let b = bake("bpm: 120\nkick: x . x .\nlead: c4 e4 g4 c5 | saw", 2);
        assert_eq!(b.tracks.len(), 2);
        let kick = b.tracks.iter().find(|t| t.name == "kick").unwrap();
        assert!(kick.drum);
        assert_eq!(kick.notes.iter().map(|n| (n.pitch, (n.beat * 1000.0).round() / 1000.0)).collect::<Vec<_>>(),
                   vec![(36, 0.0), (36, 2.0), (36, 4.0), (36, 6.0)]);
        let lead = b.tracks.iter().find(|t| t.name == "lead").unwrap();
        assert_eq!(lead.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(), vec![60, 64, 67, 72, 60, 64, 67, 72]);
        assert!((lead.notes[1].beat - 1.0).abs() < 1e-3 && (lead.notes[1].dur - 1.0).abs() < 1e-2);
    }

    #[test]
    fn tempo_changes_keep_beats_exact() {
        // two sections at different tempi: beat positions stay on the grid
        let src = "song: a*1 b*1\ntempo: a 120 b 60\nhat: x x x x";
        let b = bake(src, 2);
        assert_eq!(b.bar_bpm, vec![120.0, 60.0]);
        assert_eq!(b.sections, vec![("a".into(), 0, 1), ("b".into(), 1, 1)]);
        let beats: Vec<f64> = b.tracks[0].notes.iter().map(|n| (n.beat * 1000.0).round() / 1000.0).collect();
        assert_eq!(beats, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    }

    #[test]
    fn chords_bake_as_stacked_notes() {
        let b = bake("bpm: 120\npad: Cm7 . . .", 1);
        let mut p: Vec<u8> = b.tracks[0].notes.iter().map(|n| n.pitch).collect();
        p.sort();
        assert_eq!(p.len(), 4);
        assert!(b.tracks[0].notes.iter().all(|n| n.beat.abs() < 1e-6));
    }

    #[test]
    fn tuning_file_for_twelve_note_tunings_only() {
        let a = ascl("just", 9).unwrap();
        assert!(a.contains(" 12\n") && a.contains(" 386.310\n") && a.contains("\"A\" \"A#\""));
        assert!(a.contains("REFERENCE_PITCH 4 0 440.0000"));
        assert!(ascl("equal", 0).is_none() && ascl("19 edo", 0).is_none() && ascl("adaptive", 0).is_none());
        let j = bake("tune: pyth\nkey: c major\nlead: 1 3 5 | saw", 1).to_json();
        assert!(j.contains("\"ascl\":\"! LOOM pyth sur C.ascl"));
    }
}
