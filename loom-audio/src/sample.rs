//! Samples — WAV loading and playback, the port of the browser's `playSample`,
//! `grainSample`, `playSampleWarp` (warp / freeze), `| chop`, `| begin/end`,
//! `| reverse`.
//!
//! Every way of playing a sample is described the same way: a list of
//! [`Grain`]s, each one "read the buffer from here, at this speed, for this
//! long, with this envelope and pan". A plain hit is one grain, `| grain` is a
//! cloud of up to 40, `| warp` / `| freeze` up to 400 overlapping ones. The
//! list is built once in [`SampleVoice::new`]; `render` only reads it, so the
//! audio thread never allocates.

mod codec;
mod wav;

pub use codec::{decode_any, EXTENSIONS as SAMPLE_EXTENSIONS};

pub use wav::decode as decode_wav;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_samples.rs"));
}

use crate::clock::Clock;
use loom_core::zones::{parse_zone, pick_zone, Zone};
use crate::dsp::Noise;
use crate::note::{NoteSpec, Stereo};
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Decoded audio, kept at its own sample rate.
pub struct SampleData {
    pub sample_rate: f32,
    /// One or two channels of float samples.
    pub channels: Vec<Vec<f32>>,
}

impl SampleData {
    /// Read and decode an audio file (WAV, AIFF, FLAC, MP3).
    pub fn from_file(path: &Path) -> Result<SampleData, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        codec::decode_any(&bytes, &ext)
    }

    /// Number of sample frames.
    pub fn frames(&self) -> usize { self.channels.first().map_or(0, |c| c.len()) }

    /// Length in seconds (the JS `buffer.duration`).
    pub fn duration(&self) -> f32 { self.frames() as f32 / self.sample_rate }
}

/// All samples available to a song, by name (`bd`, `amen`…). Loaded once, off
/// the audio thread, then shared read-only.
#[derive(Default)]
pub struct SampleBank {
    samples: HashMap<String, Arc<SampleData>>,
    /// Les instruments multi-échantillons (`piano_c3_v2`…), par instrument.
    zones: HashMap<String, Vec<Zone>>,
}

impl SampleBank {
    pub fn empty() -> SampleBank { SampleBank::default() }

    /// Folders searched for `NAME.wav`, in priority order:
    /// `$LOOM_SAMPLES` (a `:`-separated list), the install location
    /// (`$XDG_DATA_HOME` or `~/.local/share`, then `/loom/samples`), the
    /// repo's `web/samples` found by walking up from the running program, and
    /// `./web/samples`.
    pub fn default_dirs() -> Vec<PathBuf> {
        use std::env;
        let mut dirs = Vec::new();
        if let Some(list) = env::var_os("LOOM_SAMPLES") {
            dirs.extend(env::split_paths(&list).filter(|p| !p.as_os_str().is_empty()));
        }
        if let Some(d) = crate::paths::data_dir() {
            dirs.push(d.join("samples"));
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(found) = exe.ancestors().skip(1).map(|a| a.join("web/samples")).find(|p| p.is_dir()) {
                dirs.push(found);
            }
        }
        dirs.push(PathBuf::from("web/samples"));
        dirs
    }

    /// Load every sample found in the default folders, then the built-in sounds
    /// the program carries (a file on disk with the same name wins).
    pub fn load_default() -> SampleBank {
        let mut bank = SampleBank::load_dirs(&SampleBank::default_dirs());
        bank.add_embedded();
        bank
    }

    /// The built-in sounds not already in the bank.
    pub fn add_embedded(&mut self) {
        for (name, bytes) in embedded::EMBEDDED {
            if self.samples.contains_key(*name) { continue; }
            if let Ok(data) = codec::decode_any(bytes, "wav") { self.insert(name, data); }
        }
    }

    /// Load every `.wav` file directly inside these folders, named by file
    /// stem (`bd.wav` → `bd`). When two folders have the same name, the
    /// earlier folder wins. Missing folders and unreadable files are skipped.
    pub fn load_dirs(dirs: &[PathBuf]) -> SampleBank {
        let mut bank = SampleBank::empty();
        let mut seen = Vec::new();
        for dir in dirs {
            // the same folder can appear twice (e.g. the repo and `./web/samples`)
            let Ok(canon) = dir.canonicalize() else { continue };
            if seen.contains(&canon) { continue; }
            seen.push(canon);
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str())
                    .is_some_and(|e| codec::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())))
                .collect();
            files.sort();
            for path in files {
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
                if bank.samples.contains_key(name) { continue; }
                if let Ok(data) = SampleData::from_file(&path) {
                    bank.insert(name, data);
                }
            }
        }
        bank
    }

    /// Add (or replace) one sample.
    pub fn insert(&mut self, name: &str, data: SampleData) { self.insert_arc(name, Arc::new(data)); }

    /// Ajoute un fichier et tient l'index des zones à jour sans relire toute la
    /// banque (des milliers de fichiers restent linéaires). Même résultat que
    /// `zones_of` sur toutes les clés : les tests le vérifient.
    fn insert_arc(&mut self, name: &str, data: Arc<SampleData>) {
        self.samples.insert(name.to_string(), data);
        let plain = |inst: &str| Zone { name: inst.to_string(), inst: inst.to_string(), root: None, vel: 0, rr: 0 };
        if let Some(z) = parse_zone(name) {
            let has_plain = self.samples.contains_key(&z.inst);
            let inst = z.inst.clone();
            let zs = self.zones.entry(inst.clone()).or_default();
            if zs.is_empty() && has_plain { zs.push(plain(&inst)); }
            if !zs.iter().any(|x| x.name == name) { zs.push(z); }
            zs.sort_by(|a, b| a.name.cmp(&b.name));
        }
        // ce fichier est peut-être le fichier nu d'un instrument déjà à zones
        if let Some(zs) = self.zones.get_mut(name) {
            if !zs.iter().any(|x| x.name == name) { zs.push(plain(name)); zs.sort_by(|a, b| a.name.cmp(&b.name)); }
        }
    }

    /// Le fichier à jouer pour `| sample INST` : une zone de l'instrument (note la
    /// plus proche de `midi`, couche de `vel`, prise `counter`) avec sa racine MIDI,
    /// sinon l'échantillon `INST` tel quel.
    pub fn resolve(&self, inst: &str, midi: Option<f32>, vel: f32, counter: u32) -> Option<(&str, Option<i32>)> {
        if let Some(z) = self.zones.get(inst).and_then(|zs| pick_zone(zs, midi, vel, counter)) {
            return Some((z.name.as_str(), z.root));
        }
        self.samples.get_key_value(inst).map(|(k, _)| (k.as_str(), None))
    }

    pub fn get(&self, name: &str) -> Option<&Arc<SampleData>> { self.samples.get(name) }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.samples.keys().cloned().collect();
        v.sort();
        v
    }
}

// --- the session bank: the disk samples + the WAVs the user loaded at runtime ---
//
// `default_bank()` used to be a fixed snapshot; now a user can open their own
// files in the SAMPLER panel and they must be heard by the very next render.
// We keep a small store: the last disk load (re-scannable) plus the session's
// user samples (always on top, surviving a disk reload).

use std::sync::{Mutex, OnceLock};

// --- `| stretch` : les sons étirés par vocodeur de phase, gardés en cache ---
//
// Étirer deux mesures coûte des dizaines de millisecondes : trop pour le fil audio.
// En direct, le premier coup part en `| warp` granulaire pendant qu'un fil à part
// calcule ; les coups suivants jouent la version étirée. Hors direct (rendu WAV,
// tests), on calcule tout de suite : le résultat ne dépend pas de la vitesse.

thread_local! {
    static LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Le fil audio en direct s'annonce ici (à chaque bloc, ça ne coûte rien) : sur ce
/// fil, `| stretch` ne calcule jamais lui-même. Les rendus WAV tournent ailleurs.
pub fn mark_live_thread() { LIVE.with(|l| l.set(true)); }

/// (adresse du son d'origine, trames de sortie) → son étiré. La valeur garde aussi
/// l'original vivant, pour que son adresse ne serve pas à un autre son.
type StretchMap = HashMap<(usize, usize), Option<(Arc<SampleData>, Arc<SampleData>)>>;

fn stretch_cache() -> &'static Mutex<StretchMap> {
    static CACHE: OnceLock<Mutex<StretchMap>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn compute_stretch(src: &SampleData, frames: usize) -> SampleData {
    SampleData { sample_rate: src.sample_rate, channels: loom_core::stretch::stretch(&src.channels, frames) }
}

/// `src` étiré à `frames` trames, hauteur gardée. `None` en direct tant que le
/// calcul n'est pas fini (ou si le cache est occupé : le fil audio n'attend jamais).
pub fn stretched(src: &Arc<SampleData>, frames: usize) -> Option<Arc<SampleData>> {
    let key = (Arc::as_ptr(src) as usize, frames);
    let live = LIVE.with(|l| l.get());
    {
        let mut map = if live { stretch_cache().try_lock().ok()? } else { stretch_cache().lock().ok()? };
        match map.get(&key) {
            Some(Some((_, done))) => return Some(done.clone()),
            // en cours : le direct attend, un rendu calcule lui-même
            Some(None) if live => return None,
            _ => {}
        }
        if live {
            // au plus 4 calculs à la fois (un tempo qui glisse change la clé à chaque mesure)
            if map.values().filter(|v| v.is_none()).count() >= 4 { return None; }
            // un cache borné : on oublie les sons finis, jamais un calcul en cours
            if map.len() >= 64 { map.retain(|_, v| v.is_none()); }
            map.insert(key, None);
        } else if map.len() >= 64 {
            map.retain(|_, v| v.is_none());
        }
    }
    if live {
        let src = src.clone();
        std::thread::spawn(move || {
            let done = Arc::new(compute_stretch(&src, frames));
            if let Ok(mut map) = stretch_cache().lock() { map.insert(key, Some((src, done))); }
        });
        return None;
    }
    let done = Arc::new(compute_stretch(src, frames));
    if let Ok(mut map) = stretch_cache().lock() { map.insert(key, Some((src.clone(), done.clone()))); }
    Some(done)
}

struct SessionBank {
    /// The last scan of the default sample folders.
    fresh: SampleBank,
    /// WAVs loaded this session, by name (priority over `fresh`).
    session: HashMap<String, Arc<SampleData>>,
}

fn session() -> &'static Mutex<SessionBank> {
    static SESSION: OnceLock<Mutex<SessionBank>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(SessionBank { fresh: SampleBank::load_default(), session: HashMap::new() }))
}

/// The bank every native path hears through: the disk samples plus the WAVs
/// loaded this session.
pub fn merged_bank() -> SampleBank {
    let s = session().lock().unwrap();
    let mut bank = SampleBank::empty();
    for (k, v) in s.fresh.samples.iter().chain(s.session.iter()) { bank.insert_arc(k, v.clone()); }
    bank
}

/// Load a user WAV into the session bank. Returns the bank name to write as
/// `| sample NAME` (the file stem — `chant.wav` → `chant`). If a disk sample
/// already owns the name, that name is returned as-is and nothing is loaded —
/// the bank never hides a real sample behind a session one.
pub fn load_user_sample(path: &Path) -> Result<String, String> {
    let data = SampleData::from_file(path)?;
    if data.frames() == 0 {
        return Err(format!("\u{201c}{}\u{201d} : fichier WAV vide.", path.display()));
    }
    let name = path.file_stem().and_then(|s| s.to_str()).map(str::to_string).unwrap_or_default();
    if name.is_empty() {
        return Err(format!("\u{201c}{}\u{201d} : nom de fichier invalide.", path.display()));
    }
    let mut s = session().lock().unwrap();
    if s.fresh.samples.contains_key(&name) {
        return Ok(name); // already available on disk — use it as-is
    }
    s.session.insert(name.clone(), Arc::new(data));
    Ok(name)
}

/// Re-scan the default sample folders (WAVs dropped on disk since launch are
/// picked up), keeping the session's user samples.
pub fn reload_bank() {
    let mut s = session().lock().unwrap();
    s.fresh = SampleBank::load_default();
}

/// How many WAVs the user loaded this session (shown as a hint in the panel).
pub fn session_sample_count() -> usize {
    session().lock().unwrap().session.len()
}

/// A short PCM16 mono WAV (a few frames of a sine), for tests that need a file.
#[cfg(test)]
fn pcm16_wav_bytes(frames: usize) -> Vec<u8> {
    let sr = 8000u32;
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + frames as u32 * 2).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());  // PCM
    v.extend_from_slice(&1u16.to_le_bytes());  // mono
    v.extend_from_slice(&sr.to_le_bytes());
    v.extend_from_slice(&(sr * 2).to_le_bytes()); // byte rate
    v.extend_from_slice(&2u16.to_le_bytes());  // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(frames as u32 * 2).to_le_bytes());
    for i in 0..frames {
        let s = ((i as f32 * 2.0 * std::f32::consts::PI * 220.0 / sr as f32).sin() * 8000.0) as i16;
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// The gain shape of one grain over its life.
#[derive(Clone, Copy)]
enum Shape {
    /// Plain playback: full gain, with tiny fades (in output frames) where a
    /// cut in the middle of the buffer would click.
    Flat { fade_in: u32, fade_out: u32 },
    /// `grainSample`: exponential rise from 0.0001 over the first 40 %, then
    /// exponential fall back to 0.0001. `ln_floor` = ln(0.0001 / peak).
    Exp { ln_floor: f32 },
    /// `playSampleWarp`: linear up to the middle, linear down.
    Tri,
}

/// One read of the buffer, placed in time.
#[derive(Clone, Copy)]
struct Grain {
    /// Output frame (since the note started) where it begins.
    start: u64,
    /// Output frames it lasts.
    len: u64,
    /// Read position in buffer frames, moved by `step` every output frame.
    pos: f64,
    step: f64,
    gain: f32,
    shape: Shape,
    /// Stereo matrix `[l←l, l←r, r←l, r←r]` (identity, or a WebAudio `StereoPannerNode`).
    mix: [f32; 4],
}

impl Grain {
    /// Envelope value `i` frames into the grain.
    fn env(&self, i: u64) -> f32 {
        let u = i as f32 / self.len.max(1) as f32;
        match self.shape {
            Shape::Flat { fade_in, fade_out } => {
                let mut g = 1.0f32;
                if i < fade_in as u64 { g = g.min(i as f32 / fade_in as f32); }
                let left = self.len - i;
                if left < fade_out as u64 { g = g.min(left as f32 / fade_out as f32); }
                g
            }
            Shape::Exp { ln_floor } => {
                let rest = if u < 0.4 { 1.0 - u / 0.4 } else { (u - 0.4) / 0.6 };
                (ln_floor * rest).exp()
            }
            Shape::Tri => 1.0 - (2.0 * u - 1.0).abs(),
        }
    }
}

const IDENTITY: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

/// WebAudio `StereoPannerNode` (equal power) as a matrix. A mono source is
/// placed with cos/sin gains; a stereo source keeps its image and folds the
/// far side in.
fn panner(pan: f32, stereo: bool) -> [f32; 4] {
    let pan = pan.clamp(-1.0, 1.0);
    if !stereo {
        let x = (pan + 1.0) / 2.0;
        return [(x * FRAC_PI_2).cos(), 0.0, 0.0, (x * FRAC_PI_2).sin()];
    }
    if pan <= 0.0 {
        let x = pan + 1.0;
        [1.0, (x * FRAC_PI_2).cos(), 0.0, (x * FRAC_PI_2).sin()]
    } else {
        [(pan * FRAC_PI_2).cos(), 0.0, (pan * FRAC_PI_2).sin(), 1.0]
    }
}

/// Cubic Hermite (Catmull-Rom) interpolation between `y1` and `y2`.
fn hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + y1
}

/// One sample being played.
pub struct SampleVoice {
    data: Arc<SampleData>,
    /// Read the buffer backwards (the JS `revBuffer`).
    reverse: bool,
    /// Sorted by `start`.
    grains: Vec<Grain>,
    /// Grains before this index have all finished.
    first: usize,
    /// Output frames rendered so far.
    t: u64,
    /// Output frame where the last grain ends.
    end: u64,
}

impl SampleVoice {
    /// Start `synth.sample` for this note. `gain` is the final amplitude,
    /// `rate` the playback-rate (pitch) ratio, `chop_index` the running hit
    /// count of this voice (for `| chop N`). `None` if the sample isn't loaded.
    pub fn new(bank: &SampleBank, spec: &NoteSpec, gain: f32, rate: f32, chop_index: u32) -> Option<SampleVoice> {
        SampleVoice::new_named(bank, spec.synth.sample.as_deref()?, spec, gain, rate, chop_index)
    }

    /// Comme [`SampleVoice::new`], mais joue le fichier `name` de la banque (une zone
    /// choisie par [`SampleBank::resolve`]) au lieu de `| sample`.
    pub fn new_named(bank: &SampleBank, name: &str, spec: &NoteSpec, gain: f32, rate: f32, chop_index: u32) -> Option<SampleVoice> {
        let s = spec.synth;
        let data = bank.get(name)?.clone();
        // nothing to play, or a nonsense sample rate (also catches NaN)
        let rates_ok = data.sample_rate > 0.0 && spec.sr > 0.0;
        if data.frames() == 0 || !rates_ok { return None; }
        let gain = if gain.is_finite() { gain.max(0.0) } else { 0.0 };
        let mut rate = if rate.is_finite() && rate > 0.0 { rate } else { 1.0 };
        let mut voice = SampleVoice { data, reverse: false, grains: Vec::new(), first: 0, t: 0, end: 0 };

        if s.stretch && !s.freeze && voice.data.duration() >= 0.02 && spec.dur > 0.03 {
            let frames = (spec.dur as f64 * voice.data.sample_rate as f64).round() as usize;
            if let Some(done) = stretched(&voice.data, frames) {
                // le son étiré joué en entier, comme le web (ni chop, ni begin/end, ni reverse)
                let len = ((done.frames() as f64) * spec.sr as f64 / done.sample_rate as f64).ceil() as u64;
                let step = done.sample_rate as f64 / spec.sr as f64;
                voice.data = done;
                voice.grains.push(Grain { start: 0, len, pos: 0.0, step, gain, shape: Shape::Flat { fade_in: 0, fade_out: 0 }, mix: IDENTITY });
                return Some(voice.finish());
            }
            // pas encore prêt : ce coup-ci en granulaire
        }
        if s.warp || s.freeze {
            if voice.data.duration() >= 0.02 && spec.dur > 0.03 {
                voice.warp(spec, gain, s.freeze);
                return Some(voice.finish());
            }
            rate = 1.0; // too short to stretch: a plain hit at the recorded pitch
        }
        match s.grain {
            Some(amount) => voice.cloud(spec, gain, rate, amount),
            None => voice.plain(spec, gain, rate, chop_index),
        }
        Some(voice.finish())
    }

    /// `playSample` without grains: one read, reversed / chopped / windowed.
    fn plain(&mut self, spec: &NoteSpec, gain: f32, rate: f32, chop_index: u32) {
        let s = spec.synth;
        let frames = self.data.frames() as f64;
        let len_sec = self.data.duration() as f64;
        self.reverse = s.reverse;
        // the region to play, in buffer seconds
        let (offset, length) = if let Some(n) = s.chop.filter(|n| *n > 0) {
            let slice = len_sec / n as f64;
            ((chop_index % n) as f64 * slice, slice)
        } else if s.begin.is_some() || s.end.is_some() {
            let b = s.begin.unwrap_or(0.0) as f64;
            let e = s.end.unwrap_or(1.0) as f64;
            ((b * len_sec).max(0.0), ((e - b) * len_sec).max(0.01))
        } else {
            (0.0, len_sec)
        };
        let sr = self.data.sample_rate as f64;
        let a = (offset * sr).min(frames);
        let b = ((offset + length) * sr).min(frames);
        let step = self.data.sample_rate as f64 / spec.sr as f64 * rate as f64;
        // (the tiny epsilon keeps float noise from adding a frame)
        let len = ((b - a) / step - 1e-6).ceil().max(0.0) as u64;
        if len == 0 { return; }
        // fade only where we cut into the sound: ~1 ms in, ~5 ms out
        let ms = spec.sr as f64 / 1000.0;
        let fade_in = if a > 0.0 { ms.min(len as f64 / 2.0) as u32 } else { 0 };
        let fade_out = if b < frames - 1.0 { (5.0 * ms).min(len as f64 / 2.0) as u32 } else { 0 };
        self.grains.push(Grain { start: 0, len, pos: a, step, gain, shape: Shape::Flat { fade_in, fade_out }, mix: IDENTITY });
    }

    /// `grainSample`: a cloud of short, randomised, panned grains over 0.45 s.
    fn cloud(&mut self, spec: &NoteSpec, gain: f32, rate: f32, amount: f32) {
        let count = (6.0 + amount * 40.0).round().clamp(6.0, 40.0) as u32;
        let glen = (0.04 + amount * 0.06).min(0.12);
        let spread = 0.45;
        let dur = if self.data.duration() > 0.0 { self.data.duration() } else { 0.4 };
        let peak = (gain / (count as f32).sqrt() * 2.0).max(0.001);
        let stereo = self.data.channels.len() == 2;
        let mut rng = Noise::new(spec.seed);
        for k in 0..count {
            // same order of random draws as the JS: time, offset, pitch, pan
            let at = k as f32 / count as f32 * spread + rng.unit() * 0.01;
            let offset = rng.unit() * (dur - glen).max(0.001);
            let r = rate * (1.0 + (rng.unit() - 0.5) * amount * 0.25);
            let pan = (rng.unit() * 2.0 - 1.0) * 0.8;
            self.grains.push(Grain {
                start: (at * spec.sr) as u64,
                len: ((glen * spec.sr) as u64).max(1),
                pos: offset as f64 * self.data.sample_rate as f64,
                step: self.data.sample_rate as f64 / spec.sr as f64 * r as f64,
                gain: peak,
                shape: Shape::Exp { ln_floor: (0.0001 / peak).ln() },
                mix: panner(pan, stereo),
            });
        }
    }

    /// `playSampleWarp`: overlapping grains that fit the whole sample into the
    /// step (`| warp`, pitch kept) or hold one spot of it (`| freeze`).
    fn warp(&mut self, spec: &NoteSpec, gain: f32, frozen: bool) {
        let len = self.data.duration() as f64;
        let dur = spec.dur as f64;
        let (grain, hop): (f64, f64) = if frozen { (0.13, 0.07) } else { (0.10, 0.05) };
        let ratio = len / dur;
        let (sr_out, sr_in) = (spec.sr as f64, self.data.sample_rate as f64);
        let mut out = 0.0;
        let mut read = if frozen { len * 0.35 } else { 0.0 };
        let mut n = 0;
        while out < dur && n < 400 {
            let gl = grain.min(len - read);
            if gl <= 0.006 { break; }
            self.grains.push(Grain {
                start: (out * sr_out).round() as u64,
                len: ((gl * sr_out) as u64).max(1),
                pos: read * sr_in,
                step: sr_in / sr_out,
                gain,
                shape: Shape::Tri,
                mix: IDENTITY,
            });
            out += hop;
            n += 1;
            if !frozen {
                read += hop * ratio;
                if read >= len { break; }
            }
        }
    }

    /// Sort the grains and note when the last one ends.
    fn finish(mut self) -> SampleVoice {
        self.grains.sort_by_key(|g| g.start);
        self.end = self.grains.iter().map(|g| g.start + g.len).max().unwrap_or(0);
        self
    }

    pub fn done(&self) -> bool { self.t >= self.end }

    pub fn render(&mut self, _clock: &Clock) -> Stereo {
        let t = self.t;
        if t >= self.end { return Stereo::ZERO; }
        self.t += 1;
        while self.first < self.grains.len() && t >= self.grains[self.first].start + self.grains[self.first].len {
            self.first += 1;
        }
        let data = &*self.data;
        let reverse = self.reverse;
        let mut out = Stereo::ZERO;
        for g in &mut self.grains[self.first..] {
            if g.start > t { break; }
            let i = t - g.start;
            if i >= g.len { continue; }
            let (l, r) = read(data, reverse, g.pos);
            g.pos += g.step;
            let amp = g.gain * g.env(i);
            out.l += (g.mix[0] * l + g.mix[1] * r) * amp;
            out.r += (g.mix[2] * l + g.mix[3] * r) * amp;
        }
        out
    }
}

/// The (left, right) value at a fractional buffer position. Outside the
/// buffer is silence; mono reads the same value on both sides.
fn read(data: &SampleData, reverse: bool, pos: f64) -> (f32, f32) {
    let i = pos.floor() as i64;
    let frac = (pos - i as f64) as f32;
    let at = |ch: &[f32], k: i64| -> f32 {
        let n = ch.len() as i64;
        if k < 0 || k >= n { 0.0 } else if reverse { ch[(n - 1 - k) as usize] } else { ch[k as usize] }
    };
    let chan = |ch: &[f32]| hermite(at(ch, i - 1), at(ch, i), at(ch, i + 1), at(ch, i + 2), frac);
    let l = chan(&data.channels[0]);
    let r = if data.channels.len() > 1 { chan(&data.channels[1]) } else { l };
    (l, r)
}

/// Dev helper: every sample name a LOOM source asks for (`| sample NAME`,
/// `| s NAME`, `| vocode NAME`), in order of first use.
pub fn sample_names_in(src: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in loom_core::expand_macros(src).lines() {
        let line = line.trim_start();
        if line.starts_with('#') || line.starts_with("//") { continue; }
        for seg in line.split('|').skip(1) {
            let mut synth = loom_core::Synth::default();
            let mut wave = loom_core::Wave::Square;
            if !synth.apply_segment(seg, &mut wave) { continue; }
            for n in [synth.sample, synth.vocode].into_iter().flatten() {
                if !names.contains(&n) { names.push(n); }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_en_direct_ne_bloque_pas_puis_sert_le_cache() {
        let src = Arc::new(SampleData { sample_rate: 8000.0, channels: vec![(0..8000).map(|i| (i as f32 * 0.3).sin()).collect()] });
        let s2 = src.clone();
        std::thread::spawn(move || {
            mark_live_thread();
            assert!(stretched(&s2, 12000).is_none(), "le fil audio n'attend pas");
            let mut got = None;
            for _ in 0..400 {
                got = stretched(&s2, 12000);
                if got.is_some() { break; }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(got.expect("calculé à part").frames(), 12000);
        }).join().unwrap();
        // hors direct : tout de suite, et le même que le cache
        assert_eq!(stretched(&src, 12000).unwrap().frames(), 12000);
    }

    #[test]
    fn index_des_zones_comme_une_relecture_complete() {
        let names = ["piano_c3_v1", "piano", "piano_g3", "bd_rr2", "bd", "bd_rr3", "pianola", "x_c999999999", "snare_rr1"];
        // plusieurs ordres d'arrivée, toujours le même index que zones_of sur tout
        for rot in 0..names.len() {
            let mut bank = SampleBank::empty();
            for k in 0..names.len() {
                let n = names[(k + rot) % names.len()];
                bank.insert(n, SampleData { sample_rate: 8000.0, channels: vec![vec![0.0; 4]] });
            }
            for inst in ["piano", "bd", "snare", "pianola", "x"] {
                let want = loom_core::zones::zones_of(bank.samples.keys().map(String::as_str), inst);
                assert_eq!(bank.zones.get(inst).cloned().unwrap_or_default(), want, "{inst} rot {rot}");
            }
        }
    }
    use loom_core::{Synth, Wave};

    const OUT_SR: f32 = 48000.0;

    fn spec<'a>(synth: &'a Synth, dur: f32, seed: u32) -> NoteSpec<'a> {
        NoteSpec {
            name: "t", synth, wave: Wave::Square, freq: 0.0, dur, peak: 1.0, pan: None,
            prev_freq: None, sr: OUT_SR, bar_sec: 2.0, seed,
        }
    }

    fn synth(verbs: &[&str]) -> Synth {
        let mut s = Synth::default();
        let mut w = Wave::Square;
        s.sample = Some("x".into());
        for v in verbs { assert!(s.apply_segment(v, &mut w), "{v}"); }
        s
    }

    /// A bank with one sample `x`.
    fn bank_with(channels: Vec<Vec<f32>>, sample_rate: f32) -> SampleBank {
        let mut b = SampleBank::empty();
        b.insert("x", SampleData { sample_rate, channels });
        b
    }

    /// One second of a slow ramp 0 → 1 at 44.1 kHz.
    fn ramp_bank() -> SampleBank {
        bank_with(vec![(0..44100).map(|i| i as f32 / 44100.0).collect()], 44100.0)
    }

    /// Four quarters holding 0.1, 0.2, 0.3, 0.4.
    fn steps_bank() -> SampleBank {
        bank_with(vec![(0..48000).map(|i| 0.1 * (1 + i / 12000) as f32).collect()], 48000.0)
    }

    #[test]
    fn user_wav_enters_the_merged_bank() {
        // a real WAV file (PCM16, written by hand) loaded through the session store
        let dir = std::env::temp_dir().join("loom_sample_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mon_son.wav");
        std::fs::write(&path, pcm16_wav_bytes(800)).unwrap();
        let name = load_user_sample(&path).unwrap();
        assert_eq!(name, "mon_son");
        let bank = merged_bank();
        let data = bank.get("mon_son").expect("le WAV chargé doit être audible");
        assert_eq!(data.frames(), 800);
        // a broken file is refused with a message
        let bad = dir.join("casse.wav");
        std::fs::write(&bad, b"pas un wav").unwrap();
        assert!(load_user_sample(&bad).is_err());
        // session samples survive a disk reload
        reload_bank();
        assert!(merged_bank().get("mon_son").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn render_all(v: &mut SampleVoice) -> Vec<Stereo> {
        let c = Clock::new(OUT_SR);
        let mut out = Vec::new();
        while !v.done() {
            out.push(v.render(&c));
            assert!(out.len() < 10 * OUT_SR as usize, "never finishes");
        }
        out
    }

    #[test]
    fn missing_sample_is_none() {
        let s = synth(&[]);
        assert!(SampleVoice::new(&SampleBank::empty(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).is_none());
        let no = Synth::default(); // no `| sample`
        assert!(SampleVoice::new(&ramp_bank(), &spec(&no, 0.1, 1), 1.0, 1.0, 1).is_none());
    }

    #[test]
    fn done_timing_converts_the_sample_rate() {
        let s = synth(&[]);
        let mut v = SampleVoice::new(&ramp_bank(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap();
        assert!(!v.done());
        let n = render_all(&mut v).len();
        assert_eq!(n, 48000, "1 s at 44.1 kHz lasts 48000 frames at 48 kHz");
        assert_eq!(v.render(&Clock::new(OUT_SR)), Stereo::ZERO);
    }

    #[test]
    fn an_octave_up_halves_the_duration() {
        let s = synth(&[]);
        let bank = ramp_bank();
        let one = render_all(&mut SampleVoice::new(&bank, &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap()).len();
        let up = render_all(&mut SampleVoice::new(&bank, &spec(&s, 0.1, 1), 1.0, 2.0, 1).unwrap()).len();
        assert_eq!(up, one / 2);
        // halfway through the octave-up note we're at the ramp's middle
        let mut v = SampleVoice::new(&bank, &spec(&s, 0.1, 1), 1.0, 2.0, 1).unwrap();
        let out = render_all(&mut v);
        assert!((out[up / 2].l - 0.5).abs() < 0.01, "{}", out[up / 2].l);
    }

    #[test]
    fn reverse_plays_backwards() {
        let s = synth(&["reverse"]);
        let out = render_all(&mut SampleVoice::new(&ramp_bank(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap());
        assert!(out[10].l > 0.99, "starts at the end of the ramp: {}", out[10].l);
        assert!((out[24000].l - 0.5).abs() < 0.01);
        assert!(out[47000].l < out[24000].l);
    }

    #[test]
    fn chop_plays_slices_in_turn() {
        let s = synth(&["chop 4"]);
        let bank = steps_bank();
        for (index, want) in [(1, 0.2), (2, 0.3), (4, 0.1), (7, 0.4)] {
            let out = render_all(&mut SampleVoice::new(&bank, &spec(&s, 0.1, 1), 1.0, 1.0, index).unwrap());
            assert_eq!(out.len(), 12000, "a quarter of the sample");
            assert!((out[6000].l - want).abs() < 1e-4, "slice {index}: {}", out[6000].l);
            // a cut in the middle of the sound fades out instead of clicking
            if index % 4 != 3 { assert!(out.last().unwrap().l.abs() < 0.01); }
        }
    }

    #[test]
    fn begin_and_end_pick_a_window() {
        let s = synth(&["begin 0.5", "end 0.75"]);
        let out = render_all(&mut SampleVoice::new(&steps_bank(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap());
        assert_eq!(out.len(), 12000);
        assert!((out[6000].l - 0.3).abs() < 1e-4);
        // end before begin: the browser's 10 ms minimum
        let s = synth(&["begin 0.8", "end 0.2"]);
        let out = render_all(&mut SampleVoice::new(&steps_bank(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap());
        assert_eq!(out.len(), 480);
    }

    #[test]
    fn grains_are_deterministic_per_seed() {
        let s = synth(&["grain 0.7"]);
        let bank = ramp_bank();
        let run = |seed| render_all(&mut SampleVoice::new(&bank, &spec(&s, 0.1, seed), 0.5, 1.0, 1).unwrap());
        let (a, b, c) = (run(7), run(7), run(8));
        assert_eq!(a, b);
        assert_ne!(a, c);
        // ~0.45 s spread + one grain of ≤ 0.12 s
        let secs = a.len() as f32 / OUT_SR;
        assert!(secs > 0.45 && secs < 0.58, "{secs}");
        assert!(a.iter().any(|x| (x.l - x.r).abs() > 0.01), "grains are panned");
    }

    #[test]
    fn mono_is_centred_and_stereo_stays_stereo() {
        let s = synth(&[]);
        let mono = render_all(&mut SampleVoice::new(&ramp_bank(), &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap());
        assert!(mono.iter().all(|x| x.l == x.r));
        let st = bank_with(vec![vec![0.5; 1000], vec![-0.25; 1000]], 48000.0);
        let out = render_all(&mut SampleVoice::new(&st, &spec(&s, 0.1, 1), 1.0, 1.0, 1).unwrap());
        assert!((out[500].l - 0.5).abs() < 1e-5 && (out[500].r + 0.25).abs() < 1e-5);
    }

    #[test]
    fn warp_fits_the_step_and_freeze_holds() {
        let bank = ramp_bank();
        for verb in ["warp", "freeze"] {
            let s = synth(&[verb]);
            let out = render_all(&mut SampleVoice::new(&bank, &spec(&s, 0.5, 1), 0.8, 1.0, 1).unwrap());
            let secs = out.len() as f32 / OUT_SR;
            assert!((0.5..0.65).contains(&secs), "{verb}: {secs}");
            if verb == "warp" {
                // the whole ramp is squeezed into 0.5 s: late output reads late material
                assert!(out[out.len() * 3 / 4].l > 0.5, "{}", out[out.len() * 3 / 4].l);
            } else {
                // frozen at 35 % of the sample
                let mid = out[out.len() / 2].l;
                assert!(mid > 0.1 && mid < 0.45, "{mid}");
            }
        }
    }

    #[test]
    fn nothing_goes_nan() {
        let tiny = bank_with(vec![vec![1.0]], 44100.0);
        let bank = ramp_bank();
        let cases: [&[&str]; 7] = [&[], &["reverse"], &["chop 64"], &["begin 1"], &["grain 1"], &["warp"], &["freeze", "reverse"]];
        for verbs in cases {
            let s = synth(verbs);
            for b in [&tiny, &bank] {
                for (gain, rate) in [(1.0, 1.0), (f32::NAN, f32::NAN), (1.0, 1e-3), (1.0, 100.0), (f32::INFINITY, -1.0)] {
                    for dur in [0.0, 0.01, 2.0] {
                        let mut v = SampleVoice::new(b, &spec(&s, dur, 3), gain, rate, 5).unwrap();
                        let c = Clock::new(OUT_SR);
                        for _ in 0..200_000 {
                            if v.done() { break; }
                            let x = v.render(&c);
                            assert!(x.l.is_finite() && x.r.is_finite(), "{verbs:?} {gain} {rate}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn repo_samples_load_quickly() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/samples");
        let t = std::time::Instant::now();
        let bank = SampleBank::load_dirs(&[dir]);
        let ms = t.elapsed().as_millis();
        assert!(bank.names().len() >= 41, "{:?}", bank.names());
        let bd = bank.get("bd").unwrap();
        assert!(bd.frames() > 1000 && bd.sample_rate > 0.0);
        assert!(bank.get("pad2").unwrap().channels.len() == 2, "float stereo file");
        eprintln!("loaded {} samples in {ms} ms", bank.names().len());
        // when the test binary lives under the repo's target/, the repo folder is found from it
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().unwrap();
        if std::env::current_exe().unwrap().starts_with(&repo) {
            assert!(SampleBank::default_dirs().contains(&repo.join("web/samples")));
        }
    }

    /// Lists the sample names example songs use that aren't in `web/samples`.
    /// Informational: run with `--nocapture` to see the list.
    #[test]
    fn report_missing_song_samples() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let bank = SampleBank::load_dirs(&[root.join("web/samples")]);
        let mut missing: Vec<(String, Vec<String>)> = Vec::new();
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "loom")).collect();
        paths.sort();
        for path in paths {
            let src = std::fs::read_to_string(&path).unwrap();
            let song = path.file_name().unwrap().to_string_lossy().to_string();
            for name in sample_names_in(&src) {
                if bank.get(&name).is_some() { continue; }
                match missing.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, songs)) => songs.push(song.clone()),
                    None => missing.push((name, vec![song.clone()])),
                }
            }
        }
        missing.sort();
        for (name, songs) in &missing {
            eprintln!("missing sample `{name}` used by {}", songs.join(", "));
        }
        assert_eq!(sample_names_in("a: x | s bd | vocode\n# b: x | sample no\nc: x | sample sd"), ["bd", "vox", "sd"]);
    }

    #[test]
    #[cfg(feature = "embed")]
    fn sons_embarques_sans_dossier() {
        // installé seul (aucun dossier de sons) : les sons de base sont là quand même
        let mut bank = SampleBank::empty();
        bank.add_embedded();
        assert!(bank.get("bd").is_some(), "bd absent");
        assert!(embedded::EMBEDDED.len() >= 40, "{}", embedded::EMBEDDED.len());
        assert_eq!(bank.names().len(), embedded::EMBEDDED.len());
    }
}
