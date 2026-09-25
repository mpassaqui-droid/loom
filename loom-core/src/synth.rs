//! Per-voice sound design: every `| verb` that shapes the SOUND rather than the
//! pattern (filters, envelopes, FM, samples, effects, sends…).
//!
//! This is a faithful port of the browser engine's `parse()` (web/index.html):
//! same verb names, same defaults, same clamping. `loom-core` only *describes*
//! the sound; `loom-audio` is what renders it.

/// A parameter that may move over time. Mirrors the JS `pval`/`parseSig`:
/// `800` (constant), `200..2000` (sweep over the section or `| over N` bars),
/// or `sine 0.5 200 2000` (a continuous signal: shape, rate in cycles per bar, min, max).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Param {
    Const(f32),
    Sweep { a: f32, b: f32 },
    Signal { shape: SigShape, rate: f32, a: f32, b: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigShape { Sine, Saw, Tri, Square, Rand }

impl Param {
    /// Parse the value at `p[i]` (and following words for a signal).
    fn parse(p: &[&str], i: usize) -> Option<Param> {
        let w = p.get(i).map(|s| s.to_lowercase()).unwrap_or_default();
        let shape = match w.as_str() {
            "sine" => Some(SigShape::Sine),
            "saw" => Some(SigShape::Saw),
            "tri" => Some(SigShape::Tri),
            "square" => Some(SigShape::Square),
            "rand" => Some(SigShape::Rand),
            _ => None,
        };
        if let Some(shape) = shape {
            let rate = num(p, i + 1).filter(|x| *x != 0.0).unwrap_or(1.0);
            let a = num(p, i + 2).unwrap_or(0.0);
            let b = num(p, i + 3).unwrap_or(1.0);
            return Some(Param::Signal { shape, rate, a, b });
        }
        if let Some((a, b)) = w.split_once("..") {
            if let (Ok(a), Ok(b)) = (a.parse::<f32>(), b.parse::<f32>()) {
                return Some(Param::Sweep { a, b });
            }
        }
        w.parse::<f32>().ok().filter(|x| x.is_finite()).map(Param::Const)
    }

    /// The constant value, or the start of a sweep/signal range (for static uses).
    pub fn base(&self) -> f32 {
        match *self { Param::Const(x) => x, Param::Sweep { a, .. } | Param::Signal { a, .. } => a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType { Lowpass, Highpass, Bandpass, Notch }

/// `| env A D S R` — seconds, seconds, level 0..1, seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Env { pub a: f32, pub d: f32, pub s: f32, pub r: f32 }

/// `| lfo TARGET RATE DEPTH SHAPE` — target is `cutoff`, `q`/`reso`/`res`, `pitch`, `pan`, `gain`/`vol`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lfo { pub target: String, pub rate: f32, pub depth: f32, pub shape: String }

/// `| fm RATIO INDEX FEEDBACK`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fm { pub ratio: f32, pub index: f32, pub fb: f32 }

/// `| echo DIV FB` — tempo-synced feedback delay, DIV taps per bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Echo { pub div: f32, pub fb: f32 }

/// `| eq LOW MID HIGH` in dB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eq3 { pub l: f32, pub m: f32, pub h: f32 }

/// `| bend SEMITONES TIME` — start detuned and glide to pitch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bend { pub semi: f32, pub time: f32 }

/// `| harmonize SEMI LEVEL` — a parallel layer at a fixed interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Harm { pub semi: i32, pub lvl: f32 }

/// `| reson FREQ DECAY` — tuned comb resonator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reson { pub freq: f32, pub decay: f32 }

/// `| flange RATE DEPTH`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flange { pub rate: f32, pub depth: f32 }

/// Everything about how a voice sounds. `Default` = the plain square-ish
/// chip voice the browser plays with no sound verbs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Synth {
    // --- filter & amplitude ---
    pub cutoff: Option<Param>,
    pub filter_type: Option<FilterType>,
    /// Filter resonance, 0.1..20 (default 1).
    pub q: Option<f32>,
    pub env: Option<Env>,
    /// `| fenv N` — filter envelope amount 0..1 (sweeps the cutoff with the env).
    pub fenv: Option<f32>,
    /// `| gate N` / `| sus N` — note length multiplier 0.05..4.
    pub gate: Option<f32>,
    pub lfo: Option<Lfo>,
    pub lfo2: Option<Lfo>,
    /// `| over N` — bars a `A..B` sweep takes (default: the section, or 16 bars).
    pub over: Option<f32>,

    // --- oscillators & sources ---
    pub unison: Option<u32>,
    /// Detune in cents between unison voices, 0..100.
    pub detune: Option<f32>,
    /// `| spread N` / `| width N` — stereo spread of unison voices, 0..1.
    pub spread: Option<f32>,
    /// `| wt organ|warm|rich|soft` — wavetable.
    pub wt: Option<String>,
    /// `| partials 1 0.5 …` / `| organ 888000000` — additive harmonic amplitudes.
    pub partials: Option<Vec<f32>>,
    pub fm: Option<Fm>,
    /// `| sub N` — sine one octave below, level 0..1.
    pub sub: Option<f32>,
    /// `| double N` — octave-up layer, level 0..1.
    pub double: Option<f32>,
    pub harm2: Option<Harm>,
    /// `| noise N` / `| air N` — high-passed noise layer, 0..1.
    pub noise: Option<f32>,
    /// `| pluck N` — Karplus-Strong string, brightness 0..1.
    pub pluck: Option<f32>,
    pub bow: bool,
    pub reed: bool,
    /// `| sync RATIO` — hard-sync lead, max ratio 1.2..8.
    pub sync: Option<f32>,
    /// `| pwm N` — pulse-width modulation depth 0.05..1.
    pub pwm: Option<f32>,
    /// `| bell` — modal (struck metal) synthesis.
    pub bell: bool,
    /// `| grain N` — granular cloud density 0.05..1 (also used by samples).
    pub grain: Option<f32>,
    /// `| slide N` — portamento seconds.
    pub slide: Option<f32>,
    pub bend: Option<Bend>,
    /// `| wow N` — tape wow & flutter 0..1.
    pub wow: Option<f32>,

    // --- per-note inserts ---
    pub drive: Option<Param>,
    /// `| fold N` — wavefolder 0..1.
    pub fold: Option<f32>,
    /// `| crush BITS` 1..8.
    pub crush: Option<u32>,
    /// `| redux N` — sample-rate reduction 0..1.
    pub redux: Option<f32>,
    pub reson: Option<Reson>,
    pub flange: Option<Flange>,
    /// `| stereo N` — mid/side width 0..2.
    pub width: Option<f32>,
    /// `| ring HZ` — ring modulation.
    pub ring: Option<f32>,
    /// `| am HZ` — amplitude modulation.
    pub am: Option<f32>,
    /// `| phaser N` 0..1.
    pub phaser: Option<f32>,
    /// `| vowel a|e|i|o|u` — formant filter.
    pub vowel: Option<char>,
    /// `| vocode SAMPLE` — channel vocoder modulated by a sample.
    pub vocode: Option<String>,
    pub eq: Option<Eq3>,
    /// `| comp N` — per-voice compressor 0..1.
    pub comp: Option<f32>,
    /// `| tgate N` — trance gate, N pulses per bar.
    pub tgate: Option<u32>,
    pub pan: Option<Param>,
    /// `| autopan RATE` Hz.
    pub autopan: Option<f32>,
    /// `| orbit RATE` Hz — circles the listener.
    pub orbit: Option<f32>,
    /// `| haas N` — precedence-effect widener 0..1.
    pub haas: Option<f32>,
    /// `| chorus N` wet 0..1.
    pub chorus: Option<f32>,
    pub echo: Option<Echo>,
    /// `| ir hall|room|plate|spring|cathedral [MIX]` — convolution-style reverb insert.
    pub ir: Option<String>,
    pub irmix: Option<f32>,

    // --- sends & routing ---
    /// `| reverb N` / `| room N` — send to the master reverb 0..1.
    pub reverb: Option<f32>,
    /// `| delay N` — send to the master ping-pong delay 0..1.
    pub delay: Option<f32>,
    /// `| bus NAME` — submix bus.
    pub bus: Option<String>,
    /// `| midi CHANNEL` 1..16 — send notes to MIDI instead of the synth.
    pub midi: Option<u8>,

    // --- samples ---
    /// `| sample NAME` / `| s NAME`.
    pub sample: Option<String>,
    /// `| root NOTE` — the pitch the sample was recorded at (default c3).
    pub root: Option<String>,
    /// `| reverse` — play the sample backwards.
    pub reverse: bool,
    /// `| warp` — time-stretch the sample to the step.
    pub warp: bool,
    /// `| freeze` — hold a granular smear of the sample.
    pub freeze: bool,
    /// `| begin N` / `| end N` — playback window 0..1.
    pub begin: Option<f32>,
    pub end: Option<f32>,
    /// `| chop N` — slice the sample into N pieces played in turn.
    pub chop: Option<u32>,

    // --- performance (applied by the player when a note is triggered) ---
    /// `| chord maj|min|…` — harmonizer intervals added to every note.
    pub chord: Option<Vec<i32>>,
    /// `| semi N` — transpose in semitones.
    pub semi: Option<i32>,
    /// `| strum SECONDS` — stagger chord notes.
    pub strum: Option<f32>,
    /// `| flam` — soft grace note just before each hit.
    pub flam: bool,
    /// `| humanize N` 0..1 — random timing/velocity.
    pub humanize: Option<f32>,
    /// `| voicing open|close|drop2…`.
    pub voicing: Option<String>,
    /// `| cresc` (1) / `| dim` (-1).
    pub dynamics: Option<i8>,
    /// `| arp MODE OCTAVES` — octaves spanned by the arpeggio (1..4).
    pub arp_octaves: Option<u32>,
    pub mute: bool,
    pub solo: bool,
    /// `| viz STYLE` / `| hue DEG` — visual hints (kept for the GUI).
    pub viz: Option<String>,
    pub hue: Option<f32>,
}

fn num(p: &[&str], i: usize) -> Option<f32> {
    p.get(i).and_then(|s| s.parse::<f32>().ok()).filter(|x| x.is_finite())
}

/// JS `x>1 ? x/100 : x` percentages, with a default when missing.
fn pct(p: &[&str], i: usize, default: f32) -> f32 {
    let n = num(p, i).unwrap_or(default);
    if n > 1.0 { n / 100.0 } else { n }
}

fn chord_intervals(name: &str) -> Vec<i32> {
    match name {
        "maj" => vec![0, 4, 7],
        "min" => vec![0, 3, 7],
        "sus" => vec![0, 5, 7],
        "sus2" => vec![0, 2, 7],
        "dim" => vec![0, 3, 6],
        "aug" => vec![0, 4, 8],
        "fifth" => vec![0, 7],
        "oct" => vec![0, 12],
        "maj7" => vec![0, 4, 7, 11],
        "min7" => vec![0, 3, 7, 10],
        "dom7" => vec![0, 4, 7, 10],
        "add9" => vec![0, 4, 7, 14],
        _ => vec![0, 4, 7],
    }
}

impl Synth {
    /// Try to read one `| segment` as a sound verb. Returns `false` when the
    /// segment isn't a sound verb (it's then a pattern verb like `rev`).
    /// `wave` is updated by the `supersaw` macro.
    pub fn apply_segment(&mut self, seg: &str, wave: &mut crate::Wave) -> bool {
        let p: Vec<&str> = seg.split_whitespace().collect();
        let Some(&head) = p.first() else { return false };
        match head {
            "lp" | "hp" | "bp" | "notch" => {
                self.filter_type = Some(match head {
                    "lp" => FilterType::Lowpass,
                    "hp" => FilterType::Highpass,
                    "bp" => FilterType::Bandpass,
                    _ => FilterType::Notch,
                });
                self.cutoff = Some(Param::parse(&p, 1).unwrap_or(Param::Const(800.0)));
                if let Some(qi) = p.iter().position(|w| *w == "q") {
                    self.q = Some(num(&p, qi + 1).filter(|x| *x != 0.0).unwrap_or(1.0).clamp(0.1, 20.0));
                }
            }
            "q" => self.q = Some(num(&p, 1).filter(|x| *x != 0.0).unwrap_or(1.0).clamp(0.1, 20.0)),
            "env" => {
                self.env = Some(Env {
                    a: num(&p, 1).filter(|x| *x != 0.0).unwrap_or(0.01).max(0.0),
                    d: num(&p, 2).filter(|x| *x != 0.0).unwrap_or(0.1).max(0.0),
                    s: if p.len() > 3 { num(&p, 3).unwrap_or(0.0).clamp(0.0, 1.0) } else { 0.6 },
                    r: num(&p, 4).filter(|x| *x != 0.0).unwrap_or(0.2).max(0.0),
                })
            }
            "lfo" | "lfo2" => {
                let lfo = Lfo {
                    target: p.get(1).unwrap_or(&if head == "lfo" { "cutoff" } else { "pitch" }).to_string(),
                    rate: num(&p, 2).filter(|x| *x != 0.0).unwrap_or(4.0),
                    depth: if p.len() > 3 { num(&p, 3).unwrap_or(0.0) } else { 0.5 },
                    shape: p.get(4).unwrap_or(&"sine").to_string(),
                };
                if head == "lfo" { self.lfo = Some(lfo) } else { self.lfo2 = Some(lfo) }
            }
            "echo" => {
                let div = num(&p, 1).filter(|x| *x != 0.0).unwrap_or(4.0).clamp(0.25, 32.0);
                self.echo = Some(Echo { div, fb: num(&p, 2).map_or(0.4, |f| f.clamp(0.0, 0.9)) });
            }
            "unison" => self.unison = Some(p.get(1).and_then(|s| s.parse::<u32>().ok()).filter(|x| *x != 0).unwrap_or(1).clamp(1, 7)),
            "supersaw" => {
                *wave = crate::Wave::Saw;
                self.unison = Some(7);
                self.detune = Some(18.0);
                self.spread = Some(0.85);
            }
            "detune" => self.detune = Some(num(&p, 1).unwrap_or(0.0).clamp(0.0, 100.0)),
            "drive" => self.drive = Param::parse(&p, 1),
            "pan" => self.pan = Param::parse(&p, 1),
            "over" => { if let Some(o) = num(&p, 1).filter(|x| *x > 0.0) { self.over = Some(o) } }
            "sample" | "s" => self.sample = p.get(1).map(|s| s.to_string()),
            "root" => self.root = p.get(1).map(|s| s.to_string()),
            "reverb" | "room" => self.reverb = Some(num(&p, 1).unwrap_or(0.0).clamp(0.0, 1.0)),
            "delay" => self.delay = Some(num(&p, 1).unwrap_or(0.0).clamp(0.0, 1.0)),
            "ir" => {
                self.ir = Some(p.get(1).unwrap_or(&"hall").to_string());
                if let Some(m) = num(&p, 2) { self.irmix = Some(m.clamp(0.0, 1.0)) }
            }
            "voicing" | "voice" => self.voicing = Some(p.get(1).unwrap_or(&"open").to_string()),
            "cresc" => self.dynamics = Some(1),
            "dim" | "decresc" => self.dynamics = Some(-1),
            "reverse" | "rev?" => self.reverse = true,
            "warp" | "stretch" => self.warp = true,
            "freeze" => self.freeze = true,
            "begin" => self.begin = Some(num(&p, 1).unwrap_or(0.0).clamp(0.0, 1.0)),
            "end" => self.end = Some(num(&p, 1).unwrap_or(1.0).clamp(0.0, 1.0)),
            "autopan" => self.autopan = Some(num(&p, 1).filter(|x| *x != 0.0).unwrap_or(1.0).clamp(0.02, 20.0)),
            "haas" | "widen" => self.haas = Some(num(&p, 1).unwrap_or(0.5).clamp(0.0, 1.0)),
            "tape" => {
                let amt = num(&p, 1).unwrap_or(0.5).clamp(0.0, 1.0);
                self.wow = Some(0.4 * amt);
                let d = self.drive.map_or(0.0, |d| d.base());
                self.drive = Some(Param::Const(d.max(0.22 * amt)));
                self.noise = Some(self.noise.unwrap_or(0.0).max(0.14 * amt));
            }
            "partials" | "harm" => {
                let a: Vec<f32> = p[1..].iter().filter_map(|s| s.parse::<f32>().ok()).take(32).collect();
                if !a.is_empty() { self.partials = Some(a) }
            }
            "organ" | "drawbars" => {
                let s: String = p.get(1).unwrap_or(&"888000000").chars().filter(|c| ('0'..='8').contains(c)).collect();
                if !s.is_empty() { self.partials = Some(s.chars().map(|d| d.to_digit(10).unwrap() as f32 / 8.0).collect()) }
            }
            "harmonize" | "hmz" => {
                if let Some(sm) = p.get(1).and_then(|s| s.parse::<i32>().ok()) {
                    self.harm2 = Some(Harm { semi: sm.clamp(-24, 24), lvl: num(&p, 2).map_or(0.5, |l| l.clamp(0.0, 1.0)) });
                }
            }
            "tgate" | "trancegate" => self.tgate = Some(p.get(1).and_then(|s| s.parse::<u32>().ok()).filter(|x| *x != 0).unwrap_or(8).clamp(1, 32)),
            "midi" => self.midi = Some(p.get(1).and_then(|s| s.parse::<u8>().ok()).filter(|x| *x != 0).unwrap_or(1).clamp(1, 16)),
            "fm" => {
                self.fm = Some(Fm {
                    ratio: num(&p, 1).filter(|x| *x != 0.0).unwrap_or(2.0).clamp(0.1, 16.0),
                    index: num(&p, 2).unwrap_or(3.0).clamp(0.0, 12.0),
                    fb: num(&p, 3).unwrap_or(0.0).clamp(0.0, 8.0),
                })
            }
            "eq" => {
                let cl = |i| num(&p, i).unwrap_or(0.0).clamp(-18.0, 18.0);
                self.eq = Some(Eq3 { l: cl(1), m: cl(2), h: cl(3) });
            }
            "viz" => self.viz = Some(p.get(1).unwrap_or(&"burst").to_string()),
            "hue" => { if let Some(h) = num(&p, 1) { self.hue = Some(h.rem_euclid(360.0)) } }
            "wt" => self.wt = p.get(1).map(|s| s.to_string()),
            "chorus" => self.chorus = Some(num(&p, 1).unwrap_or(0.5).clamp(0.0, 1.0)),
            "crush" => self.crush = Some(p.get(1).and_then(|s| s.parse::<u32>().ok()).filter(|x| *x != 0).unwrap_or(4).clamp(1, 8)),
            "gate" | "sus" => self.gate = Some(num(&p, 1).filter(|x| *x != 0.0).unwrap_or(1.0).clamp(0.05, 4.0)),
            "slide" | "glide" => self.slide = Some(num(&p, 1).map_or(0.06, |n| n.clamp(0.005, 0.5))),
            "ring" => { if let Some(n) = num(&p, 1) { self.ring = Some(n.clamp(1.0, 8000.0)) } }
            "sub" => self.sub = Some(pct(&p, 1, 0.6).clamp(0.0, 1.0)),
            "noise" | "air" => self.noise = Some(pct(&p, 1, 0.4).clamp(0.0, 1.0)),
            "vowel" | "formant" => {
                let vw = p.get(1).map(|s| s.to_lowercase()).unwrap_or_else(|| "a".into());
                if let Some(c) = vw.chars().next().filter(|c| "aeiou".contains(*c)) { self.vowel = Some(c) }
            }
            "pluck" | "string" => self.pluck = Some(num(&p, 1).map_or(0.6, |n| n.clamp(0.0, 1.0))),
            "bend" => {
                if let Some(s) = num(&p, 1) {
                    self.bend = Some(Bend { semi: s.clamp(-48.0, 48.0), time: num(&p, 2).map_or(0.08, |t| t.clamp(0.005, 2.0)) });
                }
            }
            "bell" | "modal" => self.bell = true,
            "spread" | "width" => self.spread = Some(pct(&p, 1, 0.7).clamp(0.0, 1.0)),
            "fold" => self.fold = Some(pct(&p, 1, 0.5).clamp(0.0, 1.0)),
            "grain" | "granular" => self.grain = Some(pct(&p, 1, 0.5).clamp(0.05, 1.0)),
            "phaser" => self.phaser = Some(pct(&p, 1, 0.5).clamp(0.0, 1.0)),
            "am" => { if let Some(n) = num(&p, 1) { self.am = Some(n.clamp(0.1, 8000.0)) } }
            "flam" | "grace" => self.flam = true,
            "orbit" | "rotate" => self.orbit = Some(num(&p, 1).map_or(0.5, |n| n.clamp(0.02, 20.0))),
            "humanize" => self.humanize = Some(pct(&p, 1, 0.3).clamp(0.0, 1.0)),
            "strum" => self.strum = Some(num(&p, 1).map_or(0.02, |n| n.clamp(0.003, 0.25))),
            "wow" | "flutter" => self.wow = Some(pct(&p, 1, 0.4).clamp(0.0, 1.0)),
            "mute" => self.mute = true,
            "solo" => self.solo = true,
            "bus" => self.bus = Some(p.get(1).unwrap_or(&"main").to_lowercase()),
            "comp" => self.comp = Some(pct(&p, 1, 0.5).clamp(0.0, 1.0)),
            "double" | "octup" => self.double = Some(pct(&p, 1, 0.5).clamp(0.0, 1.0)),
            "bow" => self.bow = true,
            "reed" | "wind" => self.reed = true,
            "sync" => self.sync = Some(num(&p, 1).map_or(3.0, |n| n.clamp(1.2, 8.0))),
            "vocode" | "vocoder" => self.vocode = Some(p.get(1).unwrap_or(&"vox").to_string()),
            "redux" => self.redux = Some(pct(&p, 1, 0.5).clamp(0.0, 1.0)),
            "reson" => {
                if let Some(f) = num(&p, 1) {
                    self.reson = Some(Reson { freq: f.clamp(20.0, 8000.0), decay: num(&p, 2).map_or(0.92, |d| d.clamp(0.0, 0.995)) });
                }
            }
            "flange" => {
                self.flange = Some(Flange {
                    rate: num(&p, 1).map_or(0.3, |r| r.clamp(0.01, 10.0)),
                    depth: num(&p, 2).map_or(0.6, |d| d.clamp(0.0, 1.0)),
                })
            }
            "stereo" => self.width = Some(num(&p, 1).unwrap_or(1.4).clamp(0.0, 2.0)),
            "pwm" => self.pwm = Some(pct(&p, 1, 0.6).clamp(0.05, 1.0)),
            "fenv" => self.fenv = Some(pct(&p, 1, 0.6).clamp(0.0, 1.0)),
            "semi" | "transpose" => { if let Some(n) = p.get(1).and_then(|s| s.parse::<i32>().ok()) { self.semi = Some(n.clamp(-36, 36)) } }
            "chop" => { if let Some(n) = p.get(1).and_then(|s| s.parse::<u32>().ok()) { self.chop = Some(n.clamp(2, 64)) } }
            "chord" => self.chord = Some(chord_intervals(&p.get(1).unwrap_or(&"maj").to_lowercase())),
            _ => return false,
        }
        true
    }
}

/// `room: fdn 0.84 4200` / `room: schroeder` — which master reverb algorithm.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Room {
    /// `None` = the default Schroeder reverb; `Some("fdn")` = feedback delay network.
    pub kind: Option<String>,
    pub decay: Option<f32>,
    pub damp: Option<f32>,
}

/// `bus: NAME gain G lp F hp F q Q reverb R` — a named submix.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BusConfig {
    pub name: String,
    pub gain: Option<f32>,
    pub lp: Option<f32>,
    pub hp: Option<f32>,
    pub q: Option<f32>,
    pub reverb: Option<f32>,
}

/// The mix-wide directives the browser applies to the whole song.
#[derive(Debug, Clone, PartialEq)]
pub struct MixSettings {
    /// `duck: N` — sidechain pump depth on every kick, 0..0.95.
    pub duck: f32,
    /// `feel: N` — humanisation of timing/velocity, 0..1.
    pub feel: f32,
    /// `warmth: N` — master saturation drive, 0..1 (default 0.3).
    pub warmth: f32,
    pub room: Room,
    pub buses: Vec<BusConfig>,
    /// `morph: N` — crossfade changed voices over N bars on re-evaluation (0 = instant).
    pub morph: u32,
    /// `loop: SECTION` — repeat one section of the song.
    pub loop_section: Option<String>,
    /// `tune: equal|just|pyth|werck|kirn|meantone|adaptive|N edo`.
    pub tune: String,
}

fn directive<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    for raw in src.lines() {
        let line = crate::decomment(raw).trim();
        let Some(i) = line.find(':') else { continue };
        if line[..i].trim().eq_ignore_ascii_case(name) { return Some(line[i + 1..].trim()); }
    }
    None
}

fn directive_pct(src: &str, name: &str, default: f32, max: f32) -> f32 {
    match directive(src, name).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse::<f32>().ok()) {
        Some(n) => (if n > 1.0 { n / 100.0 } else { n }).clamp(0.0, max),
        None => default,
    }
}

impl MixSettings {
    pub fn parse(src: &str) -> MixSettings {
        let room = directive(src, "room").map(|v| {
            let w: Vec<&str> = v.split_whitespace().collect();
            Room {
                kind: w.first().map(|s| s.to_lowercase()).filter(|s| !s.is_empty()),
                decay: num(&w, 1),
                damp: num(&w, 2),
            }
        }).unwrap_or_default();
        let mut buses: Vec<BusConfig> = Vec::new();
        for raw in src.lines() {
            let line = crate::decomment(raw).trim();
            let Some(i) = line.find(':') else { continue };
            if !line[..i].trim().eq_ignore_ascii_case("bus") { continue; }
            let w: Vec<&str> = line[i + 1..].split_whitespace().collect();
            let Some(name) = w.first().map(|s| s.to_lowercase()) else { continue };
            let idx = match buses.iter().position(|b| b.name == name) {
                Some(k) => k,
                None => { buses.push(BusConfig { name, ..Default::default() }); buses.len() - 1 }
            };
            let b = &mut buses[idx];
            let mut k = 1;
            while k + 1 < w.len() {
                let val = num(&w, k + 1);
                let key = w[k].to_lowercase();
                match (key.as_str(), val) {
                    ("gain", Some(v)) => b.gain = Some(v.clamp(0.0, 2.0)),
                    ("lp", Some(v)) => b.lp = Some(v),
                    ("hp", Some(v)) => b.hp = Some(v),
                    ("q", Some(v)) => b.q = Some(v),
                    ("reverb" | "rev", Some(v)) => b.reverb = Some(v),
                    _ => { k += 1; continue; }
                }
                k += 2;
            }
        }
        let tune = directive(src, "tune").map(|w| {
            let w = w.to_lowercase();
            let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() && w[digits.len()..].trim() == "edo" {
                format!("{} edo", digits.parse::<u32>().unwrap_or(12).clamp(3, 96))
            } else if w.starts_with("just") || w == "ji" { "just".into() }
            else if w.starts_with("pyth") { "pyth".into() }
            else if w.starts_with("werck") { "werck".into() }
            else if w.starts_with("kirn") { "kirn".into() }
            else if w.starts_with("mean") || w.starts_with("quart") { "meantone".into() }
            else if w.starts_with("adapt") { "adaptive".into() }
            else { "equal".into() }
        }).unwrap_or_else(|| "equal".into());
        MixSettings {
            duck: directive_pct(src, "duck", 0.0, 0.95),
            feel: directive_pct(src, "feel", 0.0, 1.0),
            warmth: directive_pct(src, "warmth", 0.3, 1.0),
            room,
            buses,
            morph: directive(src, "morph").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0).min(32),
            loop_section: directive(src, "loop").map(|s| s.to_lowercase()).filter(|s| !s.is_empty()),
            tune,
        }
    }
}

/// `def NAME: text` lines define macros used as `$NAME` elsewhere (up to 4 levels deep).
pub fn expand_macros(src: &str) -> String {
    let mut defs: Vec<(String, String)> = Vec::new();
    let mut kept = Vec::new();
    for line in src.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("def") {
            if rest.starts_with(char::is_whitespace) {
                if let Some((name, body)) = rest.trim_start().split_once(':') {
                    let name = name.trim();
                    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !name.starts_with(|c: char| c.is_ascii_digit()) {
                        defs.push((name.to_string(), body.trim().to_string()));
                        continue;
                    }
                }
            }
        }
        kept.push(line);
    }
    let mut out = kept.join("\n");
    if defs.is_empty() { return out; }
    // longest names first so `$bassline` isn't eaten by `$bass`
    defs.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
    for _ in 0..4 {
        let mut changed = false;
        let mut next = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(i) = rest.find('$') {
            next.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            let len = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').map(char::len_utf8).sum::<usize>();
            let name = &after[..len];
            match defs.iter().find(|(n, _)| n == name) {
                Some((_, body)) if len > 0 => { next.push_str(body); changed = true; }
                _ => { next.push('$'); next.push_str(name); }
            }
            rest = &after[len..];
        }
        next.push_str(rest);
        out = next;
        if !changed { break; }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Wave;

    fn synth(segs: &[&str]) -> (Synth, Wave) {
        let mut s = Synth::default();
        let mut w = Wave::Square;
        for seg in segs { assert!(s.apply_segment(seg, &mut w), "not a sound verb: {seg}"); }
        (s, w)
    }

    #[test]
    fn filters_envelopes_and_signals() {
        let (s, _) = synth(&["lp 1100 q 6", "env 1.2 0.8 0.7 3", "drive sine 0.5 0.1 0.6", "pan -0.3..0.3"]);
        assert_eq!(s.filter_type, Some(FilterType::Lowpass));
        assert_eq!(s.cutoff, Some(Param::Const(1100.0)));
        assert_eq!(s.q, Some(6.0));
        assert_eq!(s.env, Some(Env { a: 1.2, d: 0.8, s: 0.7, r: 3.0 }));
        assert_eq!(s.drive, Some(Param::Signal { shape: SigShape::Sine, rate: 0.5, a: 0.1, b: 0.6 }));
        assert_eq!(s.pan, Some(Param::Sweep { a: -0.3, b: 0.3 }));
    }

    #[test]
    fn macros_percentages_and_samples() {
        let (s, w) = synth(&["supersaw", "sub 60", "sample bd", "reverb 0.7", "organ 80800000", "chord min7", "fm 3.5 4"]);
        assert_eq!(w, Wave::Saw);
        assert_eq!((s.unison, s.detune, s.spread), (Some(7), Some(18.0), Some(0.85)));
        assert_eq!(s.sub, Some(0.6));
        assert_eq!(s.sample.as_deref(), Some("bd"));
        assert_eq!(s.reverb, Some(0.7));
        assert_eq!(s.partials.as_ref().unwrap()[..3], [1.0, 0.0, 1.0]);
        assert_eq!(s.chord, Some(vec![0, 3, 7, 10]));
        assert_eq!(s.fm, Some(Fm { ratio: 3.5, index: 4.0, fb: 0.0 }));
        let mut w = Wave::Square;
        assert!(!Synth::default().apply_segment("every 4 rev", &mut w), "pattern verbs are not sound verbs");
    }

    #[test]
    fn mix_directives() {
        let m = MixSettings::parse("duck: 58\nfeel: 0.2\nroom: fdn 0.9 3000\nbus: drums gain 0.8 hp 40\nbus: pads reverb 0.5\ntune: 19 edo\nloop: Drop");
        assert!((m.duck - 0.58).abs() < 1e-6 && (m.feel - 0.2).abs() < 1e-6);
        assert_eq!(m.warmth, 0.3);
        assert_eq!(m.room, Room { kind: Some("fdn".into()), decay: Some(0.9), damp: Some(3000.0) });
        assert_eq!(m.buses.len(), 2);
        assert_eq!((m.buses[0].gain, m.buses[0].hp), (Some(0.8), Some(40.0)));
        assert_eq!(m.tune, "19 edo");
        assert_eq!(m.loop_section.as_deref(), Some("drop"));
    }

    #[test]
    fn expands_macros() {
        let src = "def fx: reverb 0.5 | delay 0.3\ndef b: $fx | gain 0.2\npad: c3 | $b";
        assert_eq!(expand_macros(src), "pad: c3 | reverb 0.5 | delay 0.3 | gain 0.2");
        assert_eq!(expand_macros("a: x | $nope"), "a: x | $nope");
    }
}
